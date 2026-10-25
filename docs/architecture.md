# 架构说明（v0.1.0）

## 1. 进程与信任边界

```text
┌──────────────────────────── WebView（渲染进程） ────────────────────────────┐
│ Vue 3 + TypeScript（strict）                                                │
│   views / components  ──  stores（Pinia）  ──  services/*.ts（invoke 封装）  │
│                      无数据库访问、无任意文件系统访问                        │
└──────────────────────────────────┬──────────────────────────────────────────┘
                                   │  Tauri IPC（强类型命令 + 事件）
┌──────────────────────────────────▼──────────────────────────────────────────┐
│ Rust 主进程                                                                  │
│  commands/     参数校验、service 调用、错误序列化                             │
│  services/     业务规则：作品、素材、导入编排、设置                            │
│  repositories/ 所有 SQL                                                      │
│  infrastructure/database  SQLite 连接、PRAGMA、迁移                          │
│  infrastructure/filesystem 路径规范化、目录、流式复制、唯一命名               │
│  domain/       实体与状态枚举                                                │
│  errors/       错误码 + 中文文案 + 日志                                       │
└──────────────────────────────────┬──────────────────────────────────────────┘
                                   │
                    SQLite（应用数据目录）      作品目录（用户选择的根目录）
```

**信任边界规则**

- WebView 只能调用 `lib.rs` 中注册的命令；`capabilities/default.json` 仅放行 `core:default` 与 `dialog:allow-open`。
- 渲染进程没有 `fs` 插件权限，也没有 shell 权限。
- 图片预览由后端读取并返回 `data:` URL，避免开放 asset 协议范围。
- 打开文件与定位文件由 Rust 调用 opener 插件，路径先经作品根目录校验。

## 2. 模块职责

| 模块 | 职责 | 关键约束 |
|---|---|---|
| `commands/` | 命令入口，克隆 `AppState` 后把阻塞工作交给 `spawn_blocking` | 不写业务逻辑、不直接写 SQL |
| `services/work_service` | 创建/重命名/归档/状态变更 | 负责目录与数据库的一致性补偿 |
| `services/import_service` | 导入任务创建、后台执行、取消、重试、启动恢复 | 不在复制期间持有数据库事务 |
| `services/asset_service` | 素材列表与存在性校准、预览 | 每条路径都经 `resolve_within` |
| `services/settings_service` | 工作区根目录、上次打开作品 | 只存非敏感配置 |
| `repositories/` | 全部 SQL 与行映射 | 上层不出现 SQL 字符串 |
| `infrastructure/filesystem` | 路径安全、流式复制、唯一命名、原子写 JSON | 目标文件用 `create_new` 落盘 |
| `infrastructure/database` | 连接、PRAGMA、迁移执行 | 迁移失败即中止启动 |

## 3. 主要数据流

### 3.1 创建作品

```text
CreateWorkDialog → works.createWork → invoke create_work
  → work_service::create_work
      1) 校验名称与根目录
      2) 插入 works(status=creating)
      3) 建作品目录 + 四个分区目录
      4) 写 creatorflow.manifest.json（临时文件 + rename）
      5) 事务内插入 4 条 sections
      6) 更新 status=idea
  失败：清理本次新建的空目录；全部清理成功则回滚 works 记录，
        否则保留记录并置 status=error，错误写入日志并返回前端。
```

### 3.2 导入素材

```text
AssetListView → imports.start → invoke start_import
  → import_service::start
      1) 校验作品与分区
      2) 每个文件插入 import_jobs(status=queued)
      3) 启动后台线程（每任务一线程）
 线程内 execute_job:
      queued → copying（事件）
        解析源文件 → 目标目录 → unique_file_name → 写 target/temp 相对路径
        copy_streaming（1MB 缓冲，每块检查取消标志）
        进度：每 180ms 写一次库并 emit import://progress
      copying → verifying
        校验临时文件大小 == 源文件大小
        rename(temp → target)
      verifying → completed
        事务内：插入 assets(status=ready) + import_jobs(status=completed, asset_id)
 失败：清理临时文件 → status=failed + error_code/message（事件）
 取消：status=cancelled
```

### 3.3 启动恢复

`lib.rs::initialize` 在注册状态前调用 `recover_incomplete_jobs`：

- 遍历 `queued/copying/verifying` 任务；
- 清理残留 `.part` 临时文件；
- 目标文件存在且大小匹配 → 补登记 asset 并置 `completed`；
- 否则置 `failed`，`error_code=interrupted`，提示可重试；
- 报告通过 `get_startup_recovery_report` 返回，前端用通知展示。

### 3.4 元数据与缩略图（0.2.0）

