# Research: Groups 单组平台「删除平台」实际只移除分组 — 根因定位

- **Query**: 定位「Groups 分组页，单组平台，点 modal「删除平台」单按钮，平台实际只从当前分组移除未彻底销毁」bug 的根因
- **Scope**: internal
- **Date**: 2026-07-10

---

## 代码链路图（前端 → 后端 SQL 全环）

```
[用户在 Groups 视图点平台卡 ×]
  └─ PlatformCard onDelete(id)
     └─ makeGroupCardActions(gid).onDelete  (src/pages/Groups.tsx:200-203)
        └─ handleGroupRemovePlatform(p, gid)  (Groups.tsx:150-167)
           ├─ await groupDetailApi.list()  → invoke("group_detail_list")  [走后端缓存]
           │  └─ 后端 list_group_details  (aidog_core/src/gateway/db/group_platform.rs:339-364)
           │     · 命中 db.1.group_details 缓存即返 clone；miss 时重建并写缓存
           │  └─ 算 groupCount = fresh.filter(d => d.platforms.some(gp => gp.platform.id === p.id)).length
           └─ setRemoveTarget({ platform:p, gid, groupCount, groupNames })
              └─ 渲 modal  (src/pages/Groups/GroupListView.tsx:257-301)
                 · groupCount > 1 → 双按钮（移出本组 + 删除全部）
                 · groupCount ≤ 1 → 单按钮「删除平台」
                 · 单按钮 onClick = confirmDeletePlatform

[用户点单按钮「删除平台」]
  └─ confirmDeletePlatform()  (Groups.tsx:171-185)
     ├─ await cards.handleDelete(target.platform.id)
     │  └─ usePlatformCards.handleDelete  (src/components/platforms/usePlatformCards.ts:191-193)
     │     └─ await platformApi.delete(id)
     │        └─ invoke("platform_delete", { id })  (src/services/api/platforms.ts:297)
     │           └─ 后端 platform_delete command  (commands_platform/src/platform.rs:191-195)
     │              └─ db::delete_platform(&db, id)  (aidog_core/src/gateway/db/platform_lifecycle.rs:29-51)
     │                 · tx1: UPDATE platform SET deleted_at = now() WHERE id = ?  ← 真软删
     │                 · tx2: DELETE FROM group_platform WHERE platform_id = ?     ← 清所有分组关联
     │                 · db.invalidate_groups_cache()  ← 连带清 group_details 缓存
     │                 · list_platforms SQL 过滤 `WHERE deleted_at = 0` (db/platform.rs:166)
     │                    → 后续 list 不返该平台
     │
     ├─ setRemoveTarget(null)  [关 modal]
     ├─ load()  [useGroupData.load]  (src/pages/Groups/useGroupData.ts:156-183)
     │  · setDetails([]); setPlatforms([]);
     │  · await platformApi.list() → 后端已 deleted_at≠0 → 不返该平台 ✓
     │  · setPlatforms(upsert from fresh list)
     │  · await loadMore() → group_detail_list_paged → 不含该平台 ✓
     │  ⇒ Groups 视图（嵌入式）正确移除该平台
     │
     └─ onGroupsChanged?.()  [回调父级 Platforms 页]
        └─ usePlatformsState.handleGroupsChanged  (src/pages/platforms/usePlatformsState.ts:344-348)
           · 仅 setGroupDetails(await groupDetailApi.list())
           · ❌ 不重新拉 platforms 列表
           · effect 重建 platformMembership（删除平台因 group_platform 清空 → membership 无它）
           · standalonePlatforms = platforms.filter(p => !platformMembership.has(p.id))
             (usePlatformsState.ts:582-593)
             → 该平台仍在 stale `platforms` state，且已无 membership
             ⇒ 被归入 standalonePlatforms「未分组平台」段，平台以「未分组」身份继续可见
```

---

## 5 个调研问题逐项答

### Q1. 后端 `delete_platform` command 实现 — 真删平台还是只摘 group 关联？

**结论：真软删 platform 行 + 清所有 group_platform 关联 + 失效缓存。后端正确，无 bug。**

实现位置 `src-tauri/crates/aidog_core/src/gateway/db/platform_lifecycle.rs:29-51`：

