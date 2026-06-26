# router 同优先级按过期时间最早调度

## 背景

用户原话 (2026-06-26): 「同优先级的情况下，优先使用过期时间最早的」+「优先调度最旧的」
语义确认 (AskUserQuestion): 「最旧」= 过期时间最早 (expires_at 升序), 非创建/更新时间

## 需求

group 平台调度: 同 priority 候选平台中, 按 `expires_at` 升序优先选 (快过期优先用, 避免额度浪费)。

## 现状 (待 agent 研究完整链路)

- `src-tauri/src/gateway/models/platform.rs:187` `Platform.expires_at: i64` (毫秒 unix, 0 = 永不过期); `>0 且 now>=expires_at` 路由 candidate_state (memory [[platform-retry-failover]])
- `src-tauri/src/gateway/models/group.rs:104` group `priority: i32`; `level_priority: i32`; 关联表 `(platform_id, priority, weight)`
- platform select 链路: proxy/handler.rs / endpoint.rs (memory [[group-scheduling-breaker]] 三态机 + 4 策略 + breaker ∪ auto_disabled 过滤)
- 现有 tiebreaker 顺序待 agent 查清 (priority → ? → weight 随机?)

## 边界决策 (main 定)

1. **expires_at=0 (永不过期)**: 排最后, 视为 `i64::MAX` (不优先于有期限平台)
2. **已过期 (now >= expires_at)**: 不参与 "优先调度" — 走 candidate_state 过滤 (memory [[platform-retry-failover]]), 不在本需求范围 (本需求只排未过期候选)
3. **tiebreaker 插入位置**: priority 主序不变; **同 priority 内**, expires_at 升序插在现有 tiebreaker (attempts/breaker/weight) **之前** — 过期时间是最强 "用掉它" 信号, 优先于负载均衡
4. **expires_at 相同**: 落到现有 tiebreaker (attempts/breaker/weight) 决定

## 验收

- 同 priority 候选: expires_at 最小 (非 0、未过期) 的平台优先调度
- expires_at=0 平台在同 priority 内排所有有期限平台之后
- 已过期平台不因本需求被优先 (仍走 candidate_state 过滤)
- 单测覆盖: 同 priority 多候选 (混合 0/未过期/同值) 排序断言
- cargo test/clippy 0 warning, yarn build/check-i18n 全绿

## 风险

- 调度链路分散 (handler/endpoint/group/breaker 多层), expires_at 排序须插对层 (候选过滤后、weight 抽样前)
- 若现有用加权随机 (weight) 而非确定排序, 改成 expires_at 确定优先会改变调度语义 — agent 须说明现有机制 + 改动影响
- level_priority vs priority 两层优先级: expires_at 在哪层内排 (推测 level_priority→priority→expires_at)
