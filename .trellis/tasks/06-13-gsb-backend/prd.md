# GSB 后端 — 熔断器 + 智能调度 + 指标 + 集成

Parent: `06-13-group-scheduling-breaker` — Group 智能调度与熔断器。共享架构契约见 `../06-13-group-scheduling-breaker/design.md`。

## Goal

实现全局 Platform 级熔断器（三态机）+ Group 层智能调度（RoutingMode 新增 HealthAware/LeastLatency/Sticky）+ per-platform 指标（延迟 EMA/并发）+ router/proxy 集成 + 全局默认 settings + Platform/Group 字段 + 冻结 api.ts 契约。完成后：某平台连续 5xx/超时被熔断摘出候选、open_secs 后半开探测恢复；group 可选调度策略；与现有 auto_disabled 并集过滤互不冲突。

## What I already know
- 依赖：无（独立于 middleware engine）；但与 middleware 后端 child 共享 proxy.rs/models.rs → **全局后端串行**。
- 集成点 router.rs select_candidates(39)/order_load_balance(129)/select_platform(157)；proxy.rs 重试循环。
- 现有 auto_disabled 在 401/403（memory platform-retry-failover）。
- 熔断状态机/转移/候选过滤/接线/契约细节见 parent design.md（权威）。
- 调研 reference-repo-scheduling-breaker.md：熔断默认 5/30min/2，准入门在加权随机前。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| GA.1 | models/db：Platform 熔断字段 + RoutingMode 新变体 + SchedulingBreakerSettings + 迁移 | diff | 编译 + serde 往返 | P0 |
| GA.2 | scheduling 状态机 + 指标（内存）+ 有效阈值解析 | diff | 状态机/EMA 单测 | P0 |
| GA.3 | router 候选过滤(熔断∪auto_disabled) + 4 策略排序 | diff | 各策略 + 并集过滤单测 | P0 |
| GA.4 | proxy 重试循环 recordSuccess/Failure 接线 | diff | 集成测试 | P0 |
| GA.5 | commands + api.ts 契约冻结 | diff/契约 | commands 注册；api.ts 类型存在 | P0 |

## Requirements
- GR1 熔断三态机：5xx/超时 retry 耗尽计一次失败，达阈值 Open（候选踢出），open_secs 后 HalfOpen 放 half_open_max 探测，成功 Closed/失败 Open。
- GR2 配置：Platform 3 字段（0=继承）+ 全局默认 SchedulingBreakerSettings（5/1800/2，enabled=true）。
- GR3 与 auto_disabled 解耦：熔断(临时自动恢复) vs auto_disabled(401/403 永久)；候选过滤取并集；状态独立互不改。
- GR4 RoutingMode 新增 HealthAware/LeastLatency/Sticky；策略选 Group 层；全局默认 + Group 覆盖。
- GR5 指标 per-platform 延迟 EMA + 并发计数，内存。
- GR6 Sticky session 键映射 platform，失效/熔断回退正常调度。
- 不计熔断：401/403、客户端 4xx(非429)、probe。

## Acceptance Criteria
- [ ] cargo test：熔断 closed→open→half_open→closed 单测；LeastLatency 排序；熔断∪auto_disabled 并集过滤；HealthAware 摘 Open；Sticky 绑定+回退。
- [ ] 熔断与 auto_disabled 互不覆盖状态（单测）。
- [ ] cargo clippy --all-targets -- -D warnings 零警告。
- [ ] api.ts 含 Platform breaker 字段 + RoutingMode 新变体 + SchedulingBreakerSettings + schedulingApi；yarn build 类型层过。
- [ ] commands 注册。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交；熔断↔auto_disabled 分工 + 调度策略扩展落 cortex；契约冻结通知 parent。

## Out of Scope
- 前端 UI（GB）；最小连接策略；endpoint/vendor-type 级熔断；多实例/Redis。

## Technical Notes
- 改 models.rs/db.rs/router.rs/proxy.rs/lib.rs/api.ts + 新增 scheduling.rs（或并入 router）。
- **全局后端串行**：开工前确认无其他后端 child 在改 proxy.rs/models.rs，并把最新 master 合入本 worktree。
- 验证：cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cd .. && yarn build。
