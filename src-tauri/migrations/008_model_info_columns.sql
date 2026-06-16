-- Migration 021 (file 008): model_price 加模型信息列。
-- 列为索引快速读取（出站裁剪、列表展示）；price_data JSON 仍存完整原始数据（resolve_price 用）。
-- NULL = 未知/无限制。实际执行在 db.rs init_tables 内联 ALTER（幂等忽略 duplicate column）。
-- 数据源: data/models.json (GitHub raw, Python 聚合生成, 见 scripts/pricing/)。

ALTER TABLE model_price ADD COLUMN max_input_tokens INTEGER;
ALTER TABLE model_price ADD COLUMN max_output_tokens INTEGER;
ALTER TABLE model_price ADD COLUMN context_window INTEGER;
