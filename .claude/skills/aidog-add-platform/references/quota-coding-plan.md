# 余额 / coding plan 配额查询（registry 数据驱动脚本）

校对于 2026-10-02。quota 查询不再写 Rust 函数：脚本住 **registry `platform.json`**，脚本解析 / 回落链在
`aidog_db/src/registry.rs`（`resolve_quota_script` / `materialize_quota_script` / `quota_scripts_in`），
执行引擎是 boa JS（`aidog_adapter/src/quota/script.rs` 的 `http.get` / `http.post` native 函数）。
唯二留在代码里的特例：newapi / devin（`aidog_core/src/platform_cmd/quota.rs` + `aidog_adapter/src/{newapi,devin}/quota.rs`）。

## 1. 两个字段

```jsonc
// platform.json 顶层
"quota_url_match": ["api.deepseek.com"],      // base_url 小写子串分派关键词（数据驱动，禁代码硬编码）
"quota_scripts": [
  {
    "id": "default",
    "name": { "en-US": "Balance Query", "zh-Hans": "余额查询", /* 8 locale */ },
    "requires": [],                            // 需要的 ctx 字段（如 ["apiKey"]）
    "returns": { "balance": true, "coding_plan": false, "mcp": false, "tiers": [] },
    "script": "<JS 语法脚本正文>"
  }
]
```

## 2. 脚本写法（照 deepseek 模板改）

模板：`src-tauri/defaults/registry/platforms/deepseek/platform.json` 的 `quota_scripts[0]`（余额）、
glm_coding / kimi_coding 的 coding plan 变体。要点：

- JS 语法（非 TS），顶层隐式 return 结果对象：`{ success, error, balance?, coding_plan? }`。
- 可用：`http.get(url, headersObj)` / `http.post(url, body, headersObj)`（失败 throw，catch 后
  `{ success: false, error }` 返回）、`ctx.apiKey`（用户 key）等 `requires` 声明的 ctx 字段。
- `balance` → `{ remaining, total, used, currency, is_valid }`（数字 / "CNY" 等 / bool）。
- `coding_plan` → `{ tiers: [{ name, usage, limit, reset_at? }], level? }`。
  🔴 tier `name` 必须 ∈ `cycle_ms_for_tier` 已知集合（`aidog_core/src/gateway/usage_color.rs:29`，
  `{"five_hour","weekly_limit","seven_day","mcp_monthly"}`），否则 statusline 配色退 Neutral。
- 数值清洗自带兜底（string → Number，NaN → 0）。
- 多变体（同平台多查询形态）：数组多条 + 各自 `id`，用户 `platform.extra.quota_script_id` 选中；
  缺省 / id 失效回落**首条**（`select_quota_variant`）。

## 3. 分派与回落链（0 代码改动）

- **分派**：`registry.rs::quota_code_for_base_url` 读各协议 `quota_url_match` 小写子串匹配，命中的
  首个协议 code。同族多协议共享关键词时 base 变体排前（serde Map 按协议名序）。
- **执行脚本解析**（`resolve_quota_script` 回落链，顺序不可换）：
  ① 平台 `quota_script` 物化列非空（用户保存时固化，远程同步不换）→
  ② `platform.extra.quota_custom_script` 用户手写 →
  ③ registry 变体（`extra.quota_script_id` → 首条，零配置开箱即用）。
- **command / 前端已泛化**：`platform_query_quota` + `quotaApi.query` 按平台读脚本执行，无需改。

## 4. 无上游 quota API 的平台

`manual_budgets`（platform 列，JSON）本地限额兜底，与请求驱动预估并行——不写脚本，不填 quota 字段。

## 5. 验证

- `node scripts/check-registry.mjs`（结构门禁）+ 盖戳脚本。
- 冒烟：应用里给该平台配 key → 余额/配额卡片真实查询一次（脚本 bug 静默：`success:false` 只显示
  error 文案，不炸 UI）。
