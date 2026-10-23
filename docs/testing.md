# 测试与验收

## 1. 自动化测试清单

### 1.1 Rust（`cargo test`）

单元测试位于各自模块内：

| 模块 | 覆盖内容 |
|---|---|
| `domain::work` | 作品名称合法/非法（空值、首尾空格、非法字符、保留名、长度、结尾句点）；状态解析与用户可选状态 |
| `domain::section` | 四个分区 key 稳定、目录名与排序、未知 key 拒绝、扩展名 → 分区映射、分区与媒体类型对应 |
| `domain::asset` | 媒体类型与素材状态往返解析、MIME 映射 |
| `domain::import` | 导入状态往返解析、终态/运行中判定 |
| `infrastructure::filesystem` | 相对路径规范化（`\` 与 `/`、`.//`）、拒绝 `..` 与盘符、`resolve_within` 限制在根目录内、唯一文件名递增后缀、无扩展名去重、流式复制后源文件保留且大小一致、目标存在时拒绝覆盖、取消标志生效 |
| `infrastructure::database` | 打开数据库自动建表（含 `schema_migrations`）、外键约束生效 |

集成测试 `src-tauri/tests/core_flow.rs`（全部使用临时目录与临时数据库）：

| 测试 | 验证点 |
|---|---|
| `create_work_builds_directories_manifest_and_sections` | 作品目录、四个分区目录、manifest 字段、4 条 sections 及顺序、作品列表 |
| `create_work_rejects_duplicate_directory` | 同名目录冲突返回 `conflict` |
| `rename_work_moves_directory_and_updates_index` | 目录改名、分区随之移动、`root_path` 与 manifest 同步 |
| `archive_work_keeps_directory_and_marks_status` | 归档只改状态，目录保留；默认列表隐藏归档作品 |
| `import_copies_file_and_registers_asset` | 复制完成、目标内容一致、**源文件保留**、asset 记录与大小正确 |
| `import_never_overwrites_existing_file` | 同名目标保留原内容，新文件落为 `脚本 (1).md` |
| `import_marks_job_failed_when_source_missing` | 源文件缺失时任务失败且有可读错误 |
| `deleted_file_is_marked_missing_not_removed_from_index` | 删除磁盘文件后记录保留并标记 `missing`；恢复文件后回到 `ready` |
| `startup_recovery_marks_interrupted_jobs_safely` | 目标完整 → 恢复为 completed 并补登记；仅有临时文件 → 清理并置 failed(`interrupted`) |
| `sqlite_rejects_duplicate_sections_and_paths` | 分区 key 与相对路径唯一约束生效 |
| `updating_work_status_persists_and_rejects_system_status` | 状态持久化；`archived` 不能走通用状态接口 |

### 1.2 前端（`vitest`）

| 文件 | 覆盖内容 |
|---|---|
| `src/utils/format.spec.ts` | 字节/时间/进度格式化、Windows 与 POSIX 路径取文件名与扩展名、异常输入 |
| `src/utils/validation.spec.ts` | 与 Rust 一致的作品名称校验规则 |
| `src/stores/imports.spec.ts` | 进度事件应用到对应任务、未知任务忽略、完成信号只触发一次、活动/失败任务分类、移除记录 |
| `src/components/WorkStatusTag.spec.ts` | 状态文案与标签类型渲染 |

## 2. 质量门槛命令

```bash
npm run typecheck     # vue-tsc --noEmit
npm run lint          # eslint . --max-warnings 0
npm run test          # vitest run
npm run rust:fmt      # cargo fmt --check
npm run rust:clippy   # cargo clippy --all-targets -- -D warnings
npm run rust:test     # cargo test
npm run tauri:build   # 生产构建 + 安装包
```

## 3. 实际执行结果（2026-10-20）

| 命令 | 结果 |
|---|---|
| `npm run typecheck` | 通过（0 error） |
| `npm run lint` | 通过（0 error / 0 warning） |
| `npm run test` | 4 个测试文件、23 个用例全部通过 |
| `npm run rust:fmt` | 通过 |
| `npm run rust:clippy` | 通过（`-D warnings`，无告警） |
| `npm run rust:test` | 27 单元测试 + 11 集成测试 = 38 全部通过 |
| `npm run tauri:build` | 通过：`src-tauri/target/release/bundle/nsis/CreatorFlow_0.1.0_x64-setup.exe`（2.29 MiB），主程序 5.7 MB |
| `npx tauri build --debug --no-bundle` | 成功，产物 `src-tauri/target/debug/creatorflow.exe` |

