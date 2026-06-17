# Research: 小米 MiMo 开放平台 coding plan 配额查询 API

- **Query**: 调研「小米」LLM 开放平台 API 规格，重点 coding plan（订阅制编程套餐）配额查询接口，供 aidog 接入
- **Scope**: mixed（外部官方文档 + 接口实探 + 内部代码现状）
- **Date**: 2026-06-17

## 一句话结论

小米**确实有** coding plan 形态的订阅套餐（名为 **Token Plan**，面向 AI 编程场景），但**未找到任何用 API Key 鉴权的公开配额查询接口**：配额只能在登录态 Web 控制台查看，后端用量接口 `/api/v1/usage` 走小米账号 SSO Cookie 鉴权，**拒绝 `tp-`/`sk-` API Key**。因此无法照 Kimi/GLM 模式直接接入。`需要: 用户提供登录态控制台 /api/v1/usage 的抓包样本（请求头 + 响应 JSON），否则只能走 manual_budget 月度额度兜底`。

---

## 1. 平台基本规格

| 项 | 值 | 来源 |
|---|---|---|
| 确切产品名 | **Xiaomi MiMo API 开放平台**（Xiaomi MiMo API Open Platform） | platform.xiaomimimo.com `<title>` + llms.txt |
| 官方站点 | `https://platform.xiaomimimo.com` | 实探 200 |
| 官方文档（llms 索引） | `https://platform.xiaomimimo.com/llms.txt` | 实探 |
| 控制台 | `https://platform.xiaomimimo.com/#/console/usage`、`/#/console/api-keys`、`/#/console/plan-manage` | first-api-call.md / quick-access.md |
| 账号体系 | 仅小米账号登录（id.mi.com / account.xiaomi.com SSO） | first-api-call.md |

### 1.1 按量付费（Pay-As-You-Go）端点

- OpenAI 兼容: `https://api.xiaomimimo.com/v1`（chat 全路径 `…/v1/chat/completions`）
- Anthropic 兼容: `https://api.xiaomimimo.com/anthropic`（chat 全路径 `…/anthropic/v1/messages`）
- API Key 格式: `sk-xxxxx`
- 来源: `static/docs/quick-start/first-api-call.md`

### 1.2 Token Plan（订阅制 = coding plan）端点

按集群分三地，OpenAI / Anthropic 双协议（来源 `static/docs/price/tokenplan/quick-access.md`）：

| 集群 | OpenAI base_url | Anthropic base_url |
|---|---|---|
| 中国 (cn) | `https://token-plan-cn.xiaomimimo.com/v1` | `https://token-plan-cn.xiaomimimo.com/anthropic` |
| 新加坡 (sgp) | `https://token-plan-sgp.xiaomimimo.com/v1` | `https://token-plan-sgp.xiaomimimo.com/anthropic` |
| 欧洲 (ams) | `https://token-plan-ams.xiaomimimo.com/v1` | `https://token-plan-ams.xiaomimimo.com/anthropic` |

- API Key 格式: **`tp-xxxxx`**（与按量 `sk-` 独立，不可混用）
- 实探佐证: `POST https://token-plan-cn.xiaomimimo.com/v1/chat/completions` → **405**（端点真实存在，仅接 POST）

### 1.3 鉴权方式（⚠️ 非标准 Bearer）

官方 curl/SDK 示例统一用自定义请求头：

```
api-key: $MIMO_API_KEY
Content-Type: application/json
```

- 来源: `first-api-call.md` 与 `quick-access.md` 的 curl 示例（OpenAI 与 Anthropic 两种都用 `--header "api-key: $MIMO_API_KEY"`）。
- 注意：这与 aidog 现有 `Authorization: Bearer` 默认不同。**aidog 当前预设走 anthropic 协议**（见 §3），anthropic wire 用 `x-api-key`，需核实小米 anthropic 端点是否也接受 `x-api-key`（官方示例写的是 `api-key`，但 SDK `Anthropic(base_url=…)` 默认发 `x-api-key`，二者小米侧应都兼容——SDK 示例能跑通即证 `x-api-key` 可用）。

---

## 2. coding plan / 订阅配额查询接口

### 结论：未找到 API-Key 鉴权的公开配额查询接口（NO）

小米 Token Plan 是 coding plan 形态（官方原文：「Token Plan is a dedicated subscription plan launched for **AI programming scenarios**」，兼容 Claude Code / OpenCode / OpenClaw），但**配额查询无公开 API**。

#### 套餐配额模型（来源 `static/docs/price/tokenplan/subscription.md`）

- **不是** Kimi/GLM 那种 5h 滚动窗 + 周限额，而是**月度 / 年度固定 Credits 额度**：
  - 月度 4 档: Lite 4.1B / Standard 11B / Pro 38B / Max 82B Credits（$6/$16/$50/$100 每月）
  - 年度 4 档: 49.2B / 132B / 456B / 984B Credits
