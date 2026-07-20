# PRD — deep-audit-optimize

## 目标
对 aidog 项目(Tauri 2 + React 19 + Rust workspace)做审计驱动的深度优化。4 维度(代码质量 / 性能 / Rust↔TS 边界 / 架构)只读调研已产出 41 条发现,经 grill 收敛后挑 19 条进本 task 独立 Agent 修复。每条发现 = 一个独立 exec Agent(用户硬约束)。

## 用户价值
- 清理 workspace 拆分重构遗留(179 处 `#[allow(unused_imports)]` 死 import / import 模板重复 / CLAUDE.md 路径漂移)
- 降代理延迟(reqwest 共享 client 复用 TLS / tray 菜单重建防抖 / 路由 JSON 重复解析 / upsert 重复计算)
- 收敛重复(pad/clamp/ANSI_RE/Modal 基元/formatCost/relativeTime)
- 复杂度拆分(PlatformCard 914 行 / select_candidates 232 行 / model_test 272 行)
- 强类型(catch unknown / updateField 类型化 / Mutex 防毒 / eslint-disable 审 / 边界类型补)

## 边界
- 前端 `src/**/*.{ts,tsx}` + Rust `src-tauri/crates/**/*.rs` + `CLAUDE.md`
- 向后兼容优先:禁改公开 API / serde 字段名 / Tauri command 签名 / 设置 JSON 格式

## 非目标
- D 批(类型契约收紧 6 条):各派生独立 task 先 planning,不在本 task exec
- E 批(架构定调 5 条):各派生独立 task(架构决策专项),不在本 task exec
- 砍除项:perf F8(Protocol 小写化)/ perf F12(to_string trim)/ quality F13(Mutex unwrap,原 D4)/ boundary F1(ProxyLogDetail latent)/ boundary F3(Group sort_order latent)— 收益微小或无消费点
- 非目标:重写 / 破坏式重构 / 改公开契约

## 验收基准
- 每 subtask 跑门禁零回归:
  - Rust:`cargo clippy`(0 warning)+ `cargo test`
  - 前端:`yarn build` + `yarn test`(前端有 10 个 .test.ts,CLAUDE.md「前端无测试」过期)+ `scripts/check-i18n.mjs`
- C1 PlatformCard 拆分:公开 props 与导出面 12 处 import 点零影响,行为等价靠 C1a 快照测锁定
- A4 清 allow unused:逐 crate `cargo check` 通过,无 cascade 炸隐藏依赖
- 全部完成后 `skein.py board` 状态正确,findings.md 对应条目标 ✅

## 参考
- 发现详情:`findings.md` + `research/{code-quality,perf,boundary,architecture}.md`
- 契约:9 条(见 task.json,`skein.py contract deep-audit-optimize` 核对)
