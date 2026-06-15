# 通知模板多预设快捷选择

## 需求（用户确认）
1. 为每个通知类型提供**多条预设模板**供快捷选择（用户确认：每类型 3-4 条，我按场景设计）。
2. 用户**手动点选**某预设 → 填入编辑框；之后**手动修改只改自己的 template 值，不污染预设**（预设不可变）。
3. **禁止通知内容为空** → 落实为「**清空即回退预设**」：用户清空输入框时自动填回该类型默认预设文本，确保 template 始终非空。

> 前置：本任务在 remove-notif-custom（commit 72fb98d，4→3 类型）之后做。当前类型 = **task_complete / waiting_input / error**（无 custom）。

## 现状（事实）
- `src/components/settings/NotificationSettings.tsx`：
  - `NOTIF_TYPES = ["task_complete","waiting_input","error"]`（:19 附近）。
  - `NOTIF_DEFAULT_TEMPLATES: Record<NotifType,string>`（:33-37）逐字镜像后端 `models.rs::default_template`（含双向注释），值 = `{project} 完成 / {project} 等待用户输入 / {project} 出错`。当前用于 textarea placeholder（:462 附近）。
  - template textarea（:448-462 区）`value={ts.template}` + `onChange updateType(type,{template})`；persist 走 functional update + debounce + 失败回滚（commit 9379b37，**勿破坏**）。
  - `TEMPLATE_VARS = ["{project}","{status}","{time}","{session}","{group}"]`：可用变量。
- 渲染层 `notification.rs::render`：空 template → default_template 兜底（commit dfb7671，品牌兜底 aidog）。**保留作双保险**，但 UI 层「清空即回退」后正常不会再触发空。

## 实现要点（前端为主，后端通常无改动）

### 1. 预设常量（不可变）
新增 `NOTIF_TEMPLATE_PRESETS: Record<NotifType, string[]>`，每类型 3-4 条，**`[0]` 必须 === `NOTIF_DEFAULT_TEMPLATES[type]`**（默认预设 = 后端镜像默认，保持渲染/UI/回退一致）。建议内容（实现可微调用词，但变量名须用 TEMPLATE_VARS 合法占位）：

```
task_complete:
  "{project} 完成"                      // [0] 默认（= NOTIF_DEFAULT_TEMPLATES）
  "✅ {project} 任务已完成"
  "{project} 已完成 · 状态 {status}"
  "{project} 全部任务跑完，详见日志"
waiting_input:
  "{project} 等待用户输入"               // [0] 默认
  "⌛ {project} 需要你确认"
  "{project} 暂停 · 等待 {status}"
  "{project} 卡在交互步骤，请回到终端"
error:
  "{project} 出错"                       // [0] 默认
  "❌ {project} 执行失败"
  "{project} 报错 · {status}"
  "{project} 运行中断，请查看日志"
```
预设正文 **zh 硬编码非 i18n**（与 NOTIF_DEFAULT_TEMPLATES 同约定）。可让 `NOTIF_DEFAULT_TEMPLATES[type]` 直接取 `NOTIF_TEMPLATE_PRESETS[type][0]`（单一事实源，避免两处默认值漂移；但保留对后端 models.rs 的镜像注释）。

### 2. 快捷选择 UI（**2026-06-15 修订：并入输入框做 combobox 下拉，不要上方独立 chip 行**）
> 用户反馈：选择应在**输入框本身**（输入框同时支持下拉），不要在上方独立列表；输入框要能看到原始文本且可编辑。即**可编辑 combobox**：字段显示并可改当前 template 文本，右侧/内置一个下拉触发，展开列出该类型预设，点选填入。

- **删除** v1 的「textarea 上方独立 chip 列表」。
- 改为 **combobox**：保留可编辑文本字段（显示 `ts.template` 原始文本），加下拉能力列出该类型 `NOTIF_TEMPLATE_PRESETS[type]`，点选项 → `updateType(type,{template:preset})`。
- 实现方案（择优，倾向 a 以保多行编辑 + webview 稳定）：
  - **(a) textarea + 自定义下拉**（推荐）：保留 `<textarea>`（多行可编辑、显示原始文本），在其右上角放一个 `▾` 触发按钮；点击展开绝对定位的预设面板（列出预设文本，可截断 + title tooltip），点选填入并收起；点击外部/选中后收起。需 per-type「当前展开的下拉」开关 state（如 `openPresetType: NotifType | null`）。命中预设项高亮。
  - (b) `<input list>` + `<datalist>` 原生 combobox：最少代码、原生下拉 + 可编辑 + 显示文本，但单行（失多行）且 WKWebView/WebKitGTK datalist 表现有差异风险。若选此须实测 webview 下拉可用。
- 预设是 const，点选只**复制**文本进可编辑字段，编辑不回写预设 → 满足「修改不污染预设」。
- 文本直接显示（截断）免新 i18n key（**倾向**）；若加任何标签文案需 8 locale i18n。
- 点选/编辑/回填均复用现 `updateType`→`persist`（debounce/functional update/回滚，**勿破坏 9379b37**）。

### 3. 禁空 → 清空即回退预设
- template textarea **失焦（onBlur）时若值为空/纯空白** → `updateType(type, { template: NOTIF_TEMPLATE_PRESETS[type][0] })` 回填默认预设。
  - 用 onBlur 而非 onChange，避免用户编辑中途（暂时清空再输入）被打断。
- 确保回填走正常 persist（debounce/functional update），落库非空。
- 渲染层 default 兜底保留（双保险）。

## 验收
- `yarn build`（tsc+vite）通过；`yarn check:i18n` 通过（若加 chip 标签 i18n 则 8 locale 全补 + 无重复 key）。
- 后端若无改动：`cargo build` 不需重验（无碰）；若动了后端须 `cargo clippy`+`cargo test` 全绿。
- 行为：
  - 每类型显示 3-4 条预设，点选填入编辑框；
  - 编辑 textarea 不改变预设（预设 const）；切换不同预设给到干净预设文本；
  - 清空 textarea 失焦 → 自动回填默认预设，template 不为空；
  - `NOTIF_TEMPLATE_PRESETS[type][0] === NOTIF_DEFAULT_TEMPLATES[type]`（与后端 default_template 一致）。
- 不破坏 9379b37 的 persist（functional update + debounce + 失败回滚）。

## 失败处理
- 若决定让 NOTIF_DEFAULT_TEMPLATES 取 presets[0] 单一源，须保留对 models.rs 的跨层镜像注释（改预设 [0] = 改默认，须同步后端 default_template）→ 注释写清。
- chip i18n 取舍：默认走「显示预设文本免 i18n」；若 UI 需要标签，再加 8 locale，标注于回报。
- onBlur 回填与 debounce 交互异常（如回填被未决 debounce 旧值覆盖）→ 确保回填用 functional update 取最新态，必要时 flush debounce；测不通标 `需要:`。
