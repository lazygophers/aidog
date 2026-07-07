# implement.md: 分组内删平台语义重设计

> 配合 PRD。根因旁路：去掉 groupCountOf 决定行为，总弹 modal 让用户选。

## 执行层

- 载体: main 派 trellis-implement（轻量，2 文件 + i18n）
- worktree: 无
- 并行: 禁（共享 Groups.tsx state）
- 门禁: `yarn build` + `node scripts/check-i18n.mjs` + dev 手动验

## 改动清单

### 步骤 1 — handleGroupRemovePlatform 重构（D1）

`src/pages/Groups.tsx:144-150`：

```ts
// 改前：count<=1 弹 modal / count>1 直接移出
// 改后：总弹 modal（count + groupNames 作展示）
const handleGroupRemovePlatform = useCallback((p: Platform, gid: number) => {
  const groupCount = groupCountOf(p.id);
  const groupNames = details
    .filter(d => d.platforms.some(gp => gp.platform.id === p.id))
    .map(d => d.group.name);
  setRemoveTarget({ platform: p, gid, groupCount, groupNames });
}, [groupCountOf, details]);
```

`removeTarget` state 类型扩（`Groups.tsx:119`）：

```ts
const [removeTarget, setRemoveTarget] = useState<
  { platform: Platform; gid: number; groupCount: number; groupNames: string[] } | null
>(null);
```

`GroupListView.tsx:58` props 类型同步改。`removePlatformFromGroup` 保留（多组「移出本组」按钮用）。

### 步骤 2 — modal 重构（D2 + D3）

`src/pages/Groups/GroupListView.tsx:255-281`，按 `removeTarget.groupCount` 分支：

```tsx
{removeTarget !== null && createPortal(
  <div onClick={() => setRemoveTarget(null)} style={/* 现有遮罩 */}}>
    <div className="glass-surface" onClick={e => e.stopPropagation()} style={/* 现有 */}}>
      {/* 标题 */}
      <div style={{ fontSize: 15, fontWeight: 700 }}>
        {removeTarget.groupCount > 1
          ? t("group.deletePlatformMultiTitle", "移出或删除平台")
          : t("group.deletePlatformTitle", "删除平台")}
      </div>
      {/* 描述 */}
      <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>
        {removeTarget.groupCount > 1
          ? t("group.deletePlatformMultiDesc",
              "「{{name}}」属 {{count}} 个分组：{{groups}}。选择操作：",
              { name: removeTarget.platform.name, count: removeTarget.groupCount, groups: removeTarget.groupNames.join("、") })
          : t("group.deletePlatformConfirm",
              "「{{name}}」仅属此分组，移除将彻底删除该平台及其所有关联，且无法撤销。确认删除？",
              { name: removeTarget.platform.name })}
      </div>
      {/* 按钮 */}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <button className="btn btn-ghost" onClick={() => setRemoveTarget(null)}>
          {t("action.cancel", "取消")}
        </button>
        {removeTarget.groupCount > 1 && (
          <button className="btn btn-ghost" onClick={() => {
            removePlatformFromGroup(removeTarget.platform.id, removeTarget.gid);
            setRemoveTarget(null);
          }}>
            {t("group.removeFromGroupAction", "移出本组")}
          </button>
        )}
        <button className="btn btn-danger" onClick={confirmDeletePlatform}>
          {removeTarget.groupCount > 1
            ? t("group.deleteFromAllGroupsAction", "删除平台（全部组）")
            : t("group.deletePlatformAction", "删除平台")}
        </button>
      </div>
    </div>
  </div>,
  document.body,
)}
```

需把 `removePlatformFromGroup` 加进 `GroupListView` props（若未传）。检查 `GroupListView` props 解构，缺则从 `Groups.tsx` 透传。

### 步骤 3 — i18n（D4）

`src/locales/*.json`（8 个）加 key：

| key | zh-Hans 默认 |
|---|---|
| `group.deletePlatformMultiTitle` | 移出或删除平台 |
| `group.deletePlatformMultiDesc` | 「{{name}}」属 {{count}} 个分组：{{groups}}。选择操作： |
| `group.removeFromGroupAction` | 移出本组 |
| `group.deleteFromAllGroupsAction` | 删除平台（全部组） |

现有 `group.deletePlatformTitle` / `deletePlatformConfirm` / `deletePlatformAction` 复用（单组场景）。

### 步骤 4 — dev 手动验

```bash
yarn tauri dev
```

测：
1. 单组平台点删 → 弹单按钮 modal → 确认 → 平台从 Platforms 页消失（真删）
2. 多组平台（同平台加入 2 组）点删 → 弹双按钮 modal
3. 点「移出本组」→ 仅本组移除，平台仍在另一组 + Platforms 页
4. 点「删除平台（全部组）」→ 全组移除 + Platforms 页消失
5. 取消 → 无变化

## 自检

`✅ lint=无 type=yarn build过 test=N/A TODO=0 验收物=handleGroupRemovePlatform 总弹 modal + modal 单组/多组分支 + 多组双按钮接线 + i18n 4 key × 8 locale`

## 失败处理

- yarn build 报 TS 错：removeTarget 类型扩字段后，所有访问处同步（Groups.tsx + GroupListView.tsx）
- GroupListView 缺 removePlatformFromGroup prop：从 Groups.tsx 透传（检查 props 链）
- 多组场景按钮错位：CSS flex justify-end + gap，危险按钮末位 btn-danger
- check-i18n 红：对照 4 新 key 逐 locale 补
- dev 验单组仍移出不删：说明 count 仍 stale 走错分支 — 但新设计已总弹 modal（无 if/else），不应发生；若仍错查 confirmDeletePlatform 是否被覆盖
