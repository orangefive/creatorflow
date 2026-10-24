-- CreatorFlow 0003: 标签与素材关联

CREATE TABLE tags (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE,
    created_at      TEXT NOT NULL
);

CREATE TABLE asset_tags (
    asset_id   TEXT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    tag_id     TEXT NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (asset_id, tag_id)
);

CREATE INDEX idx_asset_tags_tag_id ON asset_tags (tag_id);
