# Implement — 执行层编排

> 单交付任务。5 改动点（①–⑤，定义见 prd.md §4）有依赖与共享文件冲突，需分组 + 串行/并行编排。
> 授权：本项目可自动 `git commit`（CLAUDE.md），所有改动完成后提交；禁 `git push`。

## 依赖关系

```
① usage 批量化 (db.rs/lib.rs/api.ts/Platforms.tsx)  ─┐
② proxy_log 索引 (db.rs migration)                   ─┤→ 前端消费批量结果
                                                       │
④ PlatformCard 骨架占位 (PlatformCard.tsx)           ←┘ （骨架需消费 ①的批量结果状态 + 渐进/延迟档时机）
⑤ 可视区优先 quota 调度 (Platforms.tsx)              ←  （依赖 ③ 的池存在）
③ quota 有界并发池 (Platforms.tsx)                   ─  独立（不依赖 ①②）
```

- **②** 索引独立，但是 **①** 批量查的性能最优前提；① 逻辑可独立于 ② 完成（无索引也跑，只是慢）。建议 ②→① 顺序，或 ② 与 ① 同一后端 agent 内顺序完成。
- **④⑤** 应**晚于 ①**：④ 骨架三态、⑤ 调度都要消费 ① 后端批量结果的填充时机/状态。
- **③** quota 并发池与后端 ①② 完全独立，可并行启动。
- **⑤** 依赖 **③** 的并发池已存在（observer 决定入池时机，池控上限）。

## 共享文件冲突（关键约束）

`src/pages/Platforms.tsx` 被 ①③⑤ 同时触及，是最大冲突源：
- ① 改 usage 调用点（`:1594` forEach + `:1622` refreshStats）
- ③ 改 quota forEach → 并发池（`:1603`）
- ⑤ 改 quota 调度 + 列表项 ref（与 ③ 同一 quota 区块、且加 JSX ref）

**编排决策：`Platforms.tsx` 的所有改动（①前端部分 / ③ / ⑤）必须串行化，由同一前端 agent 顺序完成，禁止多 agent 并行编辑此文件。** 仅 `PlatformCard.tsx`（④）与后端文件（①②的 db.rs/lib.rs + api.ts）可与 Platforms.tsx 的编辑分区并行。

## 阶段编排

### 阶段 1（可并行：后端组 ∥ quota 池组，二者不共享文件）

**Subagent A — 后端 usage 批量化 + 索引（①②的后端 + 跨层）**
- 目标：新增 `platform_usage_stats_all` 批量查（GROUP BY platform_id，保 platform_id=0 回溯）+ proxy_log 索引 migration + lib.rs command + api.ts TS 封装。
- 产出：`db.rs` 新查询函数 + migration；`lib.rs` 新 `#[tauri::command]`；`src/services/api.ts` 新 invoke 封装 + 类型。**不改 Platforms.tsx。**
- 验证：`cd src-tauri && cargo test && cargo clippy`（warning 清零）；`yarn build`；`EXPLAIN QUERY PLAN` 确认走索引；用例对比改前用量一致。
- 资源（文件集）：`src-tauri/src/gateway/db.rs`、`src-tauri/src/lib.rs`、`src/services/api.ts`。
- 依赖：无（阶段起点）。

**Subagent B — quota 有界并发池骨架（③，仅 Platforms.tsx 的 quota 区块）**
- ⚠️ **资源互斥**：B 也改 `Platforms.tsx`。若与后续阶段的 Platforms.tsx 改动串行，则 B 不能与任何其他改 Platforms.tsx 的 agent 同时跑。
- **编排选择（二选一，推荐前者）**：
  - **推荐**：将 ③（B）合并进阶段 2 的前端 agent，统一在一个 agent 内顺序改 Platforms.tsx（① 前端调用点 + ③ 池 + ⑤ 调度），彻底消除冲突。阶段 1 只跑 Subagent A。
  - 备选：B 独立先跑，跑完合并提交后再启阶段 2 前端 agent；二者**串行**，绝不并行。
- 目标：`Platforms.tsx:1603` 裸 forEach → worker pool（仿 `Groups.tsx:678-690`），并发上限常量。
- 验证：`yarn build`；在飞 quota 数 ≤ 上限、不丢平台。

### 阶段 2（串行，依赖阶段 1 完成并合并）

**Subagent C — 前端体感（① 前端调用点 + ④ 骨架 + ⑤ 调度，按推荐含 ③）**
- 目标：
  1. `Platforms.tsx:1594`/`:1622` usage forEach → 调用 A 新增的批量 command 填 `usageMap`。
  2. （若采纳推荐）③ quota 并发池。
  3. ④ `PlatformCard.tsx` 余额/用量骨架三态（复用 `quotaRefreshing`，禁 est_balance 闪烁；用量区渐进档前显骨架）。
  4. ⑤ IntersectionObserver 可视区优先 quota 调度 + 列表项 ref（**待 main 确认是否首版纳入**，未确认则按纳入实现）。
- 产出：`src/pages/Platforms.tsx`（usage 批量调用 + quota 池 + 可视区调度 + ref）、`src/components/platforms/PlatformCard.tsx`（骨架）、视情况 `src/components/platforms/usePlatformCards.ts`（暴露三态标志）。
- 验证：`yarn build`；首屏卡片用量/余额显骨架不空白；usage 批量到达替换；quota 逐平台无闪烁；长列表首屏仅拉可视 quota、滚动触发其余、不重复、折叠不触发；GroupsEmbedded 复用卡表现一致。
- 资源（文件集）：`src/pages/Platforms.tsx`（**独占**）、`src/components/platforms/PlatformCard.tsx`、`src/components/platforms/usePlatformCards.ts`。
- 依赖：阶段 1 Subagent A 已合并（消费其批量 command + api.ts 类型）。

> ④ PlatformCard 骨架若拆给独立 agent：`PlatformCard.tsx` 与 `Platforms.tsx` 不冲突，可与 C 的 Platforms.tsx 编辑并行；但 ④ 需要 C 暴露的 usage 批量/quota 加载状态，故仍建议同 agent 内完成或 C 先定状态契约。

## 并行/串行总览

| 阶段 | Subagent | 文件集 | 并行性 |
| --- | --- | --- | --- |
| 1 | A 后端批量+索引 | db.rs / lib.rs / api.ts | ∥ 与 B（不同文件） |
| 1 | B quota 池（备选独立） | Platforms.tsx | 与 C 串行；推荐并入 C |
| 2 | C 前端体感（①前端/③/④/⑤） | Platforms.tsx（独占）/ PlatformCard.tsx / usePlatformCards.ts | 依赖 A 合并后启动 |

## 收尾

- 全部改动完成 → `cd src-tauri && cargo test && cargo clippy`（warning 清零）+ `yarn build` + `node scripts/check-i18n.mjs`（若新增文案 key）全绿 → `git commit`（conventional commits，scope `platforms`）。
- 失败处理：cargo/clippy/build/i18n 任一红 → 修至绿再宣告 Done，禁掩盖。
