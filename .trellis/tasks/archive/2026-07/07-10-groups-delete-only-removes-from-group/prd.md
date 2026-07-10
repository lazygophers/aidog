# Groups 删除平台后 platforms state 不刷新致「未分组残留」bug 修复

## Goal

修 Groups 分组页点「删除平台」后平台以「未分组」身份残留的 bug（用户体感「只移除分组未彻底销毁」），并扫所有 `platformApi.delete` 调用入口统一 platforms state 刷新链，防类似遗漏。

**根因**（research `delete-platform-bug.md`）：后端 `delete_platform`（`platform_lifecycle.rs:29-51`）真软删（`UPDATE platform SET deleted_at` + `DELETE FROM group_platform` + invalidate cache），后端无 bug。bug 在前端刷新链断裂：
- `confirmDeletePlatform`（`Groups.tsx:171-185`）成功后调 `load()`（仅刷 useGroupData 本地 state）+ `onGroupsChanged?.()`
- 父级 `usePlatformsState.handleGroupsChanged`（`usePlatformsState.ts:344-348`）**只 refetch groupDetails，不 refetch platforms**
- `standalonePlatforms`（`usePlatformsState.ts:582-593`）= `platforms.filter(p => !platformMembership.has(p.id))`
- 被删平台仍在 stale `platforms` state，membership 被 effect 清掉后 → 归 `standalonePlatforms`「未分组」段 → 用户见「平台以未分组残留」=「只移除分组未销毁」

**对照**：Platforms 列表自身删（`usePlatformsState.handleDelete` L455-486）有乐观更新 `setPlatforms(filter)`，故不复现。

**07-08 回归根因**：上次修 modal groupCount stale（overcount 致误显双按钮），但**漏了刷新链**（confirmDeletePlatform 成功后 platforms state 不刷）—— 故用户说「出现很多次没修好」。

## 用户澄清决策（AskUserQuestion 已答，2026-07-10）

| 问题 | 用户选 |
|---|---|
| 修复策略（confirmDeletePlatform 成功后怎么刷 platforms state） | **独立信号 + 全量 refetch**（usePlatformsState 加 onPlatformDeleted 回调，confirmDeletePlatform 触发，父级监听后 platformApi.list() + setPlatforms + ++platformsEpochRef。稳，与后端真删对齐，多一次 RPC 可接受） |
| scope 边界 | **扫所有 platformApi.delete 调用入口**（防类似遗漏，非仅 Groups 路径） |

## Scope

### `platformApi.delete` 全调用入口（grep 确认，src/ 内仅 2 处）

| 入口 | 文件:行 | 现有刷新 | bug |
|---|---|---|---|
| Platforms 页 onDelete → `usePlatformsState.handleDelete` | `usePlatformsState.ts:455-486`（platformApi.delete @ L468） | 有乐观 `setPlatforms(filter)` + groupDetails refetch | **无 bug**（不复现） |
| Groups 页 confirmDeletePlatform → `cards.handleDelete` | `Groups.tsx:171-185`（platformApi.delete 经 usePlatformCards.handleDelete L191-193） | 仅 `load()` + `onGroupsChanged?.()`（刷 groupDetails 不刷 platforms） | **有 bug**（本 task 修） |

`usePlatformCards.handleDelete`（L191-193）= `await platformApi.delete(id)`，是被调对象非独立入口（仅 Groups 调）。

### R1 usePlatformsState 加独立 platforms 刷新信号

- 新增 `refreshPlatforms()` 方法（`usePlatformsState.ts`）：`platformApi.list()` 全量 refetch → `setPlatforms(data)` → `++platformsEpochRef.current`（触发派生层重算，同现有 platforms 加载链）
- **独立于 `onGroupsChanged`**（research 建议，避免语义过宽 —— onGroupsChanged 当前只刷 groupDetails，复用它刷 platforms 会污染其他 onGroupsChanged 调用点的语义）
- 暴露经 props/callback 给 Groups 页（如 `onPlatformDeleted?: () => void` 回调，绑到 refreshPlatforms）

### R2 confirmDeletePlatform 触发刷新

- `Groups.tsx:171-185` confirmDeletePlatform 成功后，除现有 `load()` + `onGroupsChanged?.()`，**新增触发 platforms 刷新**（经 R1 的 onPlatformDeleted 回调）
- 回调由父级（Platforms 编排层 / App 路由）注入，绑到 `usePlatformsState.refreshPlatforms`