```rust
pub fn delete_platform(db: &Db, id: u64) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    // ① 软删平台 + 物理清除该平台在所有分组（含手动组与 auto 组）的成员关系。
    db
        .call_traced(None, __db_caller, move |conn| {
            let tx = conn.transaction()?;
            tx.execute("UPDATE platform SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])?;
            tx.execute("DELETE FROM group_platform WHERE platform_id = ?1", params![id as i64])?;
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete platform: {e}"))?;
    // ② 保留所有分组（含孤儿 auto 组）
    db.invalidate_groups_cache();
    Ok(())
    }
}
```

- command 层 `src-tauri/crates/commands_platform/src/platform.rs:191-195` 仅日志包装 + 转发 `db::delete_platform`
- `list_platforms` SQL `SELECT ... FROM platform WHERE deleted_at = 0 ORDER BY sort_order, created_at` (`db/platform.rs:166`) → 软删后该行不再被任何 list 返回
- 连带清的表：仅 `group_platform`（单事务保证无悬空关联）；`platform` 行本身软删保留供审计 / 定时硬清理由 `purge_old_soft_deleted_platforms` (platform_lifecycle.rs:206-224) 超阈值 `DELETE FROM platform WHERE deleted_at > 0 AND deleted_at < ?`
- commit `026289e` 提的「连带清关联」即指 `DELETE FROM group_platform WHERE platform_id = ?` 这条，已生效

### Q2. `platformApi.delete` 前端封装 — command 名 + 是否传 group_id？

**结论：command 名 `platform_delete`，只传 `{ id }`（不传 group_id），后端按 id 全局软删。前端封装正确。**

`src/services/api/platforms.ts:297`：

```ts
delete: (id: number) => invoke<void>("platform_delete", { id }),
```

- 后端 command 签名 `pub async fn platform_delete(id: u64, db: State<'_, Db>)` (`commands_platform/src/platform.rs:191`) 仅收 `id`，无 group 维度
- `usePlatformCards.handleDelete` (`src/components/platforms/usePlatformCards.ts:191-193`) 仅转调：
  ```ts
  const handleDelete = useCallback(async (id: number) => {
    await platformApi.delete(id);
  }, []);
  ```
- `confirmDeletePlatform` (Groups.tsx:171-185) 经 `cards.handleDelete(target.platform.id)` 走的就是这条全局删除路径，不是「移出本组」路径（那条是 `removePlatformFromGroup` → `groupApi.setPlatforms`，仅重设本组成员集）

### Q3. modal `groupCount` 数据源 — 是否真走后端 / 缓存层 / 单组判定可靠性？

**结论：07-08 fix 后 `handleGroupRemovePlatform` 已改走 `groupDetailApi.list()`（后端缓存，写时 invalidate），单组 `groupCount===1` 判定在后端缓存正确失效时可靠。但 catch 分支仍回退到前端 stale `details`。**

`src/pages/Groups.tsx:150-167`：

```ts
const handleGroupRemovePlatform = useCallback(async (p: Platform, gid: number) => {
  let groupCount: number;
  let groupNames: string[];
  try {
    const fresh = await groupDetailApi.list();
    groupCount = fresh.filter(d => d.platforms.some(gp => gp.platform.id === p.id)).length;
    groupNames = fresh.filter(...).map(d => d.group.name);
  } catch {
    // 后端拉取失败 → 回退到前端 details（可能 stale）
    groupCount = groupCountOf(p.id);
    groupNames = details.filter(...).map(d => d.group.name);
  }
  setRemoveTarget({ platform: p, gid, groupCount, groupNames });
}, [groupCountOf, details]);
```

- `groupDetailApi.list` = `invoke("group_detail_list")` (`src/services/api/groups.ts:89-99`)，无前端缓存层（每次直发 IPC）
- 后端 `list_group_details` (`aidog_core/src/gateway/db/group_platform.rs:339-364`) **有内存缓存**：
  ```rust
  if let Ok(g) = db.1.group_details.read() {
      if let Some(cached) = g.as_ref() {
          return Ok(cached.clone());
      }
  }
  ```
