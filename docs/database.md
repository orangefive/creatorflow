# 数据库设计与迁移

## 1. 位置与连接配置

- 文件：`<应用数据目录>/creatorflow.db`
  Windows 实际路径：`%APPDATA%\com.creatorflow.desktop\creatorflow.db`
- 数据库**不放在作品目录内**，也不建议放在网络共享目录供多机同时写入。
- 连接初始化：

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;
```

- 连接由 `infrastructure/database::Database` 持有（`Mutex<Connection>`），所有访问都是短事务；文件复制期间不持锁。

## 2. 迁移机制

- SQL 文件位于 `src-tauri/migrations/`，按版本号顺序命名：`0001_initial.sql`。
- `schema_migrations(version, name, applied_at)` 记录已执行版本。
- 每个版本在**独立事务**中执行：脚本失败则整体回滚，应用启动中止（不带着未知结构继续写入）。
- 迁移只能追加，不能修改历史文件。

| 版本 | 文件 | 内容 |
|---|---|---|
| 1 | `0001_initial.sql` | 创建 works / sections / assets / import_jobs / app_settings / activity_logs 与索引 |

## 3. 表结构

### 3.1 `works`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | UUID |
| `name` | TEXT NOT NULL | 作品名称 |
| `description` | TEXT NULL | 描述 |
| `status` | TEXT NOT NULL | `idea\|scripting\|collecting\|in_production\|review\|published\|archived\|creating\|error`（CHECK） |
| `root_path` | TEXT NOT NULL | 作品目录绝对路径 |
| `template_key` | TEXT NULL | 模板标识（当前仅 `video-basic`） |
| `target_platform` | TEXT NULL | 预留字段 |
| `created_at` / `updated_at` | TEXT NOT NULL | UTC ISO-8601 |
| `archived_at` / `deleted_at` | TEXT NULL | 归档时间 / 软删除预留 |

索引：`status`、`updated_at`。

### 3.2 `sections`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | UUID |
| `work_id` | TEXT NOT NULL | REFERENCES works(id) ON DELETE CASCADE |
| `key` | TEXT NOT NULL | `documents\|audio\|video\|images`（CHECK） |
| `name` | TEXT NOT NULL | 中文名 |
| `relative_path` | TEXT NOT NULL | 统一用 `/` 分隔的相对路径 |
| `sort_order` | INTEGER NOT NULL | 1—4 |
| `created_at` | TEXT NOT NULL | |

约束：`UNIQUE(work_id, key)`、`UNIQUE(work_id, relative_path)`。

### 3.3 `assets`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | UUID |
| `work_id` / `section_id` | TEXT NOT NULL | 外键，级联删除 |
| `display_name` | TEXT NOT NULL | 实际落盘文件名（含去重后缀） |
| `original_name` | TEXT NOT NULL | 来源文件名 |
| `relative_path` | TEXT NOT NULL | 作品内相对路径（`/` 分隔） |
| `source_path` | TEXT NULL | 来源绝对路径，仅用于追溯 |
| `extension` | TEXT NULL | 小写扩展名 |
| `detected_mime_type` | TEXT NULL | 扩展名映射结果 |
| `media_kind` | TEXT NOT NULL | `document\|audio\|video\|image\|other`（CHECK） |
| `size_bytes` | INTEGER NOT NULL DEFAULT 0 | |
| `sha256` | TEXT NULL | 预留（本阶段不计算） |
| `status` | TEXT NOT NULL | `importing\|ready\|missing\|failed\|trashed`（CHECK） |
| `is_favorite` | INTEGER NOT NULL DEFAULT 0 | |
| `created_at` / `updated_at` | TEXT NOT NULL | |
| `deleted_at` | TEXT NULL | |

索引：`(work_id, relative_path)` 唯一（`WHERE deleted_at IS NULL`）、`work_id`、`section_id`、`media_kind`、`status`、`display_name`。

### 3.4 `import_jobs`

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | TEXT PK | UUID |
| `work_id` / `section_id` | TEXT NOT NULL | 外键 |
| `source_path` | TEXT NOT NULL | 源文件绝对路径 |
| `target_relative_path` | TEXT NULL | 正式目标 |
| `temporary_relative_path` | TEXT NULL | 临时文件（`.part`） |
| `source_size_bytes` | INTEGER NULL | |
| `bytes_copied` | INTEGER NOT NULL DEFAULT 0 | |
| `status` | TEXT NOT NULL | `queued\|copying\|verifying\|completed\|failed\|cancelled`（CHECK） |
| `error_code` / `error_message` | TEXT NULL | |
| `asset_id` | TEXT NULL | REFERENCES assets(id) ON DELETE SET NULL |
| `created_at` / `updated_at` / `completed_at` | TEXT | |

索引：`work_id`、`status`。

### 3.5 `app_settings`

`key TEXT PRIMARY KEY`、`value_json TEXT NOT NULL`、`updated_at TEXT NOT NULL`。
当前使用的键：`workspace_root`、`last_opened_work_id`。只存非敏感配置。

### 3.6 `activity_logs`

记录作品创建/重命名/归档、导入完成/失败等关键动作；`details_json` 只存摘要，不写入文件内容。

### 3.7 任务与模板（0005 新增）

**tasks**

| 列 | 说明 |
|---|---|
| `id` | 主键 |
| `work_id` | 外键 → `works(id)`，`ON DELETE CASCADE` |
| `title` / `description` | 标题（1—120 字符）与可选描述 |
| `status` | `todo` / `in_progress` / `done` / `cancelled` |
| `priority` | `low` / `normal` / `high` / `urgent` |
| `due_at` | 可选，UTC ISO-8601 |
| `sort_order` | 手工顺序，上移/下移时重排 |
| `completed_at` | 与 `status = done` 保持一致 |

索引：`(work_id, status)`、`(work_id, sort_order, created_at)`、`(work_id, due_at)`。

**work_templates**

| 列 | 说明 |
|---|---|
| `id` | 主键；内置模板固定为 `builtin-*` |
| `name` / `description` | 模板名称与说明 |
| `is_builtin` | 1 表示内置，禁止修改与删除 |

**template_tasks**

| 列 | 说明 |
|---|---|
| `id` | 主键 |
| `template_id` | 外键 → `work_templates(id)`，`ON DELETE CASCADE` |
| `title` / `description` / `priority` | 初始任务内容 |
| `sort_order` | 在模板中的顺序 |

## 4. 一致性约定

- 数据库是**索引**，真实文件在磁盘。文件缺失时保留记录并标记 `missing`，绝不静默删除。
- 复制与数据库登记不是原子操作，因此 `import_jobs` 承担状态机职责，并由启动恢复服务兜底。
- 时间统一 UTC ISO-8601（毫秒精度，`Z` 后缀）；前端按本地时区展示。
- 任务与模板不是引用关系：应用模板时把 `template_tasks` 复制为 `tasks`，之后两者互不影响。
- 任务状态与 `completed_at` 必须一致：`done` 有完成时间，其余状态为空。

## 5. 迁移历史

| 版本 | 文件 | 内容 |
|---|---|---|
| 1 | `0001_initial.sql` | 作品、分区、素材、导入任务、设置、活动记录 |
| 2 | `0002_asset_metadata.sql` | 素材媒体元数据与缩略图字段 |
| 3 | `0003_tags.sql` | 标签与关联表 |
| 4 | `0004_archive_restore.sql` | 归档前状态、作品列表索引 |
| 5 | `0005_tasks_and_templates.sql` | 任务、模板、模板任务与三个内置模板 |

## 6. 备份与恢复（0.3.0）

### 5.1 为什么不能直接复制 `.db`

WAL 模式下已提交但未 checkpoint 的数据仍在 `creatorflow.db-wal` 中，运行中只复制主库文件可能得到旧快照或损坏文件。
应用改为一律通过 SQLite 在线备份接口生成快照。

### 5.2 备份产物

| 文件 | 说明 |
|---|---|
| `creatorflow-YYYYMMDD-HHMMSS.db` | 数据库快照（`journal_mode = DELETE`，单文件可用） |
| `creatorflow-YYYYMMDD-HHMMSS.db.json` | 清单：格式版本、应用版本、schema 版本、创建时间、文件名、大小、SHA-256、校验结果 |
| `pre-restore-YYYYMMDD-HHMMSS.db` | 恢复前自动生成的当前库安全快照 |

默认目录 `%APPDATA%\com.creatorflow.desktop\backups\`，可在设置页更改（必须是绝对路径）。

### 5.3 校验规则

- `PRAGMA integrity_check` 必须返回 `ok`。
- `PRAGMA foreign_key_check` 必须为空。
- 必须存在 `schema_migrations`，且最高版本 ≥ 1 且 ≤ 当前应用的最新版本。
- 若清单存在，文件 SHA-256 必须与清单一致。

任一条不通过即标记为不可恢复；界面显示原因，不提供恢复按钮。

### 5.4 恢复流程

1. 校验所选备份；
2. 用在线备份接口为当前数据库生成 `pre-restore-*` 快照并校验；
3. 将备份复制到 `<db>.restoring` 并再次校验；
4. `Database::replace_with`：持锁 checkpoint → 关闭连接 → 旧库改名 `.pre-restore` → 候选改名到位 → 重开连接并执行迁移；
5. 迁移失败或打开失败：回滚 `.pre-restore` 并重新打开原库。

旧 schema 的备份允许恢复，重开连接时由既有迁移升级（0.1.0 → 0.3.0 会依次执行 0002—0004）。来自更高版本的备份直接拒绝。

### 5.5 备份范围

只包含数据库：作品/分区/素材索引、标签、导入任务、设置与活动记录。
**不包含**作品目录中的视频、音频、图片等原文件，也不包含缩略图缓存（可再生成）。