- Credits 按 token 折算扣减（mimo-v2.5-pro: cache-hit 2.5 / cache-miss 300 / output 600 Credits 每 token；mimo-v2.5: 2/100/200）。
- 配额耗尽即停服，不串扣余额/赠金。
- 官方明示查配额的方式：「You can check the quota and usage of your current plan in **Subscription Management** (`/#/console/plan-manage`)」——即**只在 Web 控制台查**，文档未给任何 REST 端点。

#### 接口实探（2026-06-17，本机 curl）

| 探测 URL | 结果 | 含义 |
|---|---|---|
| `GET token-plan-cn.xiaomimimo.com/v1/usages` | 404 | 无 Kimi 式 `/v1/usages` |
| `GET token-plan-cn.xiaomimimo.com/coding/v1/usages` | （未命中 Kimi 路径，等价 404） | 无 Kimi `/coding/v1/usages` |
| `GET token-plan-cn.xiaomimimo.com/anthropic/v1/usages` | 404 | 无 anthropic 侧用量端点 |
| `GET token-plan-cn.xiaomimimo.com/v1/dashboard/billing/usage` | 404 | 无 OpenAI 式 billing/usage |
| `GET api.xiaomimimo.com/v1/usages` | 404 | — |
| `GET api.xiaomimimo.com/user/balance` | 404 | 无 DeepSeek 式余额端点 |
| `GET platform.xiaomimimo.com/api/v1/usage` | **401 + loginUrl** | 控制台用量端点存在，但走小米账号 SSO Cookie 鉴权 |

关键证据 — `/api/v1/usage` 的 401 响应体：

```json
{"code":401,"loginUrl":"https://account.xiaomi.com/pass/serviceLogin?callback=...sid=api-platform&_group=DEFAULT"}
```

- 携带 `api-key: tp-…` 或 `Authorization: Bearer tp-…` 请求 `/api/v1/usage` **仍返回 401 + loginUrl**（两种 header 均试，结果一致）。
- 即：该用量接口**只认登录态 Cookie（小米账号 SSO），不认 API Key**。无法用 aidog 持有的平台 API Key 直接查。

#### 文档侧佐证（无 quota API）

`llms.txt` 的「API Reference」仅列：Chat（OpenAI API / Anthropic API）、Audio（ASR）。**无 usage / quota / billing / balance / monitor 任何 endpoint 文档页**（已 grep `usage|quota|balance|subscription|consum|plan|monitor|credit`，命中的全是套餐说明/新闻页，非 API 页）。

---

## 3. aidog 内部现状（已部分接入）

- **平台预设已存在**（无需新增平台），痕迹：
  - `src/pages/Platforms.tsx:41` — `{ value: "xiaomi_mimo", label: "小米 MiMo", keywords: ["xiaomi","小米","mimo"] }`（注意：**未标 `codingPlan: true`**）
  - `src/pages/Platforms.tsx:220-222` — 预设单端点：`{ protocol: "anthropic", base_url: "https://api.xiaomimimo.com/anthropic", client_type: "claude_code" }`（**指向按量 host，非 token-plan host；无 coding_plan 标记**）
  - `src/pages/Platforms.tsx:421` `PROTOCOL_LABELS.xiaomi_mimo = "小米 MiMo"`；`:495` 颜色 `#FF6900`
  - `src/services/api.ts:13` TS Protocol 含 `"xiaomi_mimo"`
  - `src-tauri/src/gateway/models.rs:54-55` Rust `Protocol::XiaomiMimo` rename `"xiaomi_mimo"`
  - `src/utils/platformPaste.ts:3` 智能粘贴样例提及「小米 MIMO（双 base_url）」
- **quota.rs 无任何小米分支**：`query_quota`（`src-tauri/src/gateway/quota.rs:373`）的 base_url 子串分派链里**没有** `xiaomimimo` / `token-plan`，落空走 `err_quota("Unsupported platform for quota query")`。
- 结论：当前小米平台**只能发请求，配额栏无数据**。

---

## 4. 映射到 aidog 的接入建议

### 4.1 若坚持接 coding plan 配额展示 —— 当前不可行（缺 API）

照 Kimi/GLM 模式（`query_<plat>_coding_plan` + base_url 子串分派）需要一个 **API-Key 可访问的 usage JSON 端点**，而小米唯一的用量端点 `/api/v1/usage` 走 SSO Cookie、拒 API Key。**没有抓包样本无法实现**，且即便抓到，Cookie 鉴权也无法用 aidog 存的平台 API Key 复现（aidog 不持有用户小米账号登录态）。

