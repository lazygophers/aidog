# coding plan 套餐用量校准失效修复 — 调研收敛

## 根因链路 (已代码实证)

coding plan 平台的套餐用量 = `est_coding_plan` JSON tiers (utilization%), **非** `est_balance_remaining`（$ 余额恒 0，订阅制设计如此，`estimate/db_ops.rs:138-142` 强制置 0）。用量靠**真查上游 API 校准**填充，链路：

1. 请求完成 → `finish_nonstream` / `finish_stream`（`gateway/proxy/finish.rs`）调 `spawn_estimate` / 构 `StreamEstCtx`。
2. `spawn_estimate` → `estimate_after_request`（`estimate/db_ops.rs:180`）→ `is_coding_plan` 分叉：`apply_coding_plan_delta`（累加 token 拟合 utilization）+ 命中阈值触发 `run_calibration` → `query_quota`（真查上游）→ `calibrate_from_quota` → 写 `est_coding_plan`。
3. 前端 `PlatformCard` → `computeQuotaDisplay`（`domains/platforms/health.ts:44`）解析 `est_coding_plan` JSON → tiers；空串 → `hasData=false` → **列表不展示**（`PlatformCard.tsx:148`）。

## 三层缺口 (代码 + preset 实证)

### A — finish 传平台级空 base_url (根因, 全平台)
- `finish.rs:119`（非流）+ `finish.rs:219`（流 `StreamEstCtx`）均传 `route.platform.base_url.clone()`。
- **全部 coding plan 平台 preset 平台级 `base_url=None`**（真 base_url 在 endpoints 内）：glm_coding/kimi_coding/bailian_coding/compshare_coding/qianfan_coding/xiaomi_mimo_coding 均如此。
- → `query_quota` 收到空 base_url → dispatch 子串匹配失败 → 返 `Unsupported platform`（`quota/mod.rs:105`）→ `calibrate_from_quota` early-return（success=false 不写）→ `est_coding_plan` **永不填充** → tiers 空白。
- **endpoint 真 base_url 在 `forward.rs:74-76` 已算出为 `target_base_url`，与 `coding_plan` flag 同源同 scope**；`coding_plan` 已传入 finish（`finish.rs:126`），`target_base_url` 却没传。**修复 = finish 同法传 endpoint base_url。**

### B — endpoint 漏 coding_plan flag (误扣 $, bailian+compshare)
- preset 端点级 `coding_plan` flag：`bailian_coding`(anthropic ep) + `compshare_coding`(anthropic ep) 为 `null`（其余平台端点均 `true`）。
- → `forward.rs:76` 得 `coding_plan=false` → `estimate_after_request` 走 `apply_balance_delta` 从 `est_balance_remaining`(初值 0) **扣成负数**（本应走 `apply_coding_plan_delta`）。
- 修复 = preset 两端点补 `"coding_plan": true`（单文件）。

### C — query_quota dispatch 不支持 4 平台 (无法校准)
- `query_quota_inner`（`quota/mod.rs:52-107`）按 base_url 子串分派，**仅支持** kimi(`api.kimi.com/coding`)/glm(`open.bigmodel.cn`/`api.z.ai`)/minimax(`api.minimaxi.com`/`api.minimax.io`) 的 coding plan 真查。
- **不支持**：bailian(`coding.dashscope.aliyuncs.com`)、compshare(`cp.compshare.cn`)、qianfan(`qianfan.baidubce.com`)、xiaomi(`token-plan-cn.xiaomimimo.com`)。即使 A 修好，这 4 平台仍返 Unsupported → 无 tiers。
- 修复 = 各补上游 coding plan 用量查询 handler（需逆向各上游 API，见 research/）。**风险**：部分上游可能无公开用量 API → 该平台标 `需要:` 降级。

## 与 custom-quota-script task 的边界
- custom-quota-script = 给**无内置支持**平台补自定义 HTTP 模板 + 外部脚本查询（扩展新数据源入口）。
- 本 task = 修**既有内置** coding plan 的 dispatch/flag/传参缺口 + 给 4 平台补**内置** handler。用户裁定 C 走本 task 内置 handler（非自定义脚本）。**不同层，不并入。**

## 计费语义 (SPEC 锚)
coding plan 平台余额语义 = utilization tiers（`est_coding_plan`），`est_balance_remaining` 恒 0；余额扣减链路（`estimate_after_request`）与 `proxy_log.est_cost` 是两套独立计算，前者按端点级 `coding_plan` flag（非协议级 `is_coding_plan`）分叉。
