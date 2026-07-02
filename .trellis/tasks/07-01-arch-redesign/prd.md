# PRD — 全仓架构重设计（分包分文件消大文件）

> 时序：**排当前 task（cli-integration-tab / sensenova-platform / test-coverage-80）finish 后启动**。用户决策（撞车风险：api.ts/editors.tsx 并发改易冲突）。
> brainstorm 排后：架构重设计是大决策，需深度逐问交互（目录结构/包边界/迁移策略），与 exec 同步启动。当前 tab/sensenova 跑中，main 编排负载已满，专注 brainstorm 待 task 空闲。

## 目标
合理分包分文件，消除大包大文件，全仓架构重设计（含目录结构重组 + 包边界重划）。

## Audit 基线（2026-07-01 main 实测）

### Rust（53912 行）— 已合理分层，问题小
- gateway/ 10 子模块，最大生产文件 schema_late.rs 741 + proxy_log.rs 659 + forward.rs 602
- 最大文件 test_integration.rs 798（测试，可接受）
- **结论**：Rust 分层清晰，无需大改；局部 forward.rs/proxy_log.rs/mod.rs 可选拆分

### 前端（34552 行）— 问题集中，4 巨型文件
| 文件 | 行数 | 病灶 |
|---|---|---|
| `components/settings/editors.tsx` | **4609** | settings 全字段编辑器 + 特殊编辑器 + 令牌 F/S 全堆 |
| `pages/Platforms.tsx` | **3568** | 巨型页面（列表/编辑/浮窗/统计全揉） |
| `pages/Groups.tsx` | **2195** | 巨型页面 |
| `services/api.ts` | **2072** | 73 command invoke + TS 类型全堆 |
| `components/settings/ImportExport.tsx` | 1525 | 导入导出 + diff 全揉 |
| `pages/Mcp.tsx` / `Skills.tsx` / `Logs.tsx` | 900-1050 | 大页面 |
| `components/settings/statusline-gen.ts` | 1045 | 状态行生成 |
| 其他 8 文件 | 700-850 | 中等 |

## scope（用户定：全仓架构重设计）
1. **前端巨型文件拆分**（首要）：editors.tsx / Platforms.tsx / Groups.tsx / api.ts / ImportExport.tsx
2. **前端目录结构重组**：pages/components/services/utils 重新划边界
3. **Rust 局部拆分**（次要）：forward.rs / proxy_log.rs / db/mod.rs（593）按职责拆
4. **包边界重划**：跨层依赖清理（如 components/settings 是否独立包）

## 决策锁（2026-07-02 AskUserQuestion，详见 design.md）

1. **拆分粒度**：按域聚簇，每子文件 ≤800 行硬上限（research/section-split-map 已给逐 export 实测 + 拆分映射表）
2. **目录新结构**：`src/domains/{platforms,groups,settings,shared}` + `services/api/` 子目录（research/dependency-graph 包边界 + 依赖图，无环）
3. **api.ts 拆法**：13 域文件 + `types.ts` + barrel `index.ts`（零外部 import churn）
4. **迁移策略**：**渐进分阶段**（api 抽 → 消重 → editors 拆 → 巨型组件二次拆 → Rust 收尾）
5. **载体**：**subagent 编排**（main 动态 DAG 调度，并发 2；不开 workflow）
6. **巨型组件二次拆**：UI 区块 + hook 混合（先抽 hook 收 state → 再抽 JSX 区块子组件）
7. **回归保障**：build + i18n 门禁 + 关键路径手测（不前置测试）

## 调度
- brainstorm 已出 `design.md`（目录新结构图 + 拆分映射 + 阶段 mermaid + 验收门）→ grill 对抗校对 → 用户评审 → start
- exec：subagent 编排，阶段间串行 / 阶段内按文件集判并发（详见 design.md 阶段调度段）

## 验收（待 brainstorm 细化）
1. 无 >800 行前端文件（editors/Platforms/Groups/api/ImportExport 全拆）
2. Rust 无 >600 行生产文件（局部拆）
3. 目录结构合理（每包/目录单一职责，包边界清晰）
4. `yarn build` + `cargo test` + `cargo clippy` 全绿（无回归）
5. 关键流程手测通过（平台增删/分组/代理转发/导入导出/设置）

## 非目标
- 不改业务逻辑（纯结构重构，行为零变更）
- 不改 i18n key / locale（key 名稳定）
- 不改 Tauri command 签名（跨层契约不动）

## 风险
- 全仓 import 路径更新（高风险，漏改 = 编译炸）
- editors.tsx 4609 行拆分易破坏 settings 编辑器状态机（[[settings-page-architecture]]）
- api.ts 拆分需保 invoke 包装泛型标注不丢（[[tauri-invoke-param-camelcase]]）
- 与 coverage task 排序：coverage 补测试应在重构后（重构改路径，测试需跟随）或重构前（测试作安全网）？brainstorm 定
