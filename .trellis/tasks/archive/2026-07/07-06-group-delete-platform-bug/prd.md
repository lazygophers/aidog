# 分组卡片「删除平台」仅移除关联未删平台 (bug)

## 现象 (user report)
- 平台**只存在于一个分组**
- 在分组卡片点「删除」→ 弹出「删除平台」确认 modal
- 点「删除平台」按钮
- **实际效果**: 平台从分组中移除, 但平台**未真正删除** (在 Platforms 主页 / 其它入口仍可见)
- **预期效果**: 平台彻底删除 (软删 + 清所有 group_platform 关联)

## 已知 (main 静态调查)
- 链路: `Groups.tsx::confirmDeletePlatform` (line 153-159) → `cards.handleDelete(id)` → `usePlatformCards::handleDelete` (line 188-190) → `platformApi.delete(id)` → `invoke("platform_delete", {id})` → `db::delete_platform`
- **疑似根因 1**: `usePlatformCards.handleDelete` 实现为 `try { await platformApi.delete(id); } catch (e) { console.error(e); }` — **吞错误无 toast**, 删除失败用户无感知, `load()` 刷新后平台重新出现 (但 group_platform 关联可能已被另一路径清掉, 造成「移出分组但平台还在」假象)
- **疑似根因 2**: `db::delete_platform` 事务 (UPDATE deleted_at + DELETE group_platform) 若中途失败, 事务回滚——两步都应回滚。但若实际观察是「关联清了 / 平台未删」, 说明事务一致性被破坏, 或前端有其它清理路径与 delete 并发
- **疑似根因 3**: `handleGroupRemovePlatform` (line 127-141) 走 `groupApi.setPlatforms(gid, remaining)` 路径——可能在 confirmDeletePlatform 之前被某事件触发 (e.g. modal 外的 card action), 实际只重设了 group_platform 而没调 platformApi.delete

## 任务目标 (bug-hunt)
**复现 → 定位真实根因 → 最小修复 → 验证**。覆盖三层:
1. 前端: `Groups.tsx` / `GroupListView.tsx` / `usePlatformCards.ts` / `usePlatformsState.ts` 的 handler 真实调用链
2. 后端: `commands::platform_delete` / `db::delete_platform` 事务行为
3. 跨层: invoke 字段名 (id 类型 u64 vs number) / serde

## 改动 (待 bug-hunt 定位后填)
- TBD (推测: `usePlatformCards.handleDelete` 错误处理 + 可能的 handler 绑定错位)

## Acceptance
- [ ] 复现: 单分组平台点删 → modal → 删 → 平台从 DB 软删 (deleted_at>0) + 所有 group_platform 关联清空 + Platforms 主页不再可见
- [ ] 删除失败时**有 toast 反馈** (不再静默吞错误)
- [ ] cargo test + yarn build + yarn test 全绿
- [ ] 回归: 多分组共享平台的「仅移出本组」路径仍正确 (不误删平台)

## Out of Scope
- 「删除平台」vs「移出分组」的 UX 重设计 (YAGNI, 现有二分逻辑保留)
- 平台软删后的 purge 物理删 (已有 `purge_auto_disabled`)

## 依赖
- 无 (与 locale-zh-hans-rename / 已 archive 的 defaults task 文件集不相交)
