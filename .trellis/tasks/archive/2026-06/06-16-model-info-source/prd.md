# 模型信息中心化: GitHub JSON 唯一信源

## Goal

当前 `price_sync.rs` 单源拉 LiteLLM JSON → upsert `model_price` 表（仅存价格，max_tokens/context 等字段虽在 price_data JSON 内但**零消费**）。本任务彻底重构：移除旧同步模块，改由 Python 脚本聚合多平台官方定价 → 写入 aidog GitHub 仓库特定目录的 JSON 文件（price + max_tokens + context 等），作为**唯一真实信源**；app 端定时从 GitHub 拉取该文件同步到本地，供 est_cost 计算 + 转换/传递上游 + 展示。

## What I already know (repo 已查证)

### 现有价格架构
- **表**: `model_price` (migration 003): `model_name + source('litellm'|'manual') + price_data(JSON) + ts + soft-delete`，UNIQUE(model_name, source)
- **price_data JSON 字段**: `input_cost_per_token / output_cost_per_token / cache_read_input_token_cost / pricing{platform_type:{...}} / default_platform`；LiteLLM 原始 JSON 还含 `max_input_tokens / max_output_tokens / context_window / mode / supports_function` 等（**当前未解析未消费**）
- **resolve_price 回退链** (db.rs:1147 calc_est_cost): `pricing[platform_type] > top_level > default_platform > PriceSyncSettings.fallback_*(默认 3.0 $/M)`；无价/0 价回退 fallback，保证非 0
- **estimate.rs**: `estimate_after_request` / `calc_est_cost` 走 resolve_price（memory: pricing-resolve-single-source）
- **price_sync.rs**: fetch LiteLLM raw JSON → 按 mode(chat/completion/responses) 过滤 → upsert model_price(source="litellm") + 更新 PriceSyncSettings.last_sync_at
- **PriceSyncSettings** (models.rs:1235): `auto_sync_enabled / sync_interval_secs(86400) / last_sync_at / fallback_input_price / fallback_output_price`
- **commands** (lib.rs:2724+): `model_price_list/count/search/list_filtered/count_filtered/delete/upsert/sync + price_sync_settings_get/set + model_price_resolve`
- **前端 PricingTab.tsx** (440 行): 列表/搜索/筛选/分页/删除/upsert + 同步设置 UI（auto_sync/interval/fallback price）+ 立即同步按钮
- **import_export**: `model_price` 是 7 scope 之一 (collect/apply/container)，含 SCOPE_MODEL_PRICE
- **挂载**: mod.rs:12 `pub mod price_sync`

### max_tokens 缺口（"用于转换、传递 ai 平台"）
- `adapter/CONVERSION_TODO.md:38,67,74`: anthropic/gemini 入站 parse **未提取** max_tokens/temperature/top_p → 出站只能给默认值，客户端设置丢失
- LiteLLM JSON 的 max_input/output_tokens/context_window **存了但没读**（grep 全仓无消费点）
- gemini.rs 已有 `max_output_tokens` 映射 generationConfig；openai_completions/responses/glm 都有 max_tokens 字段 —— 出站结构就绪，缺的是"按模型上限填充/裁剪"

### Python 脚本范式 (memory: aidog-scripts-python-uv)
- 现有 Python 脚本走 PEP723 + uv，放 `~/.aidog/scripts/`（运行时生成脚本）；`scripts/statusline-golden/engine.py` 是仓内 Python 范例
- 仓内无 pyproject.toml

## Assumptions (待确认)

- GitHub JSON 放 **aidog 仓库自身**（非独立 data repo）
- "多维度" = 每个平台一个维度，Python 各自抓官方定价页/API → 合并去重
- app 端"定时同步" = 复用现有 auto_sync 定时器，改 URL 指向 GitHub raw JSON
- fallback 默认价保留（GitHub JSON 未覆盖的模型仍需兜底）

## Open Questions (Blocking / Preference)

1. **本地手动改价 (source="manual") 何去何从？** GitHub 是"唯一真实信源"——纯只读镜像，还是 GitHub 为基 + 本地 override 叠加？
2. GitHub JSON **仓库路径** + **文件结构**（单文件 vs 按平台分目录）
3. **max_tokens 字段如何消费**：仅展示，还是出站请求时按模型上限裁剪/填充？
4. model_price **表结构演进**：新增 max_tokens 列，还是全留 price_data JSON？
5. Python 脚本**仓内位置** + uv 工程结构 + MVP 覆盖哪些平台
6. **同步触发**：保留手动同步按钮 + auto interval（改 URL），还是走 GitHub release/etag？
7. import_export 的 model_price scope 命运

## Requirements (evolving)

- 移除 `price_sync.rs`（LiteLLM 单源同步）+ 相关 commands/UI
- 新增/保留模型信息模块（price + max_tokens + context），数据从 GitHub JSON 同步
- Python 脚本聚合多平台定价 → 写 GitHub 仓库 JSON
- app 定时拉取 GitHub JSON

## Out of Scope (待定)

- (待 Q&A 收敛)

## Technical Notes

- 触点文件: price_sync.rs / estimate.rs / db.rs(resolve_price,calc_est_cost,model_price CRUD) / models.rs(ModelPrice,PriceSync*) / lib.rs(73 commands 中 price 相关 10+) / PricingTab.tsx / api.ts / import_export(3 文件) / migration 003
- memory: pricing-resolve-single-source (est_cost 必走 resolve_price 回退链) / aidog-scripts-python-uv (PEP723+uv 范式)
