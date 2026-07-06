---
updated: 2026-07-06
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# Platform Lifecycle

何时被读: 任何改动 `delete_platform` / `purge_auto_disabled_platforms` / 平台-分组关联清理的任务
谁读: main / sub-agent
不遵守的代价: 删平台连带销毁分组 → 用户数据无声丢失、空 auto 组消失、与前端文案矛盾

---

## delete_platform 契约

`delete_platform(db, id)`（`src-tauri/src/gateway/db/platform_lifecycle.rs`）的强制语义：

1. **软删 platform 行**（`UPDATE platform SET deleted_at = now() WHERE id = ?`）。
2. **物理清所有 `group_platform` 关联**（`DELETE FROM group_platform WHERE platform_id = ?`）。
3. **禁连带销毁任何分组**——含孤儿 auto 组（`auto_from_platform = id`）。
   - 单事务保证步骤 1+2 原子，不留指向已删平台的悬空关联。
   - `db.invalidate_groups_cache()` 刷缓存。
4. **保留所有分组**（`deleted_at = 0`），即便组内变空：
   - 同名 auto 组变空卡，前端 Groups 页正常展示无成员卡片。
   - 用户用 `handleDeleteGroup`（Groups.tsx）手动清空组。
   - `force_delete_group` 函数仍保留供 `delete_group`（group.rs）等场景调用，**delete_platform 不再调用**。

### 理由

前端 GroupListView.tsx 弹窗文案「删除平台」「彻底删除该平台及其所有关联」已暗示只删平台，原行为（连带删孤儿 auto 组）反而与文案矛盾。修复后行为对齐文案，并尊重用户对分组去留的最终决定权。

## purge_auto_disabled_platforms

复用 `delete_platform` 的语义，**不重写关联清理逻辑**：

- **全局（`group_id = None`）**：删全库 `status='auto_disabled'` 且 401/403 + 已过期平台，逐个 `delete_platform`。`unassigned_ids` 始终空。
- **分组级（`group_id = Some(gid)`）**：
  - 独占本分组（跨全库活跃成员数 ≤ 1）→ `delete_platform`（平台软删 + 关联全清，**分组保留**）。
  - 共享（成员数 > 1）→ 仅 `DELETE FROM group_platform WHERE group_id=? AND platform_id=? AND deleted_at=0`，platform 行保留。
- 共享判定必须 `deleted_at=0` 过滤当前活跃关联，避免软删残留误判独占。
- **402 / 429-配额等可恢复** auto_disabled 平台保留（充值/升级后自愈），仅 401/403（key 失效）+ 过期进 purge。

## purge_old_soft_deleted_platforms

定时任务（每日）：物理删除 `deleted_at > 0 AND deleted_at < now() - older_than_secs` 的 platform 行。
- `delete_platform` 软删时已物理清所有关联，此处仅 DELETE 行，无悬空关联风险。
- 分组保留由 `delete_platform` 当时已保证，此处不重做。

## 测试契约（test_platform_lifecycle.rs）

- `delete_platform_preserves_groups_with_other_members`：手动组 / 含其他成员的 auto 组 / 孤儿 auto 组三类分组在删平台后均保留。
- `delete_platform_keeps_orphan_auto_group_empty`：纯孤儿 auto 组保留为空卡（`deleted_at=0` + 组内空 + 无关联残留）。
- `r9_soft_delete_platform`：软删基础语义（list 不含 / get None / 行物理保留 `deleted_at > 0`）。
