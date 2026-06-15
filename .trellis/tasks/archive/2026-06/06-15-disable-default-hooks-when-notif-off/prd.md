# PRD: 通知关闭时禁用「默认注入通知 Hook」开关

## 目标
通知总开关 (`settings.enabled`) 关闭时，「默认为所有分组注入通知 Hook」设置项强制显示关闭且禁用修改。

## 背景
- 通知总开关 `settings.enabled` (NotificationSettings state)
- defaultHooks toggle `src/components/settings/NotificationSettings.tsx:399-408`，onClick → `handleToggleDefaultHooks`
- 逻辑：通知关了，hook 注入无意义，该开关不应可改

## 范围
- ✅ toggle 视觉：`settings.enabled=false` 时强制 off 显示（`active = defaultHooks && settings.enabled`）
- ✅ toggle 交互：`settings.enabled=false` 时不可点击（onClick 守卫 + aria-disabled + 视觉 dimmed）
- ✅ 提示文案：disabled 时副标题补「需先开启通知」（i18n 新增 key）
- ⛔ 后端 defaultHooks 值不动（通知重开后恢复用户原设定）
- ⛔ 不改 handleToggleDefaultHooks 逻辑本身

## 产出
### `src/components/settings/NotificationSettings.tsx`
L399-408 toggle 块：
- `const hooksDisabled = !settings.enabled;`
- `className={\`toggle ${defaultHooks && !hooksDisabled ? "active" : ""} ${hooksDisabled ? "disabled" : ""}\`}`
- onClick: `if (!defaultHooksBusy && !hooksDisabled) handleToggleDefaultHooks();`
- aria-disabled / tabIndex / cursor not-allowed
- 副标题：hooksDisabled 时追加 `· 需先开启通知`

### i18n（8 语言）
新增 key `notif.defaultHooksDisabledHint` = "需先开启通知" / "Enable notifications first" / ...

## 验证
- 通知开 → toggle 正常可切
- 通知关 → toggle 显示 off + dimmed + 不可点 + 副标题提示
- 关通知 → 重开 → toggle 恢复用户之前值
- `yarn build` 通过
- check-i18n.mjs 8 语言全覆盖

## 依赖
无后端改动
