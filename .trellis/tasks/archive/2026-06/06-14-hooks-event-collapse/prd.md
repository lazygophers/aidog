# PRD: 修复 Claude Code hooks 单个事件无法收起

## Bug
Settings → Claude Code → Hooks：配置过 hook 的事件无法收起（▶ 点击无效）。

## 根因
`src/components/settings/editors.tsx` 两处（HooksSection ~4845 + HooksSectionInline ~5294）：
```js
const isExpanded = expandedEvent === eventId || groups.length > 0;
```
`|| groups.length > 0` 强制：有 group 的事件永远展开，`setExpandedEvent(null)` 失效。

## 修复
改 accordion 单值模型 → per-event 用户覆盖模型：
```js
const [userToggles, setUserToggles] = useState<Record<string, boolean>>({});
const isExpanded = eventId in userToggles ? userToggles[eventId] : groups.length > 0;
// onClick: setUserToggles(prev => ({ ...prev, [eventId]: !isExpanded }));
```
- 默认：有内容则开，无内容则关
- 用户点击独立 toggle 覆盖默认
- addMatcherGroup 后 setUserToggles[eventId]=true（确保新增事件可见）

## 验收
1. 有 hook 的事件 → 点 ▶ → 收起 ✅
2. 收起后 → 点 ▶ → 重新展开 ✅
3. 新增 hook 事件 → 自动展开 ✅
4. 多事件独立开/关互不影响 ✅
5. 两组件（HooksSection + HooksSectionInline）行为一致 ✅
