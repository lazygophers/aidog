# perf-full-audit — 全量性能审计(前端+跨层+已修项复验)

## 目标
- [x] 复验上轮 perf-oom-cpu-optimize 7 修复仍生效无回归
- [x] 攻未覆盖维度: 非流式 body cap(对称漏) / retention 调度 / 前端集中 store / settings 缓存 / DashMap / Liquid Glass

## 用户价值
- OOM 彻底止血: 非流式 body 也 cap(上轮只 cap 流式)
- 长跑不胀库: retention 每日自动清理 + VACUUM 回收
- 前端 CPU 降: 去多页 listener 各 reload, 集中 store 精准刷新

## 边界
- [x] Rust 代理热路径 + 数据层调度 + 前端 store + 主题审计
- [x] 不改公开 API / serde 字段名 / DB schema(向后兼容)
- [x] VACUUM 后台 spawn 不阻塞主线程 + 低频
- [x] Liquid Glass: 审计 + 最小改(滚动容器嵌套), 不盲改主题架构
- [x] 不加新依赖(Zustand 若引入需用户确认 — 本轮用 AppContext 扩展或现有 state 方案)

## 非目标
- 不重构代理模块整体架构
- 不改 retention 默认天数(7/7/90)
- 不加运行时 profile 工具(无数据, 静态高置信项直接修)

## 验收标准
- [ ] `cargo clippy --all-targets` 0 新 warning(baseline 115)
- [ ] `cargo test --workspace` 通过(除 9 baseline test_headers)
- [ ] `yarn build` + `yarn test` 通过
- [ ] 非流式 body cap ≤ 16MB + record 受 log_upstream_request gate(与流式对称)
- [ ] retention run_retention_cleanup 纳入每日 spawn + VACUUM 后台低频
- [ ] 前端 proxy-log-updated 单 listener(store), 各页订阅 store 不各自 reload
- [ ] settings 每请求不走 DB 缓存读(ProxyState 持缓存)
- [ ] log_snapshots 改 DashMap(若 s1 后仍判值得)
- [ ] Liquid Glass 滚动容器审计报告 + 最小改(若需)

## 索引
- 详细设计: [design.md](design.md)
- 审计收敛: [findings.md](findings.md)
- 调度: task.json(6 subtask: s1/s2/s3/s6 独立起, s4 deps s1, s5 deps s4)
