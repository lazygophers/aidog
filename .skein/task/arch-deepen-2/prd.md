# 架构深化第二轮: 10 组摩擦点全量修复 — PRD (主入口)

## 目标

- [ ] 修复 4 路 Explore 审查（converter/proxy · router/quota/db · Tauri command 边界 · 前端）产出的 **22 个摩擦点**，归并为 11 个 subtask，**全量执行**（非分档取舍）
- [ ] 其中 **4 个是真 bug**（当前线上行为已错，非纯重构）：
  - Gemini 流式：`proxy/finish.rs:286` 的 `strip_prefix("data: ")` 对 Gemini 裸 JSON 帧永不匹配，跨协议转换分支全部落空
  - `ProxyTimeoutSettings.source_protocol`：TS 每次保存都发，Rust struct 无此字段，serde 静默丢弃（`useSystemSettings.ts:233` ↔ `models/settings.rs:54-61`）
  - `resolve_effective_models` 孪生：测试打在 `#[allow(dead_code)]` 的死分支，生产路径 `_cached` 零覆盖（`router/candidates.rs:555/596` ↔ `test_candidates.rs:740`）
  - `ccswitchMatch.ts:269` 写 `manual_budgets: ""`，Rust 侧是 `Vec<ManualBudget>`，靠 import 内部宽松处理兜底
- [ ] 量化目标：净删 ~2500+ 行（死代码 265 + SSE 别名 20 + tracing 样板 ~500 + 手写 TS 类型 ~1577 + 重复回退链），Rust↔TS 契约漂移从 6 对降到 0 且被 CI 闸门锁住

## 边界

**范围内**（11 subtask，形态已经 AskUserQuestion 锁定）：

| id | 组 | 形态决策 |
|---|---|---|
| c1-typedrift | 类型契约止血 | 手修 6 对漂移 + `scripts/check-types.mjs` 比对闸门 |
| c1b-tsrs | 类型契约根治 | ts-rs codegen 全量，删手写 `types/part*.ts` |
| c2-extra | `platform.extra` | Rust 侧 `PlatformExtra` serde struct + `#[serde(flatten)] rest`（TS 侧本轮不动） |
| c3-commands | commands_* 转发层 | **整体删除**，`#[tauri::command]` 上移 aidog_core，`feature = "tauri"` gate 保 core 可独立测 |
| c4-protocol | 协议身份 | `source_protocol: &str` → `Protocol` enum，加 `same_wire_family` |
| c5-sse | SSE 分帧 | 分帧收进 adapter，补 Gemini `alt=sse` |
| c6-estcost | 计费 locality | `calc_est_cost` 移出 `db/stats_today.rs`，拆纯函数 |
| c7-deadcode | 死代码 | 删 `router/selection.rs` 整模块 + 孪生函数 + 4 个 dead SSE 别名 |
| c8-fehooks | 前端 state-bag | Logs 立范式（切三段 hook）+ Settings `applyImport` 抽纯函数 |
| c9-statusline | statusline 双份推导 | 统一到 `materializeStatusline` + snapshot test |
| c10-cliproxy | CliProxy god page | 抽 `useCliProxySelection` + 3 个批量 modal 外迁 |

**范围外（非目标）**：
- [ ] TS 侧 `platform.extra` 的 14 个 parse/serialize 不动（`api.test.ts` 已覆盖，改动收益低于回归风险；c2 只收 Rust 侧）
- [ ] `quota/mod.rs:203-250` 的 `base_url.contains()` 分发不动（Explore 判定 deletion test **不通过** —— 删了只是把 URL→provider 判定挪给调用方，除非同时把 provider HTTP client 变成注入 seam）
- [ ] `mitm/ca.rs`（1223 行）不动，本轮无 seam 交叉
- [ ] `statusline-runtime.ts` 的 762 行 ENGINE_PY 常量不外迁成 .py 资源（deletion test = 只挪走）
- [ ] `Groups.tsx` ↔ `PlatformListView` 的 ref 回调总线不动（跨 3 文件状态归属重划，成本非平凡；仅摘出「给 `domains/groups/editReducer.ts` 补 test」这一小项，并入 c8）