**应用启动验证**：实际运行后成功创建 `%APPDATA%\com.creatorflow.desktop\creatorflow.db`（含迁移记录）、
`%LOCALAPPDATA%\com.creatorflow.desktop\logs\creatorflow.log.*` 与 WebView2 数据目录，日志中可见
「打开本地数据库」「已应用数据库迁移 version=1」。

**关于测试二进制的 Windows 清单**：Rust 集成测试可执行文件默认不带应用清单，Windows 会加载
comctl32 v5 并在启动时报 `0xc0000139`。`src-tauri/build.rs` 通过
`cargo:rustc-link-arg-tests` 为测试目标声明 Common-Controls 6.0 依赖，解决该问题；
应用本体由 Tauri 注入清单，不受影响。

## 4. 手动验收流程

前置：准备四个样本文件（`脚本.md`、`配乐.mp3`、`片段.mp4`、`封面.png`），建议其中包含一个 > 100 MB 的视频用于观察进度。

1. **首次启动**：应用打开，首页提示未设置作品根目录。
2. **设置根目录**：设置页 →「更改目录」→ 选择空目录（例如 `D:\CreatorFlow-Works`）→ 保存成功。
3. **创建作品**：首页 →「新建作品」→ 名称填 `我的第一支视频`，描述可选 → 选择目录后创建。
   - 磁盘上应出现 `D:\CreatorFlow-Works\我的第一支视频\{01_文档,02_音频,03_视频,04_图片}` 与 `creatorflow.manifest.json`。
4. **名称校验**：再建一个作品，输入 `CON` 或 `a/b`，应给出中文提示且不创建目录。
5. **导入单个文件**：进入作品 → 文档分区 →「导入文件」→ 选择 `脚本.md` → 确认对话框显示大小为 `.md`、目标分区「文档」→ 开始导入。
   - 队列面板出现任务，状态从排队 → 复制中 → 校验中 → 已完成。
   - 列表出现 `脚本.md`；`01_文档` 下出现同名文件；原始文件仍在原位置。
6. **拖拽导入**：把 `封面.png` 拖入窗口 → 确认对话框默认分区「图片」（按扩展名建议）→ 导入。
   - 图片分区的行点击「预览」能看到图片。
7. **同名不覆盖**：再次导入同一个 `脚本.md`。
   - 目标出现 `脚本 (1).md`；原 `脚本.md` 内容未被覆盖。
8. **大文件**：导入 > 100 MB 的视频，观察进度条推进、界面仍可切换分区与滚动。
9. **打开与定位**：对任意素材点「打开」（系统默认程序启动）、「定位」（资源管理器选中文件）。
10. **状态切换**：作品页把状态改为「制作中」，刷新后仍为「制作中」。
11. **重命名**：首页 → 重命名 → `我的第一支视频-改名`。
    - 磁盘目录同步改名，分区与文件仍在，manifest 中 `name` 已更新。
12. **重启恢复**：完全关闭应用 → 重新启动 → 作品列表、素材数量、状态、分区内容与关闭前一致。
13. **缺失检测**：在资源管理器中把 `01_文档/脚本.md` 改名 → 回到应用点「刷新」。
    - 该行显示「文件缺失」红色标记，记录未消失；顶部出现缺失提示。
14. **归档**：首页 →「归档」→ 默认列表不再显示；打开「包含已归档」可见，磁盘目录未被移动或删除。
15. **中断恢复（可选）**：导入大文件时用任务管理器结束进程 → 重启应用。
    - 应看到启动检查提示；该任务不在「复制中」悬挂，而是完成或失败可重试，`.part` 临时文件被清理。

## 5. 已知未覆盖

- 无 Playwright/Tauri 端到端自动化：当前以手动验收步骤 + 单元/集成测试覆盖。
- 未覆盖真实磁盘写满、只读目录、网络盘断开等场景的自动化测试（第二阶段补齐）。