### 4.2 推荐方案：manual_budget 月度额度兜底（0 上游依赖）

小米 Token Plan 是「月度固定 Credits」模型，天然契合 aidog 的 `manual_budgets`（`src-tauri/src/gateway/manual_budget.rs`，见 quota-coding-plan.md §5）：
- `kind: "fixed"` 或 `"daily"` 窗口 + `unit: "token"`，按套餐档位填月度 Credits 上限（如 Lite=4.1e9）。
- 由请求驱动预估递减，配额耗尽 402 阻断。与上游无耦合，无需改 quota.rs。
- 缺点：Credits≠原始 token（按模型有 2~600 倍折算系数），需在预估侧按模型乘折算系数才精确——可作为后续精细化项。

### 4.3 若未来拿到抓包、决定硬接（备查参数，勿凭此实现）

- **base_url 子串分派 key**: 建议 `token-plan` 与 `xiaomimimo`（覆盖三集群 cn/sgp/ams + 按量 host）。放在 quota.rs coding 段（优先于余额段）。
- **tier 命名**: 小米是**月度固定额度**，对齐现有集合应映射为 `mcp_monthly`（`usage_color.rs:30` 已知周期 30d，唯一月度语义槽位）；**无 five_hour / weekly_limit 语义**，不要硬塞。若要新建「monthly」周期语义需在 `cycle_ms_for_tier` 加映射。
- **绝对量**: 套餐是绝对 Credits 上限（limit）+ 已消耗 → `QuotaTier.limit/remaining` 应填绝对值（has_base=true，精确预估），`utilization = used/limit*100`。
- **鉴权**: 实际 usage 接口走 Cookie，**不是** `api-key` 头——这是阻断点，非实现细节。

### 4.4 顺带可修的预设瑕疵（非本调研结论，供主代理判断）

- 现有预设 base_url 是按量 host `api.xiaomimimo.com/anthropic`，**Token Plan 用户的 base_url 是 `token-plan-{cn,sgp,ams}.xiaomimimo.com/anthropic`**——若要支持订阅用户，预设需提供 coding_plan 变体端点（参考 kimi/qianfan 的 `cp ? … : …` 三元，`Platforms.tsx:213-218`）。
- `PROTOCOLS` 条目未标 `codingPlan: true`（`Platforms.tsx:41`）。

---

## Caveats / Not Found

- **未找到**：API-Key 鉴权的小米 Token Plan 配额查询接口。证据：官方 `llms.txt` API Reference 无 usage/quota 页；实探 `token-plan-cn…/v1/usages`、`/anthropic/v1/usages`、`/v1/dashboard/billing/usage`、`api.xiaomimimo.com/{v1/usages,user/balance}` 全 404；`platform.xiaomimimo.com/api/v1/usage` 存在但 401 强制小米账号 SSO 登录，带 `api-key`/`Bearer` 头无效。
- 鉴权头细节：官方文档示例用 `api-key:` 自定义头；anthropic SDK 默认发 `x-api-key`。小米 anthropic 端点对二者的具体接受情况未逐一实测（无有效 key）。**推测: 两者皆兼容**（官方同时给 SDK 与 curl 两种示例且都标"成功"）。
- `/web-api/plan/usage` 返回 200 但响应体是 SPA HTML（前端路由 fallback），**非** JSON API，不可用。
- 接口实探时间 2026-06-17，小米可能后续新增配额 API，接入前建议复查 `llms.txt`。

## External References

- [Xiaomi MiMo llms.txt（文档索引）](https://platform.xiaomimimo.com/llms.txt)
- [Token Plan Subscription Instructions](https://platform.xiaomimimo.com/static/docs/price/tokenplan/subscription.md) — 套餐档位/Credits/折算系数
- [Token Plan Quick Access](https://platform.xiaomimimo.com/static/docs/price/tokenplan/quick-access.md) — Token Plan 三集群 base_url + `tp-` key
- [First API Call](https://platform.xiaomimimo.com/static/docs/quick-start/first-api-call.md) — 按量 base_url + `api-key` 鉴权头 + SDK 示例
- [Claude Code Configuration](https://platform.xiaomimimo.com/static/docs/integration/claudecode.md) — 按量 vs Token Plan 凭证对比

## Related Internal Files

- `src-tauri/src/gateway/quota.rs:373` — `query_quota` 分派（无小米分支）
- `src/pages/Platforms.tsx:41,220,421,495` — xiaomi_mimo 平台预设
- `src-tauri/src/gateway/models.rs:54` — `Protocol::XiaomiMimo`
- `src-tauri/src/gateway/usage_color.rs:30` — `cycle_ms_for_tier` tier 周期集合
- `src-tauri/src/gateway/manual_budget.rs` — 推荐兜底机制
