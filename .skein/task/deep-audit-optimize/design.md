# Design — deep-audit-optimize

## 方法论
4 维度并行只读 researcher 调研(已完)→ 汇总 findings.md(41 条)→ grill 硬门收敛(砍 low + 派生 D/E + C1 先测后拆)→ 拆 19 exec subtask → DAG 自动编排 → 每发现独立 Agent。

## Agent 编排策略(用户硬约束:每发现 = 独立 exec Agent)
- exec Agent 类型:`aidog-bug-hunt`(Rust/边界/跨层修复)+ 通用 claude agent(前端/文档/机械重构)
- 每个 Agent 自包含 6 字段 prompt(目标/已知/工作目录范围/输出格式/验收标准/失败处理)
- Rust Agent 改完自跑 `cargo clippy + cargo test`;前端 Agent 改完自跑 `yarn build + yarn test + check:i18n`
- Agent 失败 → 回传 main,main 判:修复重派 / 降级标后续

## 向后兼容策略
- 不改公开契约(serde/命令签名/设置格式)— 内部实现重构
- C1 PlatformCard:**先补 characterization 快照测(C1a)锁定 render 行为,再拆(C1b)**,公开 props/导出 12 处 import 点零影响
- A4 清 allow unused:**逐 crate 增量**,每 crate `cargo check` 验,禁一次清完(防 cascade 暴露隐藏依赖)
- C2 select_candidates_ctx 拆分:已有 20+ 测试(test_candidates.rs)强回归网,签名不变

## 文件热点顺序化(DAG 关键)
- **PlatformCard.tsx**:A6(.toLocaleString→formatDateTime)+ B5(formatCost→formatCostUsd)+ C1b(拆 + B6 relativeTime 合并)三 Agent 抢 → 强顺序化 A6+B5 → C1a → C1b。C1b 独占 PlatformCard.tsx,A6/B5 在该文件的改动并入 C1b
- **formatters.ts**:B1(pad)/B2(clamp)加新函数,A6 改 caller 不动 formatters.ts → A6 与 B1/B2 可并行(不同文件)
- **candidates.rs**:C2(拆函数)+ C4(去重复解析)→ 顺序化 C2 → C4
- **forward.rs**:A1(共享 client)+ C6(pretty 条件化)→ 顺序化 A1 → C6
- **gateway 其他**:C5(log.rs)/A2(log.rs app_setup.rs)/B3(catalog.rs)/A4(逐 crate)独立

## 风险
1. workspace 拆分重构期路径漂移:researcher 已用实际 crate 布局,A5 修 CLAUDE.md 全量核对
2. PlatformCard 拆分最高风险:C1a 快照测锚兜底
3. A4 cascade:逐 crate 增量验
4. 前端「无测试」假设已推翻(10 个 .test.ts),验收门禁含 yarn test

## 派生 task(不在本 task exec)
- D 批 6 条(类型契约收紧):各自 create 独立 task,先 planning 再 exec
- E 批 5 条(架构定调):E1 全面 feature-sliced 迁移 / E2 gateway 子目录语义边界文档化 / E3 types 按领域重切 / E4 peak_hours·tray 共享 fixtures / E5 并入 E1(popover 三层随 feature-sliced 归位)— 各自独立 task
