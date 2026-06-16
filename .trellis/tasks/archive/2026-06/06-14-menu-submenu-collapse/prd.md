# PRD: 修复侧栏子菜单展开后无法收起

## Bug
Sidebar 设置分组（settings）展开后，用户导航到其他顶级页（如 Platforms），再点击 settings header 试图收起 → 反而重新展开 + 跳回 settings/system。

## 根因
`src/components/Sidebar.tsx:255` onClick:
```js
if (!inThis) { onNavigate(children[0]); setExpandedNav(true); }
else { setExpandedNav(!expanded); }
```
当 group 已展开（expandedNav[id]=true）且 activeId 不在该组（inThis=false）时，点击走 `!inThis` 分支 → setExpandedNav(true) 覆盖为 true（已是 true）+ 跳转。无法收起。

## 修复
header 点击**始终 toggle expand**；仅当"展开 + 未在组内"时 navigate 到首个 child：
```js
const willExpand = !expanded;
setExpandedNav(id, willExpand);
if (willExpand && !inThis) onNavigate(children[0]);
```

## 验收
1. settings 展开 → 点 settings header → 收起 ✅
2. settings 展开 → 切到 Platforms → 点 settings header → 收起（不跳转）✅
3. settings 收起 → 点 settings header → 展开 + 跳 settings/system ✅
4. 单页 nav（platforms）点击正常跳转 ✅
