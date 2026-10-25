-- 第四阶段：作品任务与作品模板
--
-- 任务属于作品，作品删除时级联清理；模板与任务相互独立，
-- 应用模板时把初始任务复制成作品自己的任务记录，之后修改模板不影响已有作品。

CREATE TABLE tasks (
    id           TEXT PRIMARY KEY,
    work_id      TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    description  TEXT NULL,
    status       TEXT NOT NULL CHECK (status IN ('todo', 'in_progress', 'done', 'cancelled')),
    priority     TEXT NOT NULL CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    due_at       TEXT NULL,
    sort_order   INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    completed_at TEXT NULL
);

CREATE INDEX idx_tasks_work_status ON tasks (work_id, status);
CREATE INDEX idx_tasks_work_order ON tasks (work_id, sort_order, created_at);
CREATE INDEX idx_tasks_work_due ON tasks (work_id, due_at);

CREATE TABLE work_templates (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NULL,
    is_builtin  INTEGER NOT NULL DEFAULT 0 CHECK (is_builtin IN (0, 1)),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE template_tasks (
    id          TEXT PRIMARY KEY,
    template_id TEXT NOT NULL REFERENCES work_templates (id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    description TEXT NULL,
    priority    TEXT NOT NULL CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);

CREATE INDEX idx_template_tasks_template ON template_tasks (template_id, sort_order);

-- 内置模板：固定 id 与时间，保证迁移可重复验证
INSERT INTO work_templates (id, name, description, is_builtin, created_at, updated_at) VALUES
    ('builtin-video', '通用视频', '从选题到发布的通用视频制作流程', 1,
     '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'),
    ('builtin-clip', '直播切片', '把长直播剪成可发布的短视频切片', 1,
     '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'),
    ('builtin-podcast', '播客 / 音频节目', '一期音频节目的制作与上线流程', 1,
     '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');

INSERT INTO template_tasks (id, template_id, title, description, priority, sort_order, created_at) VALUES
    ('tpl-video-1', 'builtin-video', '确定选题与脚本', '写清主题、目标观众与结构', 'high', 0, '2026-01-01T00:00:00.000Z'),
    ('tpl-video-2', 'builtin-video', '收集素材', '画面、音效、引用素材统一放进对应分区', 'normal', 1, '2026-01-01T00:00:00.000Z'),
    ('tpl-video-3', 'builtin-video', '粗剪', '先搭结构，确定时长与节奏', 'normal', 2, '2026-01-01T00:00:00.000Z'),
    ('tpl-video-4', 'builtin-video', '精剪与字幕', '处理转场、音频与字幕', 'high', 3, '2026-01-01T00:00:00.000Z'),
    ('tpl-video-5', 'builtin-video', '封面与标题', '准备封面图、标题与简介', 'normal', 4, '2026-01-01T00:00:00.000Z'),
    ('tpl-video-6', 'builtin-video', '发布与复盘', '发布后记录数据与改进点', 'low', 5, '2026-01-01T00:00:00.000Z'),

    ('tpl-clip-1', 'builtin-clip', '挑选切片片段', '标出高光时间点与理由', 'high', 0, '2026-01-01T00:00:00.000Z'),
    ('tpl-clip-2', 'builtin-clip', '备份直播源', '保留原始录像，避免二次下载', 'normal', 1, '2026-01-01T00:00:00.000Z'),
    ('tpl-clip-3', 'builtin-clip', '裁切与调色', '统一画幅与基础色调', 'normal', 2, '2026-01-01T00:00:00.000Z'),
    ('tpl-clip-4', 'builtin-clip', '字幕与贴纸', '补字幕、重点强调与花字', 'normal', 3, '2026-01-01T00:00:00.000Z'),
    ('tpl-clip-5', 'builtin-clip', '批量导出与发布', '按平台规格导出并发布', 'low', 4, '2026-01-01T00:00:00.000Z'),

    ('tpl-podcast-1', 'builtin-podcast', '确定话题与嘉宾', '确认主题、时长与嘉宾档期', 'high', 0, '2026-01-01T00:00:00.000Z'),
    ('tpl-podcast-2', 'builtin-podcast', '录音', '双轨录制并检查电平', 'normal', 1, '2026-01-01T00:00:00.000Z'),
    ('tpl-podcast-3', 'builtin-podcast', '剪辑与降噪', '去口误、压噪、统一响度', 'high', 2, '2026-01-01T00:00:00.000Z'),
    ('tpl-podcast-4', 'builtin-podcast', '撰写 shownotes', '时间轴、提及链接与嘉宾信息', 'normal', 3, '2026-01-01T00:00:00.000Z'),
    ('tpl-podcast-5', 'builtin-podcast', '上线与分发', '上传各平台并检查播放', 'low', 4, '2026-01-01T00:00:00.000Z');
