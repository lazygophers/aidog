-- Model price table: stores per-model pricing data synced from LiteLLM or entered manually

CREATE TABLE IF NOT EXISTS model_price (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    model_name       TEXT NOT NULL DEFAULT '',
    source           TEXT NOT NULL DEFAULT 'manual',  -- 'litellm' | 'manual'
    price_data       TEXT NOT NULL DEFAULT '{}',       -- JSON: {input_cost_per_token, output_cost_per_token, cache_read_input_token_cost, pricing: {platform_type: {...}}, default_platform, ...}
    created_at       INTEGER NOT NULL DEFAULT 0,
    updated_at       INTEGER NOT NULL DEFAULT 0,
    deleted_at       INTEGER NOT NULL DEFAULT 0,
    UNIQUE(model_name, source)
);

CREATE INDEX IF NOT EXISTS idx_model_price_name
    ON model_price(model_name) WHERE deleted_at = 0;
