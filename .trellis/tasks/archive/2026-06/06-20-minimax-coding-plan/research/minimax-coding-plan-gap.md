# Research: MiniMax coding plan 配额查询缺口

- **Query**: 确认 aidog 中 MiniMax「coding plan 配额查询」的确切缺口 + 补齐触点
- **Scope**: 内部为主（quota.rs / Platforms.tsx / git 历史）+ 外部 endpoint 标注「需联网核实」
- **Date**: 2026-06-20

## 结论速览

MiniMax **两者皆缺，但缺得不对称**：

- **后端 quota 查询：完全缺**。`query_quota_inner`（`quota.rs:463`）无任何 MiniMax 分支，base_url 含 `minimaxi.com` / `minimax.io` 时落到末尾 `err_quota("Unsupported platform for quota query")`（`quota.rs:503`）。
- **前端展示链路：已通用，无需新增展示组件**。command / api / `computeQuotaDisplay` 全协议无关，只要后端返回 `coding_plan` 即自动渲染。
- **前端 discoverability：缺一个 `codingPlan: true` 下拉变体 + endpoints 的 `coding_plan: true` 标记**（当前 MiniMax 预设两端点都没标 coding_plan，见 `Platforms.tsx:193-196`）。
- 🔴 **历史关键**：曾有完整 `query_minimax_coding_plan` 实现，7 天前 commit `a25b042 fix(quota): remove MiniMax coding plan (unsupported)` 整段删除。**补齐 = 复活该实现 + 核实当时为何判 unsupported**。

---

## Findings

### Files Found

| File Path | 角色 |
|---|---|
| `src-tauri/src/gateway/quota.rs` | 配额查询服务；MiniMax 分支已被删，现无 |
| `src/pages/Platforms.tsx` | 平台预设 `PROTOCOLS` / `getDefaultEndpoints` / coding plan 卡片展示 |
| `src/services/api.ts` | `quotaApi.query` 封装 + `PlatformQuota` 类型（协议无关） |
| `src-tauri/src/lib.rs` | `platform_query_quota` command（`:2366`，协议无关，已注册 `:4348`） |
| `src-tauri/src/gateway/usage_color.rs` | `cycle_ms_for_tier` tier→周期映射（tier name 白名单源） |
| `src-tauri/src/gateway/models.rs` | `Protocol::MiniMax`(`:30`) / `MiniMaxEn`(`:32`) 枚举变体（已存在） |

### 1. MiniMax 现有 quota 实现现状

**无任何 MiniMax quota 分支。** `query_quota_inner`（`quota.rs:463-504`）的 coding plan 段（`:470-500`）只覆盖 kimi / bigmodel / z.ai，余额段覆盖 deepseek / stepfun / siliconflow / openrouter / novita。MiniMax base_url（`api.minimaxi.com` / `api.minimax.io`）不命中任何 `url.contains(...)`，直接落 `err_quota("Unsupported platform for quota query")`（`quota.rs:503`）。

模块顶部 doc 注释（`quota.rs:5`）也只写 `Coding Plan: Kimi, GLM (智谱)`——MiniMax 已从列表移除。

**git 历史证据**：

- `a25b042`（2026-06-13）`fix(quota): remove MiniMax coding plan (unsupported)`：删除 64 行，含完整 `query_minimax_coding_plan` 函数 + `query_quota` 里两行分派（`api.minimaxi.com` / `api.minimax.io`）。
- `32e01d8`（2026-06-10）`fix(proxy): MiniMax deprecated path + coding_plan protocol mismatch`：早期 MiniMax 协议/路径修复。
- `fababdd` `feat(quota): add platform balance & coding plan quota query service`：MiniMax coding plan 最初随 quota 服务一起引入。

### 2. 被删除的 MiniMax 实现（直接复活参照，来自 `git show a25b042`）

被删的 `query_minimax_coding_plan(db, api_key, is_cn)` 完整骨架（**这是 MiniMax 自己的一手实现，不用抄 Kimi/GLM**）：

