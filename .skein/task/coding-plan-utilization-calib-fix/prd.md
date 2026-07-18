# coding plan 套餐用量校准失效修复 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] coding plan 平台(glm/kimi/minimax/bailian/compshare/qianfan/xiaomi)套餐用量 tiers(`est_coding_plan`)能被自动校准填充，并在平台列表(PlatformCard)展示 utilization。
- [x] 修根因：finish 传 endpoint 真 base_url（非平台级空 base_url）到校准链路，使 `query_quota` dispatch 命中。
- [x] 修次生 bug：bailian_coding/compshare_coding 端点漏 `coding_plan` flag 致误按 $ 扣 `est_balance_remaining` 成负数。
- [x] 补 dispatch：4 个不支持平台(bailian/compshare/qianfan/xiaomi)的内置 coding plan 用量查询 handler。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] **范围内**：A(finish 传 endpoint base_url) + B(preset 补 2 端点 flag) + C(query_quota 补 4 平台 handler)。
- [x] **范围外**：coding plan 改按 $ 扣费（订阅制设计如此，余额恒 0，不改）；自定义 HTTP 模板/外部脚本查询（归 custom-quota-script task）；前端展示逻辑改造（已能展示，仅验证）。
- [x] **已知约束**：preset 手维护禁机器覆盖 + 同步 `last_updated`；禁改 `RouteResult` 结构；C 复用 `CodingPlanInfo`+`calibrate_from_quota` 禁改下游签名；某平台上游无用量 API → 降级标 `需要:`，禁伪造数据。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [x] A：`finish_nonstream`/`finish_stream` 用 endpoint base_url 传 `spawn_estimate`/`StreamEstCtx`；dispatch 子串命中（单测/mock 验），首请即触发校准（`last_real=0` → `should_calibrate` 恒 true）。真 tiers 填充 + PlatformCard 展示 = **用户真实账号 e2e 手验**（本机无付费订阅 key）。
- [x] B：`bailian_coding`+`compshare_coding` anthropic 端点含 `coding_plan:true`；请求走 `apply_coding_plan_delta` 不再扣 `est_balance_remaining` 成负数。
- [x] C：**research-gate** — s3 调研先盘清 4 平台上游用量 API，main 按结论逐个裁定 s4-s7：有公开 API → 建 handler（`query_quota_inner` 补子串分派）+ dispatch 单测；无 API → 降级标 `需要:` 报告，返 `err_quota`，留 custom-quota-script 兜底，不阻塞其余。
- [x] `cargo build` + `cargo clippy` 零 warning；`cargo test` 绿（含 dispatch 命中单测）；`yarn build` 通过。真 tiers e2e 交用户手验，非 check 门。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list coding-plan-utilization-calib-fix`)
