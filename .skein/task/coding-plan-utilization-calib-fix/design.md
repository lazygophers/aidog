# coding plan 套餐用量校准失效修复 — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json):

## 数据流 (修复后)

请求完成 → forward 已算出 `target_base_url`(endpoint 真 base_url) + `coding_plan` flag
  → finish_nonstream/finish_stream **新增接 endpoint base_url 参数**
  → spawn_estimate / StreamEstCtx 用 endpoint base_url(非平台级空 base_url)
  → estimate_after_request 命中阈值 → run_calibration → query_quota(endpoint base_url)
  → dispatch 子串命中(A 修好传参 + C 补 4 平台 handler) → 真查上游 tiers
  → calibrate_from_quota 写 est_coding_plan
  → 前端 computeQuotaDisplay 解析 → PlatformCard 展示 tiers ✓

## A — finish 传 endpoint base_url (根因修复)

- `forward.rs`：调 `finish_nonstream` / `finish_stream` 处已把 `coding_plan` 传入；同法把 `target_base_url`(已在 scope, forward.rs:74-76) 一并传入。
- `finish.rs`：两函数签名各加一参 `quota_base_url: String`（或 `&str`）；
  - `finish_nonstream`: `spawn_estimate(... route.platform.base_url.clone() ...)` → 改用新参（:119）。
  - `finish_stream`: `StreamEstCtx { base_url: route.platform.base_url.clone() ...}` → 改用新参（:219）。
- **取舍**：不改 `RouteResult`（endpoint 解析在 forward 内，改结构体波及面大）；只加 finish 参数，与 `coding_plan` 参数同 idiom，最小 diff。
- **回退兜底**：endpoint base_url 空时（OAuth 未回填/手建漏配）forward.rs:80 已有 base_url 空 guard 提前 skip，不会走到 finish；故 finish 收到的必非空。仍保留：若新参空串则回退 `route.platform.base_url`（防御，与现状等价）。

## B — preset 补 coding_plan flag

- `defaults/platform-presets.json`：`bailian_coding` + `compshare_coding` 的 anthropic 端点对象补 `"coding_plan": true`。
- 手维护真值源，禁机器生成覆盖（CLAUDE.md 约束）。
- 需同步更新 `last_updated` Unix 秒。

## C — query_quota 补 4 平台 dispatch handler

- **research-gate（用户裁定 2026-07-18）**：s3 调研先盘清 4 平台(bailian/qianfan/xiaomi/compshare)上游 coding plan 用量查询 API（endpoint / auth header / response schema / tier 字段映射）→ 落 research/。**s3 done 后 main 读结论逐个裁定** s4-s7：有公开 API → 保留该 subtask 建 handler，产出 `CodingPlanInfo{tiers[], level}` 解析契约；无公开 API → 该 subtask 降级为 `需要:` 标记报告（不建 handler，留 custom-quota-script 兜底）。禁未见 research 结论先写 handler。
- **实现**：仿 `quota/coding_plan.rs` 既有 `query_zhipu_coding_plan` / `query_kimi_coding_plan` 范式，各加一 handler：
  - `quota/mod.rs::query_quota_inner` 补子串分派：`coding.dashscope.aliyuncs.com` / `qianfan.baidubce.com` / `token-plan-cn.xiaomimimo.com` / `cp.compshare.cn`。
  - 各 handler 解析上游返回 → `CodingPlanInfo`（复用现有 struct + `calibrate_from_quota` 下游，零改）。
- **取舍**：每个 handler 独立、彼此不依赖 → 契约(research)一 done 即 4 路并行。
- **风险降级**：某平台上游无用量 API → 该 handler 标 `需要:` 报告，返 `err_quota`，不阻塞其余 3 个（该平台留待 custom-quota-script 自定义脚本兜底）。

## 前端展示

- 无需改码：`est_coding_plan` 一旦填充，`computeQuotaDisplay`(health.ts:44) + `PlatformCard` tiers 分支已能展示。展示缺失是 A/C 的下游表象，A/C 修好即恢复。
- 验收：glm_coding/kimi_coding 请求后真查 → PlatformCard 显示 tiers（e2e 验证，非改码）。

## 不变量 (契约)

1. coding plan 平台 `est_balance_remaining` 恒 0，禁按 $ 扣（B 修复保证走 `apply_coding_plan_delta`）。
2. finish 新增 base_url 参数与 `coding_plan` 参数同 idiom，禁改 `RouteResult` 结构。
3. C 各 handler 复用 `CodingPlanInfo` + `calibrate_from_quota`，禁改校准下游签名。
4. preset 改动手维护 + 同步 `last_updated`，禁机器生成覆盖。
5. 某平台无上游用量 API → 降级标 `需要:`，禁伪造 tiers 数据。
