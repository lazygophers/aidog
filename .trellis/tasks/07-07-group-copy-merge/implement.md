# implement.md: 分组列表项复制按钮合并

> 配合 PRD。轻量：1 组件扩 + 1 调用点改 + i18n。

## 执行层

- 载体: main 派 trellis-implement（轻量，内联直做）
- worktree: 无
- 并行: 禁（单文件链）
- 门禁: `yarn build` + `node scripts/check-i18n.mjs`

## 改动清单

### 步骤 1 — CopyButton 扩 props（D1）

`src/components/shared/CopyButton.tsx`：

1. 加类型：

```ts
export interface CopyMenuItem {
  key: string;
  label: string;
  text: string;
  icon?: ReactNode;
}
```

2. `CopyButtonProps` 加 optional：

```ts
menu?: CopyMenuItem[];
defaultLabel?: string;
hoverLabel?: string;
```

3. 组件内：
- `menu` 缺失 → 原逻辑不动
- `menu` 传入：
  - state: `open` / `hovered`
  - click → `setOpen(true)`（不 writeText）
  - 菜单选项 click → `writeText(item.text)` + copied 反馈 + `setOpen(false)`
  - label = `hovered ? (hoverLabel ?? defaultLabel) : defaultLabel`
  - hover 态 onMouseEnter / onMouseLeave 切 `hovered`

4. 菜单渲染（menu 模式）：

```tsx
{open && menu && createPortal(
  <div style={{
    position: "fixed",
    top: rect.bottom + 4,
    left: rect.right - menuWidth,  // 右对齐按钮
    zIndex: 1000,
  }} className="glass-elevated" ...>
    {menu.map(item => (
      <button onClick={() => { writeText(item.text); copied feedback; close; }}>
        {item.icon}{item.label}
      </button>
    ))}
  </div>,
  document.body
)}
```

5. 边界翻转：`rect.bottom + menuHeight > window.innerHeight` → 上方展开（`top: rect.top - menuHeight - 4`）。

6. 关闭：`useEffect` mousedown 外点 + Esc keydown（open 时挂载）。

### 步骤 2 — GroupListItem 合并（D2）

`src/pages/Groups/GroupListItem.tsx:144-146`，3 个 CopyButton 替换为 1 个：

```tsx
<CopyButton
  text={group.group_key}
  defaultLabel={t("group.copyCommand", "复制启动命令")}
  hoverLabel={t("group.copyKeyLabel", "复制密钥")}
  menu={[
    { key: "key", label: t("group.menuCopyKey", "API Key"), text: group.group_key },
    { key: "claude", label: t("group.menuCopyClaude", "Claude 启动命令"),
      text: buildClaudeCommand(group.group_key),
      icon: <img src={claudeIcon} width={14} height={14} alt="Claude" /> },
    { key: "codex", label: t("group.menuCopyCodex", "Codex 启动命令"),
      text: buildCodexCommand(group.group_key, group.env_vars),
      icon: <img src={codexIcon} width={14} height={14} alt="Codex" /> },
  ]}
/>
```

旁侧 stats / test / add 按钮不动。`buildClaudeCommand` / `buildCodexCommand` / `claudeIcon` / `codexIcon` 现有 import 复用。

### 步骤 3 — i18n（D3）

`src/locales/*.json`（8 个）加 key：

| key | zh-Hans 默认 |
|---|---|
| `group.copyKeyLabel` | 复制密钥 |
| `group.menuCopyKey` | API Key |
| `group.menuCopyClaude` | Claude 启动命令 |
| `group.menuCopyCodex` | Codex 启动命令 |

`group.copyCommand`（"复制启动命令"）已存在，复用作 defaultLabel。

## 自检

`✅ lint=无 type=yarn build过 test=N/A TODO=0 验收物=CopyButton 扩 menu + GroupListItem 三合一 + i18n 8 locale`

## 失败处理

- yarn build 报 TS 错：CopyButton 新 props 全 optional，检查调用点传参类型
- 菜单定位偏：dev 工具查 `getBoundingClientRect` + viewport 翻转逻辑；liquid-glass 下确认 createPortal 生效（非 absolute 退化）
- hover 文字不切换：触屏 / `:hover` 伪类无关，确认 React state `hovered` 切换（onMouseEnter/Leave）
- check-i18n 红：对照 4 新 key 逐 locale 补
- 现有调用点回归：grep `CopyButton` 所有调用点（Home/GroupEditPanel/GroupListView），确认未传 menu 走原逻辑