- 缓存失效链：`delete_platform` / `set_group_platforms` / `move_group_platform` 等写操作均调 `db.invalidate_group_details_cache()` 或 `invalidate_groups_cache()`（后者连带清前者，`db/mod.rs:576-592`），故正常写后立即读必走重建 → count 准确
- **catch 回退分支**仍用 `groupCountOf(p.id)`（基于前端 `details`，分页/乐观更新未刷新时可 stale）— 仅在 IPC 失败时触发，非主路径
- 单组判定 `groupCount > 1`（GroupListView.tsx:267）可靠前提：后端缓存正常失效。证据：测试 `test_group_platform.rs::delete_platform 后缓存仍含已删平台 → 失效漏` 守护此不变量

### Q4. delete 后前端刷新链 — `load()` 拉什么 / platforms 是否同步刷新 / `onGroupsChanged` 触发 Platforms 页 reload 没？

**结论（关键）：`load()` 仅刷新 GroupsEmbedded 本地 state（`useGroupData` 内部）；`onGroupsChanged` 回调到父级 `usePlatformsState.handleGroupsChanged`，**只 refetch `groupDetails`，不 refetch `platforms`**。这是「平台以未分组身份残留」的直接原因。**

- `confirmDeletePlatform` 成功后 (Groups.tsx:174-177)：
  ```ts
  await cards.handleDelete(target.platform.id);
  setRemoveTarget(null);
  load();             // ← useGroupData.load，仅刷 GroupsEmbedded 本地
  onGroupsChanged?.(); // ← 回调父级
  ```

- `useGroupData.load` (`src/pages/Groups/useGroupData.ts:156-183`) 拉的数据：
  - `setDetails([])` + `setPlatforms([])` 清空
  - `await platformApi.list()` → 写回 `useGroupData.platforms`（局部 state）
  - `await loadMore()` → `group_detail_list_paged` 写 `details`
  - **这些都是 GroupsEmbedded 内部 state，不影响父级 Platforms 页的 `usePlatformsState.platforms`**

- `onGroupsChanged` 在 `usePlatformsState` 的实现 (`src/pages/platforms/usePlatformsState.ts:344-348`)：
  ```ts
  const handleGroupsChanged = async () => {
    try {
      setGroupDetails(await groupDetailApi.list());
    } catch { /* ignore */ }
  };
  ```
  - 仅 `setGroupDetails`，**不 setPlatforms / 不 platformApi.list**
  - effect 重建 `platformMembership`（基于 fresh groupDetails）→ 被删平台的 membership 条目消失（因其 group_platform 已被后端 DELETE）

- `standalonePlatforms` 派生 (`usePlatformsState.ts:582-593`)：
  ```ts
  const standalonePlatforms = useMemo(
    () => platforms
      .filter(p => !platformMembership.has(p.id))
      .filter(... searchQuery ...),
    [platforms, platformMembership, searchQuery],
  );
  ```
  - `platforms` state（父级 usePlatformsState）STALE 仍含被删平台
  - `platformMembership` 已不含被删平台（membership 被 effect 清掉）
  - ⇒ `!platformMembership.has(p.id)` 为 true ⇒ **被删平台被归入 standalonePlatforms**
  - ⇒ 用户在「未分组平台」段看到该平台 = 「只移除分组未彻底销毁」错觉

- 对照：Platforms 页自己删平台走 `usePlatformsState.handleDelete` (L455-486)：
  ```ts
  const handleDelete = async (id: number) => {
    platformsEpochRef.current++;
    setPlatforms(prev => prev.filter(x => x.id !== id));  // ← 乐观移除 platforms state
    try {
      await platformApi.delete(id);
      handleGroupsChanged();
      window.dispatchEvent(new Event("aidog-groups-changed"));
    } catch { /* rollback */ }
  };
  ```
  这条路径**有**乐观更新 `platforms`，所以从 Platforms 列表删不会复现本 bug。

- `usePlatformsState` 不监听任何「平台已删」窗口事件（搜遍 L615-630 仅 `aidog:platform` deep link + `aidog-platform-test-completed` 测试事件），故无法被动感知 GroupsEmbedded 内的删除

### Q5. 复现路径推断 — 根因排序