```rust
// GET https://{domain}/v1/api/openplatform/coding_plan/remains
//   domain = is_cn ? "api.minimaxi.com" : "api.minimax.io"
// Header: Authorization: Bearer {api_key}  +  Content-Type: application/json
//
// 响应: { base_resp:{status_code,status_msg}, model_remains:[ {model_name, ...} ] }
//   取 model_remains 中 model_name == "general" 的项：
//   - five_hour 桶: current_interval_remaining_percent → utilization = 100 - pct
//                   resets_at = end_time(millis) → ISO8601
//   - weekly 桶 (仅 current_weekly_status == 1):
//                   current_weekly_remaining_percent → utilization = 100 - pct
//                   resets_at = weekly_end_time(millis) → ISO8601
// 两桶 limit/remaining 均 None（MiniMax 只暴露百分比，无绝对量 → 方案 B 拟合）
```

返回 `CodingPlanInfo { tiers, level: None }`，tier name 用 `"five_hour"` / `"weekly_limit"`（均 ∈ `cycle_ms_for_tier` 白名单，合规）。

**与现有模板对照（作抄写参照）**：

| 实现 | URL | 鉴权 | client/解析特点 | file:line |
|---|---|---|---|---|
| `query_kimi_coding_plan` | `api.kimi.com/coding/v1/usages` | `Bearer {key}` | `limits[].detail`→five_hour，`usage`→weekly_limit；**带绝对 limit/remaining** | `quota.rs:328` |
| `query_zhipu_coding_plan` | `{open.bigmodel.cn\|api.z.ai}/api/monitor/usage/quota/limit` | 🔴 **裸 key 无 Bearer** + Content-Type | 按 `unit` 分类(3→5h,6→weekly)，TIME_LIMIT→mcp_monthly；无绝对 limit | `quota.rs:372` |
| ~~`query_minimax_coding_plan`~~（已删） | `{api.minimaxi.com\|api.minimax.io}/v1/api/openplatform/coding_plan/remains` | `Bearer {key}` + Content-Type | `model_remains[model_name=="general"]` 取百分比；无绝对量 | （a25b042 删除前 `quota.rs:367` 附近） |

> 注意：被删实现裸用 `http_client(db).get(...)` 手写，**未走 `quota_get_json`**（`quota.rs:123`，统一 GET + proxy_log 落库）。复活时应改写为 `quota_get_json` 以保持出站日志一致（见 memory `platform-egress-http-logging`）。

### 3. MiniMax coding plan 官方接口信息缺口

🔴 **需联网核实 / 需用户提供**：被删代码用的 endpoint `GET /v1/api/openplatform/coding_plan/remains` 在删除时被标 **"unsupported"**，原因 commit message 未明说。可能原因（推测，未核实）：

- 推测: 该 endpoint 已下线 / 返回 404 / 改路径。
- 推测: MiniMax 编程套餐（M2/M3 系列）的用量查询 API 当时未对外开放或需特殊权限。
- 推测: 仅国内 `api.minimaxi.com` 支持、国际 `api.minimax.io` 不支持，致 is_cn=false 路径恒失败。

**已知线索（来自被删代码，可作核实起点）**：

- 候选 endpoint：`https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains`（国内）/ `https://api.minimax.io/...`（国际）
- 鉴权：`Authorization: Bearer {api_key}`
- 响应骨架：`base_resp.status_code`（0=成功）+ `model_remains[].{model_name, current_interval_remaining_percent, end_time, current_weekly_status, current_weekly_remaining_percent, weekly_end_time}`
- 官方文档站：`platform.minimaxi.com/document`（国内）/ MiniMax io 国际文档

**本次未能联网核实**：调研环境 WebSearch/Exa 工具不可用，curl 探测受沙箱网络限制（`api.minimaxi.com/v1/query/coding_plan` 返回 404 page not found，但这非正确路径，不构成结论）。**实施前必须由 main/用户联网确认该 endpoint 现状 + 抓一次真实响应核对字段名**，禁直接复活后假定可用。

### 4. 补齐触点清单

**后端（必改，缺口主体）**：

1. `src-tauri/src/gateway/quota.rs` — 复活 `query_minimax_coding_plan(db, api_key, is_cn)`，但用 `quota_get_json`（`:123`）重写出站，失败走 `err_quota_platform("minimax", &e)`（`:104`）。参照 a25b042 删除内容 + §2 表。
2. `src-tauri/src/gateway/quota.rs:470` 附近（coding plan 段，**优先于余额段**）— 加分派：
   ```rust
   if url.contains("api.minimaxi.com") { return query_minimax_coding_plan(db, api_key, true).await; }
   if url.contains("api.minimax.io")  { return query_minimax_coding_plan(db, api_key, false).await; }
   ```