```text
导入完成 / 启动补齐 / 手动重试
  → MetadataService.enqueue(assetId)
  → 单线程队列依次处理
      图片：image 读取宽高（只解析文件头），再解码生成 480px JPEG 缩略图
      音频：lofty 读取时长/采样率/声道/编码，内嵌封面转缩略图
      视频：mp4 解析时长/宽高/帧率/编码；封面调用本机 ffmpeg（可选）
  → 事务内更新 assets 的元数据与缩略图状态
  → emit asset://metadata，前端刷新当前页
```

- 缩略图存放在应用缓存目录 `<app_cache>/thumbnails/<workId>/<assetId>-<size>-<mtime>.jpg`，文件名带源文件指纹，源文件变化后自动重建。
- 前端通过受控的 `asset://` 协议读取缩略图，协议范围仅限缓存目录（`tauri.conf.json` 的 `assetProtocol.scope`）。
- 媒体信息提取失败只写入 `metadata_status = failed` 与原因，素材本身仍是 `ready`。
- ffmpeg 不随应用分发，仅在用户本机已安装时启用视频封面；未安装时明确显示不可用。

### 3.5 重新定位缺失素材（0.2.0）

```text
选择新文件 → 校验：存在、是普通文件、位于作品根目录内、未被其他素材占用
  → 比较大小；若记录有 SHA-256 且大小一致，再比较哈希
  → 不一致：requiresConfirmation（不写库）
  → 用户确认或内容一致：事务更新 relative_path/size/hash/status，写活动记录
  → 内容不一致时重置元数据为 pending，重新提取
```

### 3.6 归档、恢复与删除（0.2.1）

```text
归档：works.status = archived，同时记录 archived_from_status
恢复：按 archived_from_status 还原（缺失或非法时回落到 idea），清空 archived_at
删除作品：作品根目录 → Shell 回收站 → 事务内软删除 works 与其全部 assets → 清理缩略图缓存
删除素材：单个文件 → Shell 回收站 → 软删除 assets → 解除标签关联 → 删除缩略图
```

- **回收站**：Windows 上调用 `SHFileOperationW`（`FO_DELETE | FOF_ALLOWUNDO | FOF_WANTNUKEWARNING`），文件进入系统回收站，可从回收站还原。路径会先去掉 `\\?\` 前缀，因为该 API 不接受扩展长度路径。
- **顺序**：先动磁盘、再改库。回收站失败（文件被占用、权限不足）时直接返回错误，数据库保持原状，避免「记录没了、文件还在」。
- **软删除**：只写 `deleted_at`，行数据保留；所有列表、分区计数、标签计数、元数据补齐都以 `deleted_at IS NULL` 过滤。
- **删除作品**：同时软删除该作品下全部素材，并清理 `<app_cache>/thumbnails/<workId>/`，缓存可按需重建。
- **文件已缺失时**：仍允许删除记录，不再尝试回收站；路径解析失败（记录异常）时只记日志，绝不触碰作品目录以外的文件。

### 3.7 全局搜索（0.3.0）

```text
GlobalSearchView → AssetQuery（work_id 可空）
  → assets::search：assets JOIN sections JOIN works
  → 条件：关键词（display_name / original_name / works.name）、作品、分区、类型、状态、标签
  → 排序白名单 + LIMIT/OFFSET；总数用同一 WHERE 单独 COUNT
  → 结果按所属作品分别解析根目录，再批量校准存在性
```

- 关键词同时匹配作品名，便于「只记得作品名」的场景。
- 标签过滤使用 `EXISTS (SELECT 1 FROM asset_tags ...)`，按素材维度判断，不会跨作品串数据。
- 结果携带 `work_name` 与 `section_key`，界面无需二次查询。
- 跨作品时不复用单作品路径根，改为按 `work_id` 分组解析（`resolve_assets_path_multi`）。

### 3.8 数据库备份与恢复（0.3.0）

```text
创建：rusqlite::backup 生成快照 → PRAGMA integrity_check + foreign_key_check
     → 计算 SHA-256 → 写 <db>.json 清单（格式版本/应用版本/schema 版本/时间/大小/校验结果）

恢复：校验输入（完整性、外键、schema ≤ 当前）→ 为当前库生成 pre-restore 快照
     → 复制到 <db>.restoring 并再次校验 → Database::replace_with
         → 持锁 checkpoint → 关闭旧连接 → 旧文件改名 .pre-restore
         → 候选改名到位 → 重开连接并执行迁移
         → 任一步失败：回滚 .pre-restore，重新打开原库
