# OpenAI 一手定价 scraper + 上下文阶梯计费

## 背景
`scripts/pricing/scrapers/openai.py` 现 `return {}` (空), aggregate 打印 `· openai: 空 (跳过)`。
注释称"官方页 JS 渲染无程序化路径"——**过时**。新域名 `developers.openai.com` (Astro v6 SSR) 静态可抓。

## 目标 (用户确认)
1. openai scraper **纯 live fetch** (不硬编码) 从 `developers.openai.com/api/docs/pricing` 抓权威价。
2. **支持上下文阶梯计费**: OpenAI 旗舰分 short/long context 两档 (gpt-5.5 short $5/$30, long $10/$45, 阈值 272K)。est_cost 按实际 input_tokens 选档, 非只存一档。

## 现状 (核实)
- openrouter 骨干当前**偶然匹配** OpenAI 官方 short 价 (gpt-5.5 $5/$30/$0.50 全等), 供 60 OpenAI 系 + token 上限。缺口: `chat-latest` 不在; 是第三方近似源随时漂移; **无 long-context 档**。
- `price_data` 是 raw JSON blob (price_sync.rs:55 entry verbatim 存) → schema 加字段**无需 DB 迁移**。
- `resolve_price` 4 调用点全已有 input_tokens; 仅命令 `model_price_resolve` (lib.rs:2871) 无 (加默认 0)。

## 设计
### schema 扩展 (scripts/pricing/schema.py)
```python
class ContextTier(BaseModel):
    min_tokens: int  # 该档适用于 input_tokens >= min_tokens
    input_cost_per_token: Optional[float] = None
    output_cost_per_token: Optional[float] = None
    cache_read_input_token_cost: Optional[float] = None

class ModelEntry(BaseModel):
    ...  # 现有字段不变
    context_tiers: list[ContextTier] = Field(default_factory=list)  # 新增, 可选
```
top-level 价 = short-context (基线, 向后兼容); context_tiers = [{min_tokens:272000, long 价}]。

### resolve_price 阶梯选档 (db.rs:3008)
签名加 `input_tokens: i64`。现有 4 级回退 (platform_override → top_level → default_platform → fallback) **先解出 base 价**, 再扫 `context_tiers`: 取 `min_tokens <= input_tokens` 中最大档, 非 null 字段覆盖 base。返回 ResolvedPrice (source 追加 "+tier" 标记)。

调用点改:
- `calc_est_cost` (db.rs:1182): 传 `input_tokens as i64` (已有)。
- `estimate.rs:434` (balance_cost) / `:451` (manual_budget): 传 input_tokens (已有)。
- `model_price_resolve` 命令 (lib.rs:2871): 加 `input_tokens: Option<i64>` 默认 0。

### openai scraper (scripts/pricing/scrapers/openai.py) — 纯 live
httpx GET pricing 页 → 正则/JSON 解析内嵌 Astro island `"rows"` 数组 → Standard tier table → 每模型取 short (Input/Cached/Output) + long context 列 → ModelEntry:
- top-level = short 价 ($/M ÷1e6)
- context_tiers = [{min_tokens: 解析阈值 (如 272000), long 价}] (有 long 档的模型)
- pricing["openai"] = short per-platform 价
- default_platform = "openai"
- max_output_tokens/context_window: pricing 页同源解析 (无则留 None, openrouter 合并补)
- 失败抛异常 (aggregate 跳过保留旧值), **禁静默返空**

### REGISTRY 顺序
openai 须排 openrouter **之前** (aggregate top-level 取首次非 0, 先到先得 → openai 权威胜)。

## 模型范围
flagship text (gpt-5.5/5.5-pro/5.4/5.4-mini/5.4-nano/5.4-pro) + gpt-5.3-codex + chat-latest。
跳 image/video/realtime/transcription (单位非 per-text-token, schema 不适用)。

## 验证
1. `cd scripts/pricing && uv run python aggregate.py` → `✓ openai: N 模型` (非空)。
2. 产物 gpt-5.5: top input 5e-6 + context_tiers [{272000, input 1e-5, output 4.5e-5, cache 1e-6}]。
3. Rust: `cargo test` (db/price 相关); `cargo clippy` 0 warning (block v0.1.6 除外)。
4. 单测: resolve_price 短档 (input<272K 取 short) / 长档 (input≥272K 取 long) / 无 tier 模型 (取 base)。
5. 不提交 data/models.json (CLAUDE.md)。

## 不做
- PricingTab UI 展示阶梯 (后端就绪即可; UI 单独 task, 非阻塞)。
- 其他平台 (anthropic/glm 等) 阶梯化 (本任务只 OpenAI)。