| 排序 | 根因候选 | 证据强度 | 反证 |
|---|---|---|---|
| **#1（最可能）** | **父级 `usePlatformsState.platforms` state 不刷新**：`onGroupsChanged` 回调仅 refetch groupDetails 不 refetch platforms，被删平台残留 `platforms` state，membership 被清后归入 `standalonePlatforms` 以「未分组」身份可见 | **强**：代码原文铁证。`handleGroupsChanged` (usePlatformsState.ts:344-348) 仅 setGroupDetails；`standalonePlatforms` (582-593) filter 平台不在 membership；`confirmDeletePlatform` (Groups.tsx:171-185) 不直接刷父级 platforms | 用户若切回纯 Platforms 视图后主动刷新（如重 search / 重挂载）会消失，但用户观察的「点删除后立即看」确实复现 |
| #2 | modal `groupCount` 仍走 catch 回退分支（前端 stale details）→ 单组场景误显双按钮 → 用户实际点了「移出本组」 | 中：catch 分支存在；但需 IPC `groupDetailApi.list()` 失败才触发，正常路径下后端缓存 invalidate 链有测试守护（`test_group_platform.rs:64-73`） | 用户描述明确为「单按钮」（modal 单按钮 = groupCount≤1），故此候选要求用户看错按钮 |
| #3 | 后端 `delete_platform` 未真删（只摘关联） | **极弱/排除**：`platform_lifecycle.rs:37` `UPDATE platform SET deleted_at` 铁证软删；`list_platforms` SQL `WHERE deleted_at = 0` 铁证过滤；测试 `test_platform_lifecycle.rs::r9_soft_delete_platform` 守护 | — |
| #4 | GroupsEmbedded 本地 `load()` 刷新失败 | 弱：`load()` 有 seq 守卫和 try/catch，且 `platformApi.list` 后端过滤正确 | 若此则 Groups 视图内卡片也不会消失，与「只移除分组」描述不符 |

---

## 修复方向建议（仅给 main/PRD 参考，不动手）

按根因 #1（最强证据）：

1. **首选**：`usePlatformsState.handleGroupsChanged` 在收到「平台增删」语义时连带 refetch `platforms`。最小改动 — 加一个独立信号（如 `onPlatformDeleted?.()` 回调或 `aidog-platform-deleted` window event），由 `confirmDeletePlatform` 触发，`usePlatformsState` 监听后跑 `platformApi.list()` + `setPlatforms` + `platformsEpochRef.current++`（保持 epoch 守卫一致）。不复用 `onGroupsChanged` 是因其语义仅「分组结构变」（移平台 / 增删组），平台本身未变时不应触发整列表重拉。

2. **备选**：`confirmDeletePlatform` 直接乐观更新父级 — 但 `usePlatformCards.handleDelete`（共用 hook）不应承担父级 state 责任，故建议在 Groups.tsx 层透传「platformDeleted」事件给 Platforms 页。

3. **兜底**：`usePlatformsState` 监听 `aidog-groups-changed` window event 时连带 refetch platforms — 简单但语义过宽（每次组结构变都重拉全量平台，浪费 IPC）。

辅助：
- 修复后建议加 e2e 测试覆盖「GroupsEmbedded 删单组平台 → 父 Platforms 页 standalonePlatforms 不含该 id」不变量
- catch 分支回退 stale details 的脆弱性（Q3）可顺带用「IPC 失败 → 强制走单按钮删除分支（按最保守语义）」加固，但优先级低于 #1

---

## Caveats / Not Found

- 未实际运行复现；结论基于代码静态分析 + 数据流推导。建议 main 用 `yarn tauri dev` 实测：单组平台点删除后，立即看下方「未分组」段是否出现该平台卡片，以最终验证根因 #1。
- `usePlatformsState.platforms` 的初始 mount load 由 `usePlatformsState` 内部触发（未细看其 mount effect），但本场景关键在于「删除后」不重拉，mount 行为不影响结论。
- 未深挖 `groupApi.setPlatforms` 在 `removePlatformFromGroup` 路径的全部分支（本 bug 主路径是 `confirmDeletePlatform` → `delete_platform`，与 `setPlatforms` 无关）。
