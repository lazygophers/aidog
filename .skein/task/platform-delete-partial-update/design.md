# 平台删除改组件级局部更新 — 设计

## 根因
删除平台两入口行为分裂：

| 入口 | 路径 | 行为 |
|---|---|---|
| standalone 删 | `Platforms.tsx:68 → usePlatformsState.handleDelete:492` | 局部 `setPlatforms(filter)` + epoch++ ✅ |
| Groups 单删 | `Groups.tsx:225 confirmDeletePlatform → cards.handleDelete(id) + onPlatformDeleted?.()` | API 调用后父级 `refreshPlatforms` **全量 refetch** ❌ |
| Groups 批删 | `Groups.tsx:283 confirmBatchDelete → platformApi.batchDelete(ids) + onPlatformDeleted?.()` | 同上 ❌ |

`refreshPlatforms`（usePlatformsState:375）= `const list = await platformApi.list(); setPlatforms(list); platformsEpochRef.current++` —— 整列表 RPC 拉取 + setState 替换，触发所有 PlatformCard 重渲染 = 用户体感「整页刷新」。

`cards.handleDelete`（usePlatformCards:202）= 仅 `platformApi.delete(id)`，不碰任何 state。故 Groups 删除后父级 platforms state 的同步完全依赖 `onPlatformDeleted`，当前接的是全量 refetch。

## 修复
复用 `handleDelete:492` 已验证的局部移除模式（乐观 filter + epoch++），但跳过 API（Groups 已调 cards.handleDelete / batchDelete）。

### 数据流（修后）
```
Groups confirmDeletePlatform:
  await cards.handleDelete(id)          # API delete（usePlatformCards:203）
  load()                                # Groups 内部 useGroupData 重建
  onGroupsChanged?.()                   # 父级 handleGroupsChanged → 刷 groupDetails
  onPlatformDeleted?.([id])             # 父级 removePlatformsByIds → 局部 filter + epoch++（不再全量 refetch）

Groups confirmBatchDelete:
  await platformApi.batchDelete(ids)
  load(); onGroupsChanged?.()
  onPlatformDeleted?.(ids)              # 同上局部移除
```

### removePlatformsByIds 实现（usePlatformsState）
```ts
const removePlatformsByIds = useCallback((ids: number[]) => {
  if (ids.length === 0) return;
  platformsEpochRef.current++;
  setPlatforms(prev => prev.filter(x => !ids.includes(x.id)));
}, []);
```
- epoch++ 让 membership effect + standalonePlatforms useMemo 重算（同 handleDelete:494 模式）
- 不调 handleGroupsChanged（onGroupsChanged 已在 Groups 侧调，避免重复）
- 不调 API（调用方已调）

## 改动文件
1. `src/pages/platforms/usePlatformsState.ts` — 加 `removePlatformsByIds`（~6 行）+ return 暴露 + interface 声明
2. `src/pages/platforms/PlatformListView.tsx:88` — `onPlatformDeleted={refreshPlatforms}` → `onPlatformDeleted={(ids) => s.removePlatformsByIds(ids)}`
3. `src/pages/Groups.tsx` — `onPlatformDeleted?: () => void` → `(ids: number[]) => void`；confirmDeletePlatform:233 + confirmBatchDelete:288 传 ids

## 不改
- `refreshPlatforms` 方法保留（PlatformEditForm:153 CPA apply 仍用）
- `handleDelete` 不动（standalone 路径无 bug）
- Groups 内部 `load()`（另 task）
- 后端

## 风险
- **低**：局部移除模式已被 handleDelete:492 验证（standalone 删每秒级发生，无回归报告）
- 派生层（membership/standalonePlatforms）靠 epoch++ + platforms 变更触发 useMemo 重算，与 handleDelete 同链路