3. `src-tauri/src/gateway/quota.rs:5` — doc 注释加回 `MiniMax`。
4. tier name `five_hour` / `weekly_limit` 已 ∈ `cycle_ms_for_tier`（`usage_color.rs:30`），**无需改 usage_color**。

**前端（discoverability + endpoint 标记）**：

5. `src/pages/Platforms.tsx:30-31` `PROTOCOLS` — 加 MiniMax coding plan 下拉变体（参照 kimi `:29` / glm `:26`）：
   ```ts
   { value: "minimax", label: "MiniMax Coding Plan", codingPlan: true, keywords: ["海螺编程","minimax coding"] },
   ```
   不加 = 用户下拉里选不到 coding plan 版（软失败无报错）。
6. `src/pages/Platforms.tsx:193-200` `getDefaultEndpoints` 的 minimax/minimax_en — 现两端点**无 `coding_plan: true`**。若走「同 protocol 值靠 codingPlan 拆 base_url」模式，需改造为 `cp ? coding-base : normal-base` 并对 coding 端点标 `coding_plan: true`（参照 glm `:176-177` / kimi `:188`）。⚠️ **coding base_url 待 §3 联网核实后确定**。

**展示链路（已通用，0 触点，仅供确认）**：

7. command `platform_query_quota`（`lib.rs:2366`，已注册 `lib.rs:4348`）— base_url 泛化，**无需改**。
8. `quotaApi.query`（`api.ts:1397-1399`）+ `PlatformQuota` 类型（`api.ts`）— **无需改**。
9. `computeQuotaDisplay`（`Platforms.tsx:1148`）+ coding plan 卡片渲染（消费 `q.coding_plan.tiers` `:1180` / `est_coding_plan` `:1151`）— 协议无关，后端一返回 `coding_plan` 即渲染，**无需改**。
10. 落库：command 层 `persist_quota_to_db` 走 `calibrate_from_quota`/`build_calibrated_coding_plan`（estimate.rs），手写函数只要返回正确 `PlatformQuota` 即可，**无需碰落库链**。

### 5. 避坑标注

- 🔴 **client_type 白名单（协议 ≠ 身份，memory `coding-plan-client-type-whitelist`）**：MiniMax coding 端点的 `client_type` 须匹配上游身份白名单。当前普通预设 openai 端点用 `codex_tui`、anthropic 端点用 `claude_code`（`Platforms.tsx:194-195`）。coding plan 上游若只接某一身份，须按上游实际白名单填，**勿按 wire 协议想当然**。配 coding endpoint 前需核实 MiniMax coding plan 上游接受哪种 client_type。
- 🔴 **UI 折叠态展示（memory `coding-plan-header-display`）**：「coding plan 平台折叠状态看不到配额」是 UI 条件渲染问题，**非数据链路缺失**。若复活后端后配额仍不显，先查卡片折叠态条件渲染，别误判后端没返回。
- **tier name 白名单（memory，§4.4）**：`five_hour`/`weekly_limit` 合规；若新增其它周期语义须先在 `cycle_ms_for_tier`（`usage_color.rs:30`）加映射，否则配色退 Neutral。
- **死代码 adapter**：`adapter/minimax.rs` 是 `#[allow(dead_code)]` 死代码，**与本任务无关，禁去改**（memory `aidog-add-platform-skill`）。
- **出站日志一致性**：复活实现务必走 `quota_get_json` 而非裸 `http_client`，否则该次查询不落 proxy_log（memory `platform-egress-http-logging`）。

## Caveats / Not Found

- ⚠️ **MiniMax coding plan endpoint 现状未联网核实**：被删代码标 "unsupported" 原因不明（commit 未解释），实施前必须联网确认 endpoint 是否仍可用 + 真实响应字段名，否则复活后大概率仍失败。本环境无 WebSearch/Exa 工具，curl 受网络限制无法验证。
- coding base_url（若需拆 coding/normal 端点）未知，依赖上一条核实结果。
- MiniMax coding plan 上游接受的 client_type 身份白名单未核实。
