-- CreatorFlow 0002: 素材媒体元数据与缩略图状态
-- 兼容第一阶段数据：新增列均有默认值，已有素材自动标记为待提取。

ALTER TABLE assets ADD COLUMN width INTEGER NULL;
ALTER TABLE assets ADD COLUMN height INTEGER NULL;
ALTER TABLE assets ADD COLUMN duration_ms INTEGER NULL;
ALTER TABLE assets ADD COLUMN frame_rate REAL NULL;
ALTER TABLE assets ADD COLUMN sample_rate INTEGER NULL;
ALTER TABLE assets ADD COLUMN channels INTEGER NULL;
ALTER TABLE assets ADD COLUMN codec TEXT NULL;

ALTER TABLE assets ADD COLUMN metadata_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (metadata_status IN ('pending', 'ready', 'partial', 'failed', 'unsupported'));
ALTER TABLE assets ADD COLUMN metadata_error TEXT NULL;
ALTER TABLE assets ADD COLUMN metadata_updated_at TEXT NULL;

ALTER TABLE assets ADD COLUMN thumbnail_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (thumbnail_status IN ('pending', 'ready', 'failed', 'unsupported'));
ALTER TABLE assets ADD COLUMN thumbnail_relative_path TEXT NULL;
ALTER TABLE assets ADD COLUMN thumbnail_error TEXT NULL;
ALTER TABLE assets ADD COLUMN thumbnail_updated_at TEXT NULL;

-- 列表默认按导入时间倒序，按作品过滤后排序使用该索引
CREATE INDEX idx_assets_work_created_at
    ON assets (work_id, created_at DESC)
    WHERE deleted_at IS NULL;
