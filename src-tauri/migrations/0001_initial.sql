-- CreatorFlow 0001: 初始结构
-- 约定：业务主键为 UUID 字符串；时间为 UTC ISO-8601 字符串；字段使用 snake_case。

CREATE TABLE works (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    description   TEXT NULL,
    status        TEXT NOT NULL CHECK (
        status IN ('idea', 'scripting', 'collecting', 'in_production',
                   'review', 'published', 'archived', 'creating', 'error')
    ),
    root_path     TEXT NOT NULL,
    template_key  TEXT NULL,
    target_platform TEXT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    archived_at   TEXT NULL,
    deleted_at    TEXT NULL
);

CREATE INDEX idx_works_status ON works (status);
CREATE INDEX idx_works_updated_at ON works (updated_at);

CREATE TABLE sections (
    id            TEXT PRIMARY KEY,
    work_id       TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    key           TEXT NOT NULL CHECK (key IN ('documents', 'audio', 'video', 'images')),
    name          TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    sort_order    INTEGER NOT NULL,
    created_at    TEXT NOT NULL,
    UNIQUE (work_id, key),
    UNIQUE (work_id, relative_path)
);

CREATE INDEX idx_sections_work_id ON sections (work_id);

CREATE TABLE assets (
    id                TEXT PRIMARY KEY,
    work_id           TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    section_id        TEXT NOT NULL REFERENCES sections (id) ON DELETE CASCADE,
    display_name      TEXT NOT NULL,
    original_name     TEXT NOT NULL,
    relative_path     TEXT NOT NULL,
    source_path       TEXT NULL,
    extension         TEXT NULL,
    detected_mime_type TEXT NULL,
    media_kind        TEXT NOT NULL CHECK (
        media_kind IN ('document', 'audio', 'video', 'image', 'other')
    ),
    size_bytes        INTEGER NOT NULL DEFAULT 0,
    sha256            TEXT NULL,
    status            TEXT NOT NULL CHECK (
        status IN ('importing', 'ready', 'missing', 'failed', 'trashed')
    ),
    is_favorite       INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    deleted_at        TEXT NULL
);

CREATE UNIQUE INDEX idx_assets_work_relative_path
    ON assets (work_id, relative_path)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_assets_work_id ON assets (work_id);
CREATE INDEX idx_assets_section_id ON assets (section_id);
CREATE INDEX idx_assets_media_kind ON assets (media_kind);
CREATE INDEX idx_assets_status ON assets (status);
CREATE INDEX idx_assets_display_name ON assets (display_name);

CREATE TABLE import_jobs (
    id                     TEXT PRIMARY KEY,
    work_id                TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    section_id             TEXT NOT NULL REFERENCES sections (id) ON DELETE CASCADE,
    source_path            TEXT NOT NULL,
    target_relative_path   TEXT NULL,
    temporary_relative_path TEXT NULL,
    source_size_bytes      INTEGER NULL,
    bytes_copied           INTEGER NOT NULL DEFAULT 0,
    status                 TEXT NOT NULL CHECK (
        status IN ('queued', 'copying', 'verifying', 'completed', 'failed', 'cancelled')
    ),
    error_code             TEXT NULL,
    error_message          TEXT NULL,
    asset_id               TEXT NULL REFERENCES assets (id) ON DELETE SET NULL,
    created_at             TEXT NOT NULL,
    updated_at             TEXT NOT NULL,
    completed_at           TEXT NULL
);

CREATE INDEX idx_import_jobs_work_id ON import_jobs (work_id);
CREATE INDEX idx_import_jobs_status ON import_jobs (status);

CREATE TABLE app_settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE activity_logs (
    id           TEXT PRIMARY KEY,
    work_id      TEXT NULL REFERENCES works (id) ON DELETE SET NULL,
    asset_id     TEXT NULL REFERENCES assets (id) ON DELETE SET NULL,
    action       TEXT NOT NULL,
    summary      TEXT NOT NULL,
    details_json TEXT NULL,
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_activity_logs_work_id ON activity_logs (work_id);
CREATE INDEX idx_activity_logs_created_at ON activity_logs (created_at);
