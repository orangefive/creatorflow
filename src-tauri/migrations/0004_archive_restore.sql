-- 记录归档前的状态，取消归档时按原状态恢复；
-- 旧数据为空时由服务层回落到「策划中」。
ALTER TABLE works ADD COLUMN archived_from_status TEXT NULL;

-- 列表过滤已删除记录时，按更新时间排序会走这个索引
CREATE INDEX IF NOT EXISTS idx_works_alive_updated
    ON works (updated_at DESC)
    WHERE deleted_at IS NULL;
