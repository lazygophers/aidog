# Research: 前端现状 + 逐事件配置 UI 集成点

- **Query**: NotificationSettings.tsx 结构、api.ts 类型、UI 集成不冲突
- **Scope**: internal
- **Date**: 2026-06-15

## 前端文件
- `src/components/settings/NotificationSettings.tsx`（668 行）—— **另一任务正在改其 template combobox，只读勿改**。
- `src/services/api.ts`（871-998 段）—— 类型 + notificationApi。

## NotificationSettings.tsx 结构

### 常量区（顶部）
- `NOTIF_TYPES = ["task_complete","waiting_input","error"]`（:22）
- `NOTIF_FORMS`（:23）/`TTS_BACKENDS`（:24）
- `TEMPLATE_VARS = ["{project}","{status}","{time}","{session}","{group}"]`（:25）—— 新事件特有变量可在此按事件扩展提示
- `NOTIF_TEMPLATE_PRESETS`（:35 Record<NotifType,string[]>）
- `NOTIF_DEFAULT_TEMPLATES`（:61 Record<NotifType,string>）—— **跨层镜像 models.rs default_template，改后端须同步**
- `DEFAULT_TYPE_SETTING`（:67）/`DEFAULT_SETTINGS`（:77 含 `per_type:{}`）

### 组件 `NotificationSettingsTab`（:96）
- state：`settings`(:98) / `defaultHooks`(:102) / 各种 busy/modal。
- `hooksDisabled = !settings.enabled`（:104）—— 通知总开关关时 hook 开关强制 off。
- 加载：`notificationApi.getSettings()`(:117) + `getDefaultHooksEnabled()`(:123)。
- `persist(updater)`(:140)：functional update + debounce 落库（注意闭包竞态处理 :138-163）。
- `typeSetting(type)`(:175) / `updateType(type,partial)`(:178)：per_type 读写。
- 测试按钮：`handleTest`(:187)/`handleTestTts`(:198)/`handleTestPopup`(:207)/`handleTestBeep`(:215)。
- `handleToggleDefaultHooks`(:286)：调 `setDefaultHooksEnabled`，控制 `_aidog_hooks.enabled` 全分组生效（:590-598 区块）。
- 渲染：按 NOTIF_TYPES 循环每类型卡片（:415 `typeSetting(type)` + :509 template combobox）。

### 现有区块布局（渲染顺序）
1. 通知总开关 + TTS 设置
2. **按类型配置**（NOTIF_TYPES 循环，每类型：tts/popup/form/template + presets + 测试按钮）
3. **默认注入总开关**（`_aidog_hooks`，:590-598）「默认为所有分组注入通知 Hook」

注：单 group 注入按钮已删（:5 注释，API `injectHooks/removeHooks` 仍保留）。

## 「逐事件配置」UI 集成建议

### 放哪
新增**第 4 区块「逐事件触发」**，置于「按类型配置」之后、「默认注入总开关」附近（逻辑上：事件配置依赖 hook 注入开启）。或并入默认注入区块内作子项。

### 形态
- 全量约 30 事件列表（折叠/分组：精选默认 on 一组 + 其余默认 off 一组，见 05）。
- 每事件一行：`enabled` 开关 + `notif_type` 选择器（3 类型）+ 可选自定义文案输入（空 placeholder 显示该 type 的 default_template）。
- 复用 `persist` 写 `settings.per_event[event]`（新字段，见 02）。

### 不冲突要点
- per_event 与 per_type **正交**：per_type 管呈现，per_event 管「哪些事件开+走哪 type」。两区块互不覆盖。
- `_aidog_hooks` 总开关仍是「是否注入任何 hook」的硬闸；per_event 在总开关 on 时才有意义（UI 可在总开关 off 时禁用逐事件区，类似 hooksDisabled 模式）。
- **避免改 template combobox 区**（另一任务在改），新区块独立组件/独立 state 分支。

## api.ts 类型扩展点 — api.ts:871-998

- `NotifType`(:875) / `NotifForm`(:878) / `TypeSetting`(:884) / `NotificationSettings`(:896 含 `per_type:Record<string,TypeSetting>`)。
- **扩展**：`NotificationSettings` 加 `per_event?: Record<string, EventSetting>`，新增 `EventSetting {enabled, notif_type, template}` interface（镜像后端 02 的 struct）。
- `HookClient`(:995)、`notificationApi`(:942)：getSettings/setSettings/inject/remove/getDefaultHooksEnabled/setDefaultHooksEnabled/buildNotifyHooksFragment 已齐。逐事件配置走 setSettings 即可（整 blob），**无需新 command**（除非要单独的「列全量事件元数据」端点，可纯前端常量）。

## 实现影响（本主题）
- 改 `NotificationSettings.tsx`（加区块）—— 但需与另一任务协调（template 区勿动）。建议新区块拆独立子组件。
- 改 `api.ts`（加 EventSetting 类型 + NotificationSettings.per_event 字段）。
- 全量事件清单 + 默认 notif_type 映射建议作**前端常量**（见 05），无需后端端点。

## Caveats
- 另一任务并行改 template combobox，**merge 冲突风险高**（memory 多处记录并行改同文件灾难）。强烈建议逐事件区块拆成新文件 `components/settings/NotificationEventList.tsx`，NotificationSettings.tsx 仅加一行 import + 挂载，最小化交叉。
- i18n：新区块文案需补 7 语言 key（项目硬规 + check-i18n.mjs 防线，见 CLAUDE.md / memory frontend-i18n-coverage）。