**已知约束**：
- [ ] **c3 打破 crate 分层**：`aidog_core` 将依赖 `tauri`。用户已知悉并拍板。缓解：`feature = "tauri"` gate，`cargo test -p aidog_core --no-default-features` 必须仍能跑
- [ ] Protocol 枚举改动受 spec `arch/trellis-04.md`（变体扩展范式）+ `domain/rule-51.md`（5 协议锚点）+ `build/rule-05.md`（wire protocol 白名单同步）约束
- [ ] 跨 Rust↔TS 字段必须 snake_case（spec `cross-layer/trellis-20.md`），ts-rs 生成配置须锁 snake_case，禁 camelCase 化
- [ ] `extra` 改 struct 必须保 unknown-key 往返（`_ui_*` 键，`db/test_ui_extra.rs:32` 已有测试兜底）
- [ ] worktree 禁用（原地执行），auto_commit 启用，max_parallel 2

## 验收标准

- [ ] c1: 6 对漂移全修（`ProxyTimeoutSettings` / `Platform.sort_order` / `Group.sort_order` / `GroupPlatform` 三时间戳 / backup meta / ccswitch 构造器）；`scripts/check-types.mjs` 接进 `package.json:check:types`，当前仓库跑出 0 不一致
- [ ] c1b: 183 个 Rust struct 中被前端消费的部分加 `#[derive(TS)]`，生成物落 `src/services/api/types/generated/`，手写 `part1.ts`/`part2.ts` 删净，`tsc` 0 err
- [ ] c2: `PlatformExtra` struct 落地，Rust 侧 12 个 ad-hoc 解析器归一，4 份 `to_string(&platform_type).trim_matches('"')` hack 消除；`db/test_ui_extra.rs` 全过（unknown-key 往返不破）
- [ ] c3: 6 个 `commands_*` crate 删除，`startup.rs` 注册表瘦身，tracing 走单点 macro，错误日志覆盖率 49/194 → 194/194；`cargo test -p aidog_core --no-default-features` 通过
- [ ] c4: 全 gateway 无 `source_protocol: &str`，`same_wire_family` 单点，`to_client_sse` 为穷尽 match（无 `_ =>` 兜底）
- [ ] c5: Gemini 出站带 `alt=sse`，`to_gemini_sse` 自带前缀与另两协议约定一致，`finish.rs` 的 `strip_prefix("data: ")` 循环删除；新增 ≥2 个 gemini 流式端到端测试
- [ ] c6: `calc_est_cost` 迁出 `db/stats_today.rs`，`platform_peak_hours` 删除（回退链单份），纯函数 `est_cost_from` 有 ≥3 个测试
- [ ] c7: `router/selection.rs` + `test_selection.rs` 删净（~265 行），`resolve_effective_models` 孪生消除且测试改指生产路径，4 个 dead SSE 别名 + 2 个陪葬断言删除
- [ ] c8: `useLogsData` 切为 filters/list/detail 三段，新增 ≥1 个 hook 测试；`applySelectedPaths` 纯函数外迁 + 测试；`editReducer` 补测试
- [ ] c9: `useStatusLinePanel` 不再自带推导，统一取 `materializeStatusline`；新增 2 个脚本生成 snapshot test
- [ ] c10: `CliProxy.tsx` < 600 行，`useCliProxySelection` 抽出（7 个布尔碎片收敛），`quotaTypeOf` 纯函数化 + 测试
- [ ] 全周期门禁：`cargo clippy` 0 warning / `cargo test` 全过 / `tsc` 0 err / `yarn test` 全过 / `check-i18n` 0 缺译

## 索引
- [ ] 详细设计: [design.md](design.md)（DAG + 各组 deletion test 判定 + 高风险点缓解）
- [ ] 审查证据: 本 task 由 4 路 Explore agent 产出，所有断言带 `file:line`，见 design.md 证据表
- [ ] 任务/子任务/调度: task.json（`skein subtask list arch-deepen-2`）
