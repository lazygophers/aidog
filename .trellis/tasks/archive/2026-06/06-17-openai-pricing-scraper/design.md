# Design — OpenAI scraper + 阶梯计费

## 数据流
developers.openai.com/api/docs/pricing (HTML, Astro JSON island)
  → openai.py fetch() 解析 rows[]
  → ModelEntry{top=short, context_tiers=[{272000, long}], pricing["openai"]=short}
  → aggregate.py (openai 排 openrouter 前, top-level 先到先得 → openai 胜)
  → data/models.json (不提交)
  → price_sync.rs sync_github_prices (entry verbatim → price_data blob)
  → resolve_price(input_tokens) 选档 → calc_est_cost

## Astro island 解析
pricing HTML 内嵌: `"rows":[1,[[1,[[0,"gpt-5.5 (<272K context length)"],[0,5],[0,0.5],[0,30]]],[1,[[0,"gpt-5.5-pro (<272K context length)"],[0,30],...]]]]`
编码: `[typeTag,value]`, typeTag 0=string/number literal。每行 = [model名, input, cached, output]。Standard table short-context 列在 rows[0]; long-context 列追加同模型第二行 (名含 ">272K" 或 long 标记)。

解析策略: 用正则定位 `"rows":\[...\]` JSON 片段 → `json.loads` (需 HTML-unescape &quot;) → 遍历行, 按模型名聚合 short/long。阈值从模型名 "(<NNNK context length)" 提取 (272K→272000)。

## resolve_price 阶梯算法 (db.rs)
```
fn resolve_price(db, model, platform, fb_in, fb_out, input_tokens: i64):
    # 现有 4 级回退解出 base (in/out/cache)
    ...
    # 新增: tier 选档
    let tiers = pd.get("context_tiers")  # array
    if let Some(best) = tiers.filter(min_tokens <= input_tokens).max_by(min_tokens):
        if best.input.is_some(): base.in = best.input
        ...  # 非 null 覆盖
        source += "+tier"
    return ResolvedPrice{...}
```

## 调用点改动 (全部已有 input_tokens)
| 位置 | 改动 |
|---|---|
| db.rs:1182 calc_est_cost | resolve_price(..., input_tokens as i64) |
| estimate.rs:434 | resolve_price(db,m,p,0,0, input_tokens) |
| estimate.rs:451 | resolve_price(db,m,p,0,0, input_tokens) |
| lib.rs:2871 model_price_resolve | 加 input_tokens: Option<i64>=None → 0 |

## 阈值语义
`min_tokens`: 该档适用于 input_tokens >= min_tokens。OpenAI "<272K context" = short 档 (< 272000), long 档 ≥ 272000 → tier.min_tokens=272000。

## 容错
- scraper: 解析失败/网络异常 → raise (aggregate 捕获 → "✗ openai: FAIL", 保留旧值)。禁返 {}。
- resolve_price: context_tiers 缺失/非数组 → 跳过 tier (取 base)。向后兼容旧 price_data。
