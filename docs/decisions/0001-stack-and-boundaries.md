# ADR 0001：第一阶段技术选型与边界

- 状态：已采纳
- 日期：2026-10-20

## 背景

第一阶段需要在本机 Windows 上交付可安装、可日常使用的作品/素材管理最小闭环。要求：本地优先、不覆盖用户文件、可测试、可迁移、模块分层。

## 决策

1. **桌面容器：Tauri 2**，而非 Electron。安装包体积小、内存占用低，Rust 侧可直接实现可靠的文件操作与 SQLite 访问，不需要 Node 运行时。
2. **前端：Vue 3 + TypeScript strict + Vite**，UI 组件统一使用 Element Plus，状态管理用 Pinia，路由用 Vue Router（hash 模式，避免打包后路径问题）。
3. **后端：Rust 分四层**——`commands`（校验/调度）、`services`（业务规则）、`repositories`（SQL）、`infrastructure`（数据库/文件系统/日志）。禁止把 SQL 或文件操作散落到命令层。
4. **数据库：`rusqlite`（bundled）**。同步 API 与 `Mutex<Connection>` 组合，所有操作短事务；命令统一 `spawn_blocking`，避免阻塞主线程。选择 bundled 以免依赖系统 SQLite 版本。
5. **迁移：有序 SQL 文件 + `schema_migrations` 表**，逐版本事务执行，失败即中止启动。
6. **导入模型：`import_jobs` 独立状态机** + 临时文件 + rename，复制与登记不在同一事务内，由启动恢复服务兜底。
7. **文件访问最小化**：不启用 asset 协议、不给前端 `fs` 权限；图片预览由后端读文件并返回 data URL（上限 24 MB）；打开/定位文件通过 opener 插件在 Rust 侧执行。
8. **文件命名**：磁盘与数据库统一使用 `/` 分隔的相对路径，实际路径由后端转换，避免跨平台歧义。

## 影响

- 正面：安全边界清晰；测试可在无 GUI 环境下覆盖核心逻辑（`ImportManager::headless`）；打包体积与启动速度优于 Electron。
- 负面：Rust 编译时间较长；同步 SQLite 需要手动管理线程边界；第一阶段每文件一个导入线程，未限制并发。

## 后续

- 第二阶段：导入队列与并发上限、缩略图缓存、数据库热备份（SQLite Backup API）。
- 若引入 FFmpeg/FFprobe，将以外部进程方式调用并固定参数构造，不拼接 shell 字符串。
