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

## 4. 一致性约定

- 数据库是**索引**，真实文件在磁盘。文件缺失时保留记录并标记 `missing`，绝不静默删除。
- 复制与数据库登记不是原子操作，因此 `import_jobs` 承担状态机职责，并由启动恢复服务兜底。
- 时间统一 UTC ISO-8601（毫秒精度，`Z` 后缀）；前端按本地时区展示。

## 5. 备份注意事项

- **不要**在应用运行时只复制 `creatorflow.db`：WAL 模式下还有 `-wal` / `-shm` 文件，直接复制可能拿到不一致快照。
- 正确做法：先退出应用，再复制整个数据目录；或使用 SQLite Online Backup API（`rusqlite::backup`）做热备份。
- 第二阶段将提供「数据库安全备份 / 恢复校验」UI 与自动备份策略。
