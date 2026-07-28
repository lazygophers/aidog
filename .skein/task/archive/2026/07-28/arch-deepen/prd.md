# 架构深化: C1-C10 候选全执行 (deletion test / locality / leverage) — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] 落地 `/improve-codebase-architecture` 审查产出的 10 候选深化项, 按 deletion test / locality / leverage 三维过滤后分档执行
- [ ] **Strong (C1-C5)** 直接执行: 死代码清扫 + proxy/parser 真冗余抽象 + PlatformCard 拆解
- [ ] **Worth exploring (C6/C8)** grill 定形态后执行: event 总线单入口 + mitm seam 收敛
- [ ] **Speculative (C7/C9/C10)** grill 评估是否做: god surface reducer 化 + db mod.rs 拆 + migration 编号
- [ ] 量化目标: 净删 ~3500+ 行, proxy/platforms/db 三层 locality 提升, bug 面 (字段错位 / 双写漂移 / event 漏广播) 单点化

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: 10 候选各自闭环 (plan→exec→check→finish), 按依赖 DAG 编排 (Wave1={C1,C2,C3,C4,C8,C9} → Wave2={C5} → Wave3={C6} → Wave4={C7} → Wave5={C10})
- [ ] 范围外: 不重构 call_*_traced 6 份复制 (作者有意选复制, maintenance.rs:64 注释, 泛型签名复杂度≈复制成本) / 不拆 schema_late.rs 按库分文件 (已按库分 3 fn, 痛感来自注释密度非结构, 仅 C10 整编号体系) / 不引入 query builder (SQL 不复杂, ponytail ladder rung 5 已判)
- [ ] 已知约束: 每候选 deletion test 必须通过 (删浓缩 vs 只挪); 浅模块 (deletion test 未过) 标 Speculative 需 grill 重设计形态; proxy 路径有现成 test 兜底 (forward 290行/parser 305行/passthrough 部分), 抽 helper 不破测试
- [ ] worktree 禁用 (原地执行), auto_commit 启用, max_parallel 2

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] C1: 非-Inline editors×4 + EnhancedSelect/pinyin + 11 ui primitive 删净, barrel editors/index.ts 同步删 export, grep 零残留, tsc 0 err / yarn test 全过 (基线 299, 删 EnhancedSelect 测后降基线) / check-i18n 0 缺译 / package.json 移除 pinyin-pro 依赖
- [ ] C2: forward.rs finalize_proxy_502 抽出, 5 caller 缩 1 行, cargo test forward 全过, 净删 ~80 行
- [ ] C3: parser.rs parse_simple_array 抽出, 5 caller 各 1 行, parser/archive.rs 拆出, cargo test parser 全过, 净删 ~80 行
- [ ] C4: passthrough.rs relay_passthrough + PassthroughOpts 抽出, 2 caller 设 opts, StreamLogGuard 单点, cargo test passthrough 全过, 净删 ~150 行
- [ ] C5: PlatformCard useProtocolMeta hook 抽出 (5 effect → 1), 健康判定移 health.ts, getBaseUrl 用 canonical, yarn test PlatformCard 全过, 净删 ~65 行
- [ ] C6: afterPlatformMutation 单入口, 3 magic event 收口 (或 grill 定保留/移除), handleSave/Delete/Toggle/runBatch 统一调, yarn test usePlatformsState 全过
- [ ] C7: grill 定形态 (reducer / ref / 文档化) 后执行或明确不做, 产出决策记录
- [ ] C8: mitm handle_connect_mitm 单入口, connect.rs 反向依赖消 (或 grill 定保留), serve_plaintext 跨模块递归评估, cargo test connect+mitm 全过
- [ ] C9: db/cache.rs 拆出 KeyPair/DbCache, parse/serialize 移 platform.rs, mod.rs 回归连接根, cargo test db 全过, 净删 ~40 行
- [ ] C10: grill 定编号方案 (日期戳 / 文档化 / 按库拆) 后执行或明确不做, 产出决策记录
- [ ] 全周期: lint (clippy + eslint) 0 warning / tsc 0 err / cargo test 全过 / yarn test 全过 / check-i18n 0 缺译

## 索引
- [ ] 详细设计: [design.md](design.md) (DAG + 各候选 deletion test 判定 + grill 待决点)
- [ ] 审查报告: `/var/folders/_n/h0lzhkxx3671xlpg9k69c2qh0000gn/T/architecture-review-20260726.html` (4 Explore agent 证据 + before/after 图)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein subtask list arch-deepen`)
