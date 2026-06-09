-- Platform models 配置
-- 存储为 JSON 数组字符串，如 '["gpt-4o","gpt-4o-mini"]'

-- SQLite 不支持 IF NOT EXISTS on ALTER TABLE，用 try-catch 在 Rust 侧处理
ALTER TABLE platforms ADD COLUMN models TEXT NOT NULL DEFAULT '[]';
