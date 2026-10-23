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

## 4. 安全设计

1. **路径穿越防护**：`normalize_relative` 拒绝绝对路径、盘符、`..`、控制字符；`resolve_within` 再对最近的已存在祖先做 canonicalize 并校验前缀，防止符号链接逃逸。
2. **绝不覆盖**：复制使用 `OpenOptions::create_new(true)`；重命名前再次检查目标存在性。
3. **不暴露内部细节**：`AppError` 序列化为 `{ code, message, detail }`，`message` 为中文用户文案，`detail` 只写日志（序列化时自动 `tracing::error!`）。
4. **无 shell**：打开文件/定位文件使用 opener 插件的结构化 API，不拼接命令行字符串。
5. **最小权限**：capabilities 仅放行事件与文件选择对话框。
6. **数据库损坏防护**：外键开启、WAL、busy_timeout=5s；迁移逐版本事务执行。

## 5. 错误模型

`ErrorCode`：`invalid_input`、`not_found`、`conflict`、`permission_denied`、`disk_full`、`io_failure`、`database_failure`、`migration_failure`、`cancelled`、`internal`。

前端 `IpcError` 统一归一化，界面只展示 `message`；`detail` 用于问题排查。

## 6. 线程与并发

- 命令层统一走 `tauri::async_runtime::spawn_blocking`，避免阻塞主线程。
- SQLite 连接放在 `Mutex` 中，所有操作都是短事务；文件复制期间不持锁。
- 导入任务每文件一个后台线程；第一阶段的并发上限留给第二阶段（批量导入与受限并发）。
- 进度写库做了时间节流，避免频繁锁争用。

## 7. 已知技术债

- 导入线程数未设上限，大量文件同时导入会争用磁盘（第二阶段引入队列与并发上限）。
- 重命名时 manifest 重写失败只记录警告，不回滚（manifest 为辅助识别文件）。
- 素材列表的存在性校准逐条 `stat`，超大分区可考虑批量或缓存（第二阶段）。
- 未实现数据库备份 UI（第二阶段）。
