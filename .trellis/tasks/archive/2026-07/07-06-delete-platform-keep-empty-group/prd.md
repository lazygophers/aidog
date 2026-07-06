# delete_platform 保留空组不连带 force_delete_group 孤儿 auto 组

## Goal

当前 `delete_platform` (platform_lifecycle.rs:29) 删平台后连带 `force_delete_group` 清孤儿 auto 组（line 45-62）。用户期望：删平台本身 + 清所有 group_platform 关联，但**保留分组**（即使变空），让用户手动决定组去留。

证据：Groups 页单组平台点删除 → 弹窗「删除平台」→ 实际连带同名 auto 组消失。用户期望组保留。

## What I already know

- **bug 点**: `src-tauri/src/gateway/db/platform_lifecycle.rs:45-62` ② 步
  - 查 `auto_from_platform = 该平台 id` 的 auto 组 → 若组内空（get_group_platforms 空）→ `force_delete_group` 删组
- **调用链**: 
  - Groups 页 `confirmDeletePlatform` (Groups.tsx:153) → `cards.handleDelete(pid)` → `platformApi.delete` → invoke `platform_delete` cmd → `db::delete_platform`
  - `purge_auto_disabled` (platform_lifecycle.rs:90) 复用 `delete_platform`，purge 路径同样连带删组
- **现有测试**: `delete_platform_preserves_groups_with_other_members` (test_platform_lifecycle.rs:32) 测「有其他成员的组保留」；**未测「孤儿 auto 组保留」**（因为现行为是删，需新增反转测试）
- **前端提示**: GroupListView.tsx:264-274 弹窗文案「删除平台」「彻底删除该平台及其所有关联」—— 已暗示只删平台，**实际行为（连带删组）反而与文案矛盾**，修复后行为对齐文案
- **force_delete_group 定义**: group_platform.rs:6（生产路径仅 delete_platform 调用，test_settings.rs:62 测试直接调）
- **auto_from_platform**: 创建平台时 (commands/platform.rs:24) 自动建同名 auto 组，`auto_from_platform = platform.id.to_string()`

## Requirements

- `delete_platform` 删平台 + 清所有 group_platform 关联，**不删任何分组**（含孤儿 auto 组）
- 空 auto 组保留 `deleted_at=0`，前端 Groups 页正常展示空组卡片（已有能力，手动组本就可能空）
- `purge_auto_disabled` 复用 `delete_platform`，purge 路径同步受益（不再连带删组）
- 用户可手动删空组（已有 `handleDeleteGroup` Groups.tsx:288）
- force_delete_group 函数保留（其他场景仍可用），仅 delete_platform 不再调

## Acceptance Criteria

- [ ] `delete_platform(p)` 后：platform 软删 + 所有 group_platform 关联清空 + **同名 auto 组仍存在**（deleted_at=0）
- [ ] 空 auto 组在前端 Groups 页正常显示（无成员卡片）
- [ ] `purge_auto_disabled` 路径同样保留空组（复用 delete_platform 自动对齐）
- [ ] 现有测试 `delete_platform_preserves_groups_with_other_members` 不回归（有其他成员的组仍保留）
- [ ] 新增测试 `delete_platform_keeps_orphan_auto_group_empty`（孤儿 auto 组保留 + 组内空）
- [ ] cargo clippy 0 new warning，cargo test 全绿
- [ ] spec 沉淀：delete_platform 契约（删平台保留所有分组，禁连带删孤儿 auto 组）

## Definition of Done

- platform_lifecycle.rs:45-62 ② 步删除（或改为 no-op + 注释）
- 测试更新：现有断言「孤儿组被删」反转为「保留」+ 新增保留测试
- spec PATCH（若 backend spec 有 platform lifecycle 段）或 sediment 新建

## Technical Approach

### platform_lifecycle.rs 删 ② 步

```rust
pub fn delete_platform(db: &Db, id: u64) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        // ① 软删平台 + 物理清所有 group_platform 关联。**保留所有分组**（含孤儿 auto 组），
        //    让用户手动决定空组去留（已有 handleDeleteGroup）。auto 组同名空卡前端正常展示。
        db.call_traced(None, __db_caller, move |conn| {
            let tx = conn.transaction()?;
            tx.execute("UPDATE platform SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])?;
            tx.execute("DELETE FROM group_platform WHERE platform_id = ?1", params![id as i64])?;
            tx.commit()?;
            Ok(())
        }).await.map_err(|e| format!("delete platform: {e}"))?;

        db.invalidate_groups_cache();
        Ok(())
    }
}
```

### 测试更新

- `test_platform_lifecycle.rs` 现有 `delete_platform_preserves_groups_with_other_members` 不动（仍验有成员组保留）
- 新增 `delete_platform_keeps_orphan_auto_group_empty`：
  - 建平台 p → auto 组 g（auto_from_platform=p.id）
  - delete_platform(p)
  - 断言：p 软删 / group_platform 清空 / **g 仍存在 deleted_at=0** / g 组内 get_group_platforms 空
- 若现有测试有断言「孤儿 auto 组被删」（grep test 文件确认）→ 反转为保留

### spec sediment

backend spec 若无 platform lifecycle 段 → 新建 `platform-lifecycle.md`：
- delete_platform MUST 软删 platform + 清所有 group_platform + **禁连带删任何分组**
- 空 auto 组保留语义（用户手动清）
- purge_auto_disabled 复用 delete_platform 同步对齐

## Decision (ADR-lite)

**Context**: delete_platform 连带删孤儿 auto 组，与前端「删除平台」提示文案矛盾，用户期望组保留。
**Decision**: delete_platform 仅删平台 + 清关联，保留所有分组（含空 auto 组）。用户手动清空组。
**Consequences**: 数据库可能累积空 auto 组（已软删源平台）；前端 Groups 页显示空组卡片；用户用 handleDeleteGroup 手动清理。force_delete_group 函数保留供其他场景。

## Out of Scope

- 自动清理空 auto 组（YAGNI，用户手动删）
- 空组视觉特殊标识（已有空组展示能力）
- purge_disabled 行为独立调整（复用 delete_platform 自动受益）

## Technical Notes

- bug 点: platform_lifecycle.rs:45-62
- 调用链: Groups.tsx:153 confirmDeletePlatform → usePlatformCards.ts:188 handleDelete → api/platforms.ts:197 platformApi.delete → commands/platform.rs:193 → db::delete_platform
- 测试: test_platform_lifecycle.rs
- 前端文案: GroupListView.tsx:264-274（已暗示只删平台，修复后行为对齐）
