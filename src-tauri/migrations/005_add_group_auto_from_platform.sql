-- 自动分组：记录由哪个平台自动创建
ALTER TABLE groups ADD COLUMN auto_from_platform TEXT;
