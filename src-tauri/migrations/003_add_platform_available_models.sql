-- 存储平台从 API 获取到的可用模型列表
ALTER TABLE platforms ADD COLUMN available_models TEXT NOT NULL DEFAULT '[]';
