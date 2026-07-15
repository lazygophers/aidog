# 平台删除改组件级局部更新 — PRD

## 目标
- Groups 页删除平台（单删 `confirmDeletePlatform` / 批删 `confirmBatchDelete`）后，父级 Platforms 页 platforms state 改局部移除（按 id filter + epoch++），禁全量 `platformApi.list()` refetch
- 消除用户体感「整页刷新」（整列表 setPlatforms 替换 → 全 PlatformCard 重渲染），改为仅被删平台卡片移除 + 派生层重算

## 边界
**范围内**:
- `onPlatformDeleted` 签名 `() => void` → `(ids: number[]) => void`（传被删 id 集合，单删/批删统一）
- usePlatformsState 加 `removePlatformsByIds(ids)`：局部 `setPlatforms(prev => prev.filter(x => !ids.includes(x.id)))` + `platformsEpochRef.current++`（让 membership/standalonePlatforms 派生层重算），不调 API（Groups 已调）
- PlatformListView:88 接线 `onPlatformDeleted={refreshPlatforms}` → `onPlatformDeleted={(ids) => s.removePlatformsByIds(ids)}`
- Groups.tsx confirmDeletePlatform:233 `onPlatformDeleted?.()` → `onPlatformDeleted?.([target.platform.id])`
- Groups.tsx confirmBatchDelete:288 `onPlatformDeleted?.()` → `onPlatformDeleted?.(ids)`

**范围外(非目标)**:
- 不改 Groups 内部 `load()` 重建（useGroupData 自己的数据真值，含 balance/stats，局部化复杂度高，另 task）
- 不改 PlatformEditForm:153 的 refreshPlatforms 调用（CPA apply 创建场景，全量 refetch 合理：新平台需拉回）
- 不改 usePlatformsState.handleDelete（standalone 删除已是局部更新，无 bug）
- 不改后端 delete_platform / batch_delete_platforms

## 验收标准
- [ ] Groups 页单删平台 → 仅该平台卡片消失，其余卡片不重渲染（React DevTools profiler 验无整列表 reconcile）
- [ ] Groups 页批量删 → 选中平台消失，其余不动
- [ ] 被删平台从 standalonePlatforms「未分组」段消失（epoch++ 触发 membership/standalone 重算）
- [ ] groupDetails 正确更新（onGroupsChanged 覆盖，不回归）
- [ ] Group 卡片 balance/stats 仍正确（Groups 内部 load() 覆盖，不回归）
- [ ] 删除失败时不误移（onPlatformDeleted 在 try 内，catch 不触发）
- [ ] `yarn build` 过（tsc + vite）
- [ ] usePlatformsState.test.ts 相关测试不回归

## 索引
- 详细设计: [design.md](design.md)
- 任务/子任务/调度: task.json (`skein.py subtask list platform-delete-partial-update`)