```

- **写入屏障**：`replace_with` 全程持有连接互斥锁，其他命令在此期间阻塞，避免活动连接写入旧文件。
- **WAL 安全**：不使用文件复制作为备份手段；切换前 `wal_checkpoint(TRUNCATE)`。
- **旧 schema**：允许恢复，重开连接时按既有迁移升级；出现「结构版本高于当前应用」直接拒绝。
- **只备份索引**：作品原文件与缩略图缓存不在备份内，设置页与 README 明确说明。

### 3.9 缩略图缓存管理（0.3.0）

- 统计：递归枚举缓存目录中的 `.jpg/.jpeg`，得到文件数与总字节；跳过非缩略图文件。
- 上限：`app_settings.thumbnail_cache_limit_bytes`，默认 2 GiB，读写时裁剪到 256 MiB—20 GiB。
- 超限清理：按修改时间升序删除，直到降到上限内；1 分钟内修改过的文件跳过，避免删到正在写入的缩略图。
- 触发：每生成 25 个缩略图检查一次（`WRITE_COUNTER` 原子计数），也可在设置页手动清理全部。
- 清理只作用于缓存目录内、扩展名为 jpg 的文件，不触碰素材原文件与索引记录。

### 3.10 批量操作（0.3.0）

- 批量删除复用单条删除路径（回收站 + 软删除 + 标签解绑 + 缩略图清理），逐项返回 `{ succeeded, failed[] }`。
- 批量标签：先 `tags::ensure` 复用同名标签，再逐个 `attach`/`detach`，失败项记录素材名与错误文案。
- 前端多选使用 `el-table` selection，操作后清空选择并刷新列表与分区计数。

## 4. 安全设计

1. **路径穿越防护**：`normalize_relative` 拒绝绝对路径、盘符、`..`、控制字符；`resolve_within` 再对最近的已存在祖先做 canonicalize 并校验前缀，防止符号链接逃逸。
2. **绝不覆盖**：复制使用 `OpenOptions::create_new(true)`；重命名前再次检查目标存在性。
3. **不暴露内部细节**：`AppError` 序列化为 `{ code, message, detail }`，`message` 为中文用户文案，`detail` 只写日志（序列化时自动 `tracing::error!`）。
4. **无 shell**：打开文件/定位文件使用 opener 插件的结构化 API，不拼接命令行字符串。
5. **最小权限**：capabilities 仅放行事件与文件选择对话框。
6. **数据库损坏防护**：外键开启、WAL、busy_timeout=5s；迁移逐版本事务执行。
7. **删除可回退**：删除只走系统回收站 + 数据库软删除，应用内没有任何不可恢复的抹除路径；回收站失败即中止，不会只删一半。
8. **备份可验证**：备份必须通过 `integrity_check`、`foreign_key_check` 与清单哈希校验；恢复前自动生成当前库快照，失败回滚。
9. **缓存可再生成**：清理缓存只删除缓存目录内的 jpg，不涉及作品目录与素材记录。

## 5. 错误模型

`ErrorCode`：`invalid_input`、`not_found`、`conflict`、`permission_denied`、`disk_full`、`io_failure`、`database_failure`、`migration_failure`、`cancelled`、`internal`。

前端 `IpcError` 统一归一化，界面只展示 `message`；`detail` 用于问题排查。

## 6. 线程与并发（0.2.0）

- 命令层统一走 `tauri::async_runtime::spawn_blocking`，避免阻塞主线程。
- SQLite 连接放在 `Mutex` 中，所有操作都是短事务；文件复制期间不持锁。
- **导入队列**：`ImportManager` 维护一个 `mpsc` 队列与固定数量的工作线程（默认 2，配置范围 1—4），任务排队执行，不再按文件创建线程；活动线程数与峰值可观测（`peak_concurrency` 用于测试）。
- **元数据队列**：单线程顺序处理，避免图片解码或 ffmpeg 并发抢占 CPU。
- 进度写库与事件都做了节流，避免频繁锁争用与前端重绘。

## 7. 已知技术债（0.3.0）

- 重命名时 manifest 重写失败只记录警告，不回滚（manifest 为辅助识别文件）。
- 素材列表的存在性校准已改为按目录批量枚举（0.3.0），目录极大时仍会一次性列出全部文件名。
- 全局搜索使用 `LIKE`，未迁移 FTS5；5000 条规模实测可接受。
- 搜索使用 `LIKE '%关键词%'`，数据量大时无法命中索引，可考虑 FTS5。
- 缩略图缓存没有容量上限与清理策略。
- 音频波形、批量重命名尚未实现。
- 桌面端 E2E 未自动化，依赖手工验收清单（见 `docs/testing.md`）。
- 回收站操作依赖系统对话框行为：文件体积超过回收站容量时会由系统提示「将永久删除」，该提示在后台线程中弹出。
- 未实现数据库备份 UI。