### R3 Platforms 页 handleDelete 刷新链复验（扫所有入口）

- `usePlatformsState.handleDelete`（L455-486）现有乐观更新 `setPlatforms(filter)` + groupDetails refetch，**复验无遗漏**（platforms state 真同步删 + groupDetails 刷 + standalonePlatforms 派生正确）
- 若复验发现遗漏（如乐观更新后未 ++epoch 致派生层 stale）→ 补齐（对齐 R1 的 refreshPlatforms 语义）
- **若复验无遗漏** → 不改 Platforms 路径（已有乐观更新足够），仅文档记录「已验」

### R4 standalonePlatforms 派生正确性

- `standalonePlatforms`（L582-593）= `platforms.filter(p => !platformMembership.has(p.id))`，依赖 platforms state + membership（groupDetails 派生）
- R1 refreshPlatforms 后 platforms state 删被删平台 → membership 无需清（平台不在 platforms）→ standalonePlatforms 自动排除
- 验证：删后 standalonePlatforms 不含被删平台 id

### R5 回归测试（hook 级 renderHook + 扫全入口，用户决策）

前端测试基建齐全（vitest + @testing-library/react + jsdom，已有 16 测试文件 utils/services/shared 层；CLAUDE.md「前端无 test 脚本」过时，实际 `yarn test` / `test:watch` / `test:cov` 都有）。本 task 写 hook 级回归测试，扫全 `platformApi.delete` 入口：

- **测试文件**：`src/pages/platforms/usePlatformsState.test.ts`（新）+ `src/pages/Groups.test.tsx`（新，若 Groups 组件 mock 可行）或合并到一个 hook 级测试
- **测试覆盖**（hook 级 renderHook）：
  1. **confirmDeletePlatform 路径**（Groups bug 路径）：renderHook(usePlatformsState) → mock `platformApi.delete` resolve + `platformApi.list` 返回删后集 → 模拟 confirmDeletePlatform（经 onPlatformDeleted 回调）→ 断言 `platforms` state 删被删平台 + `platformsEpochRef` ++ + `standalonePlatforms` 派生不含被删 id
  2. **Platforms handleDelete 路径**（现有乐观更新复验）：renderHook → mock delete resolve → 调 handleDelete → 断言 `setPlatforms(filter)` 乐观更新触发 + groupDetails refetch + platforms state 删 + epoch 对齐（若有遗漏补齐）
- **mock 策略**：`vi.mock('@/services/api')` mock `platformApi.delete` / `platformApi.list` + `groupApi` / `groupDetailApi`；invoke（Tauri）经 services/api 层封装 mock（不直 mock @tauri-apps/api）
- **难点**：`usePlatformsState(PlatformsStateParams)` 依赖 params（onNavigate / initialFilter / groupsReloadRef），需构造最小 mock params；若依赖过重，降级测 `refreshPlatforms` 单元 + confirmDeletePlatform 调用链断言（mock usePlatformsState 局部）

## Requirements

### 改动文件（预期，worktree 内）
- `src/pages/platforms/usePlatformsState.ts` — 加 `refreshPlatforms()` 方法 + 暴露 `onPlatformDeleted` 回调（绑 refreshPlatforms）；复验 handleDelete L455-486 刷新链
- `src/pages/Groups.tsx` — confirmDeletePlatform L171-185 新增触发 onPlatformDeleted（经 props 从父级注入）；Groups props 加 `onPlatformDeleted?: () => void`
- `src/pages/Platforms.tsx`（或 App 路由层）— 把 usePlatformsState.refreshPlatforms 经 props 传给 Groups（onPlatformDeleted）

**禁改**：后端 delete_platform（无 bug）/ usePlatformCards.handleDelete（是被调对象非入口）/ client-types.json / 任何 Rust 源码。

### 验证
- `yarn build` 全绿（TS 类型 / 调用链全通）
- `yarn check:i18n`（无 i18n 改动，应秒过）
- 手动验证路径（Tauri dev 冒烟，可选 exec agent 跑）：
  - Groups 页单组平台点「删除平台」→ 平台从 platforms state 移除 → standalonePlatforms 不含 → 平台彻底消失（非「未分组残留」）
  - Groups 页多组共享平台点「删除平台（全部组）」→ 同上
  - Platforms 页 onDelete → 复验现有乐观更新仍正常
