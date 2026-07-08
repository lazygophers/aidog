# 删除平台仅移除分组未删平台（回归，07-06 bug 复现）

## 现象（user report）
点「删除平台」→ 平台从分组移除，但平台**未真正删除**（仍在别处可见）。07-06 已修过同类 bug（archive `07-06-group-delete-platform-bug`），07-08 复现。

## 已排除（main 静态调查 + 后端测试验证）
- **后端 `db::delete_platform` 正确**（`platform_lifecycle.rs:29-52`）：单事务 `UPDATE platform SET deleted_at=now()` + `DELETE FROM group_platform WHERE platform_id=?`。测试 `r9_soft_delete_platform`（test_platform_lifecycle.rs:8）断言：删后 list 返回 0、get 返回 None、deleted_at>0。`delete_platform_preserves_groups_with_other_members` 断言 group_platform 残留 0。**后端软删+清关联完全正确**。
- **list_platforms 过滤 deleted_at=0**（platform.rs:166）；**get_group_platforms 两层过滤**（group_platform.rs:196 `deleted_at=0` + load_platforms_by_ids `deleted_at=0`）。软删平台不返回。
- **前端 handleDelete 链路**（usePlatformCards.ts:191）：`await platformApi.delete(id)` → `invoke("platform_delete",{id})`。07-06 已修吞错（现抛出+toast）。
- **command 注册**（startup.rs:49）正常。

## 疑点（bug-hunt 待定位）
1. **`groupCountOf` stale**（Groups.tsx:125）：基于 `details` 前端状态。若 stale 致 groupCount>1（实际单组）→ 多组 modal 显示「移出本组」+「删除平台（全部组）」。用户可能误点「移出本组」→ `removePlatformFromGroup`（groupApi.setPlatforms，只清关联不删平台）。需验证 details 刷新时机。
2. **id 传递错位**：handleDelete(id) 收到的 id 是否与 modal removeTarget.platform.id 一致？cards 闭包 stale？
3. **Platforms 主页 handleDelete 乐观更新掩盖**（usePlatformsState.ts:453）：乐观移除 + await delete。若 delete 成功但 `handleGroupsChanged` 刷新前用户切页，可能看到中间态。
4. **load() 缓存**：Groups 页 load() 是否返回缓存 groupDetails，未实际重查 DB？
5. **用户实际操作入口**：Platforms 主页 vs Groups 页？多组 vs 单组？点哪个按钮？

## 任务目标（bug-hunt 自主）
**复现 → 定位真实根因 → 最小修复 → 验证**。聚焦前端 Groups.tsx / GroupListView.tsx / usePlatformCards.ts / usePlatformsState.ts 的 handler 真实调用链 + groupCountOf 刷新时机。若静态无法定位，建测试平台跑 cargo test 模拟 invoke 链路 + 查 DB 状态。

## 改动范围（待定位后填）
- 推测：Groups.tsx groupCountOf 刷新保障 / modal 默认行为 / cards 闭包依赖

## Acceptance
- [ ] 复现：单分组平台点删 → modal → 删 → DB `platform.deleted_at>0` + `group_platform` 残留 0 + Platforms 主页/Groups 页均不可见
- [ ] 多组共享平台「移出本组」路径仍正确（不误删平台）
- [ ] 多组「删除平台（全部组）」路径真删平台
- [ ] cargo test + yarn build 全绿
- [ ] 删除失败有 toast（07-06 已修，回归验证）

## Out of Scope
- 「删除平台」vs「移出分组」UX 重设计
- 软删后 purge 物理删（已有 purge_auto_disabled）

## Technical Notes
- 上次修复：commit 850ddbfc（07-06，只改错误处理，根因未定位）
- 后端测试全过 → 聚焦前端
- spec: .trellis/spec/backend/platform-lifecycle.md（delete_platform 契约）