- grep 验收：
  - grep `refreshPlatforms` 在 usePlatformsState.ts ≥1（新方法）
  - grep `onPlatformDeleted` 在 Groups.tsx + 父级注入点（R2 接线）
  - confirmDeletePlatform 内调 onPlatformDeleted（Groups.tsx:171-185 段）

## Acceptance Criteria

- [ ] usePlatformsState 加 refreshPlatforms() 方法（platformApi.list + setPlatforms + ++epoch）
- [ ] confirmDeletePlatform 成功后触发 platforms 全量 refetch（经独立 onPlatformDeleted 信号，非复用 onGroupsChanged）
- [ ] Groups 页 props 接 onPlatformDeleted，父级注入绑 refreshPlatforms
- [ ] Platforms 页 handleDelete 刷新链复验（无遗漏则文档记录，有遗漏则补齐）
- [ ] standalonePlatforms 派生正确（删后不含被删平台）
- [ ] yarn build 全绿
- [ ] yarn test 新 hook 级回归测试过（confirmDeletePlatform + Platforms handleDelete 两路径，mock platformApi.delete/list）
- [ ] 手动冒烟：单组删 + 多组删全部 + Platforms 页删，三路径平台均彻底销毁无残留
- [ ] 后端零改（delete_platform / Rust 源码 0 diff）
- [ ] 主仓零改动（worktree 内）

## Out of Scope

- 后端 delete_platform 逻辑（research 确认真软删，无 bug）
- usePlatformCards.handleDelete（是被调对象非独立入口，不改）
- modal groupCount stale 修复（07-08 已修，本 task 不动）
- Platforms 页 handleDelete 乐观更新改全量 refetch（用户选「独立信号+全量refetch」针对 Groups 路径；Platforms 已有乐观更新 work，复验后保留不改，除非复验发现遗漏）
- 批量删 / 清 auto_disabled 平台路径（grep 确认仅 2 个 platformApi.delete 入口，无其他）
- client-types.json / Rust 源码 / 其他 defaults JSON

## Technical Notes

- **反复犯错标记**（sediment 候选）：07-08 修 modal groupCount stale，漏刷新链 → 仍复现。同类「删除后前端 state 刷新链断裂」错误在 ≥2 次修复尝试出现（grep task journal 可验），根因可写为可验证契约「platformApi.delete 调用入口 MUST 触发 platforms state 刷新（乐观或全量 refetch），不能仅刷 groupDetails」—— finish 时 sediment 判定门评估
- **刷新策略选独立信号非复用 onGroupsChanged**（research 建议）：onGroupsChanged 当前语义「group 结构变更 → 刷 groupDetails」，扩它刷 platforms 会污染其他调用点（如 group 增删 / 平台拖拽）。独立 onPlatformDeleted 语义专一「platform 被删 → 刷 platforms state」
- **全量 refetch 非乐观 filter**（用户选）：乐观 filter 快但需保后端真删（已验），且 groupDetails 需另刷（乐观 filter 不触发 groupDetails reload）。全量 refetch 一次 RPC 同时刷 platforms + 触发派生层（membership/standalonePlatforms）重算，语义清晰。多一次 RPC（~10ms）可接受
- **platformsEpochRef**（usePlatformsState 现有）：派生层（membership/standalonePlatforms/usageMap 等）依赖 epoch 触发重算，refreshPlatforms 必须 ++epoch 对齐现有 platforms 加载链（grep platformsEpochRef 确认现有用法）
- **跨层契约**（spec guides/cross-layer-rules.md）：本 task 纯前端刷新链修复，后端零改，公共契约层（Rust struct/serde/command/TS invoke 类型）0 diff

## 数据流（修复后，强制）

```
Groups confirmDeletePlatform
  → platformApi.delete(id)  [后端真软删 deleted_at + 摘 group_platform + invalidate cache]
  → load()  [刷 Groups 本地 groupDetails]
  → onGroupsChanged?.()  [父级刷 groupDetails]
  → onPlatformDeleted?.()  [新增：父级 refreshPlatforms]
      → platformApi.list() + setPlatforms(data) + ++platformsEpochRef
      → platforms state 删被删平台
      → membership effect / standalonePlatforms 派生层重算（epoch 触发）
      → 被删平台从 standalonePlatforms 排除（不在 platforms state）
  → UI：平台彻底消失（非「未分组残留」）
```