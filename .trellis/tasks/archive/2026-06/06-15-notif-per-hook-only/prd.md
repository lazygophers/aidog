# 通知模块重构：仅保留逐 Hook 事件触发

## 需求（用户确认）
1. **移除「按类型配置」**（任务完成/等待输入/出错 三类的 UI 卡片 + 模板 combobox/预设），只保留「逐 Hook 事件触发」功能。
2. 每个 hook 事件可独立：**启用/关闭自身 + 语音(TTS) + 弹窗** + **自定义模板**。
3. **每个 hook 的模板入参不同**（不要统一），**默认模板独立设计**（每事件一套，非共用一个）。

## 设计（定）

### 数据模型（models.rs）
- **`EventSetting` 改为**：`{ enabled: bool, tts: bool, popup: bool, template: String }`（全 `#[serde(default)]`）。
  - **删 `notif_type` 字段**。`Default` = `{enabled:false, tts:true, popup:true, template:""}`（用户启用某事件时 TTS/弹窗默认都开）。
  - 向后兼容：旧 DB per_event 含 `notif_type`（serde 无 deny_unknown → 反序列化时忽略多余字段，OK）；旧缺 tts/popup → serde default true。
- **per-event 目录（每事件专属，核心）**：新增 `fn default_template_for_event(event: &str) -> &'static str`（每事件**独立默认模板**，见下「事件目录表」）。
- **`per_type` / `NotifType` / `TypeSetting` 保留内部最小**：仅供 ① Codex 路径（complete 脚本 POST type=task_complete，无 event）② /api/notify 裸 type 兼容 ③ default_title 弹窗标题兜底。**不再有 per_type 用户 UI**。`default_template()`（NotifType）保留供 type 路径。
- 保留 `CC_HOOK_EVENTS`(30) / `DEFAULT_ON_EVENTS`。删 `default_notif_type_for_event`（不再需要 event→type 映射，#[allow(dead_code)] 那个）及其测试。
- **`DEFAULT_ON_EVENTS`（2026-06-15 修订）= `{Stop, PermissionRequest}`**（移除 PreCompact/SessionEnd/Notification/SubagentStop 默认启用——用户指示这 4 个默认不启用，仍可手动开）。前后端镜像同步；改相关测试（DEFAULT_ON 子集断言）。

### 通用脚本（hooks.rs `build_event_notify_script`）
- **改为通用透传**：读 stdin JSON，把**所有顶层标量字段**（str/int/float/bool）塞进 vars（`for k,v in data.items(): if isinstance(v,(str,int,float,bool)): vars[k]=str(v)`）；额外派生 `project`=cwd basename、`session`=session_id（若有）。
- POST `/api/notify` `{event: hook_event_name, vars}`（不传 type，hook_event_name 缺失静默退）。endpoint/Bearer/异常静默沿用。
- 这样**每事件不同入参自动进 vars**，无需为每事件枚举字段；模板用 `{字段名}` 即可，未知占位 substitute_vars 原样忽略。

### dispatch（notification.rs）—— event 路径自包含
- event 命中 `per_event[event]` 且 `enabled`：
  - 通道直接取 EventSetting：`do_tts = settings.tts_enabled && es.tts`；`do_popup = es.popup`；**inbox 恒落库**（历史）。**不再经 notif_type/per_type/form**。
  - body 模板：`es.template` 非空 → 用它；否则 `default_template_for_event(event)`；都空兜底类型 default（防空，沿用 dfb7671 链）。
  - 弹窗标题：`vars["project"]` 非空用之，否则事件名或 "Notification"（default_title 兜底）。
  - render 复用（substitute_vars + default 兜底）。
- event 未命中/未启用/无 event（Codex/裸 type）→ **保留现有 type 路径**（notif_type/per_type/default_template），向后兼容不破坏 Codex。
- `DispatchResult` 字段不变（tts/popup/sound/inbox）；event 路径 sound 跟随 popup（弹窗自带系统音）。

### 注入（hooks.rs/lib.rs）—— 不变
- 注入泛化逻辑（遍历 enabled 事件挂 aidog-notify.py / 移除遍历全量目录）**保持**（commit de74237 已实现）。`enabled_hook_events` 仍按 per_event.enabled。
- Codex 块、ScriptPaths.complete **保持不动**。

### 前端
- **`NotificationSettings.tsx` 删除**：`NOTIF_TYPES` / `NOTIF_TEMPLATE_PRESETS` / `NOTIF_DEFAULT_TEMPLATES` / `DEFAULT_TYPE_SETTING` / `typeSetting`/`updateType` / combobox state(openPreset/useEffect 外部收起) / 按类型配置整块 UI（3 卡片 + combobox textarea）。**保留**：总开关 enabled、全局 TTS 开关 tts_enabled + backend 选择、默认注入 hook 总开关(defaultHooks)、macOS 授权引导按钮、挂载 `<NotificationEventList>`。
- **`NotificationEventList.tsx` 重写**：每事件一行/卡 = 启用开关 + TTS 开关 + 弹窗开关 + 模板输入（textarea，placeholder=该事件默认模板）+ **该事件可用入参提示**（chips/文字，列该事件专属 `{占位}`，**每事件不同**）。删 notif_type 下拉。默认集（DEFAULT_ON_EVENTS）初始 enabled。
- **per-event 目录前端常量**：`EVENT_CATALOG: Record<string,{defaultTemplate:string, vars:string[]}>`，逐字镜像后端 `default_template_for_event` + 入参表，跨层注释。
- `api.ts`：`EventSetting` 改 `{enabled,tts,popup,template}`（删 notif_type）。`NotificationSettings` 的 per_type 可保留类型（后端仍有）但 UI 不用。
- 持久化复用 setSettings（per_event）。

### i18n
- 删/调整「按类型」相关文案 key（若仅该处用）；新增/调整逐事件区文案（TTS/弹窗/模板/可用入参标签）8 locale 全补。事件名英文原样。加 key 后 **Counter 查重**。

## 事件目录表（每事件专属默认模板 + 专属可用入参）
> 通用入参（所有事件都有）：`{project}`(项目名) `{session}`(会话id) `{cwd}` `{hook_event_name}`。下表「专属入参」为各事件额外字段（来源 code.claude.com/docs/zh-CN/hooks stdin）。默认模板 zh、各自独立。脚本通用透传所有标量字段故均可用；缺失字段占位由 substitute_vars 忽略（实现保证不残留裸 `{x}`：见失败处理）。

| 事件 | 默认模板（独立设计） | 专属入参 |
|---|---|---|
| SessionStart | `{project} 会话开始` | source, model, agent_type, session_title |
| Setup | `{project} 初始化（{trigger}）` | trigger |
| InstructionsLoaded | `{project} 已加载 {memory_type}` | file_path, memory_type, load_reason |
| UserPromptSubmit | `{project} 收到新指令` | prompt |
| UserPromptExpansion | `{project} 展开命令 {command_name}` | command_name, command_args |
| MessageDisplay | `{project} 消息更新` | turn_id, final |
| PreToolUse | `{project} 即将执行 {tool_name}` | tool_name |
| PermissionRequest | `{project} 请求授权：{tool_name}` | tool_name |
| PermissionDenied | `{project} 拒绝 {tool_name}：{reason}` | tool_name, reason |
| PostToolUse | `{project} {tool_name} 完成（{duration_ms}ms）` | tool_name, duration_ms |
| PostToolUseFailure | `{project} {tool_name} 失败：{error}` | tool_name, error |
| PostToolBatch | `{project} 批量工具完成` | （array，无标量特有） |
| Notification | `{project}：{message}` | message, type |
| SubagentStart | `{project} 子代理 {agent_type} 启动` | agent_type |
| SubagentStop | `{project} 子代理 {agent_type} 完成` | agent_type |
| Stop | `{project} 任务完成` | （无特有） |
| StopFailure | `{project} 中断：{error_message}` | error_code, error_message |
| TeammateIdle | `{project} 队友 {teammate_id} 空闲` | teammate_id, status |
| TaskCreated | `{project} 新建任务：{task_name}` | task_id, task_name |
| TaskCompleted | `{project} 任务完成：{task_name}` | task_id, task_name |
| ConfigChange | `{project} 配置变更（{config_source}）` | config_source |
| CwdChanged | `{project} 切换目录：{new_cwd}` | old_cwd, new_cwd |
| FileChanged | `{project} 文件变更：{file_path}` | file_path, change_type |
| WorktreeCreate | `{project} 创建 worktree` | worktree_path |
| WorktreeRemove | `{project} 移除 worktree` | worktree_path |
| PreCompact | `{project} 即将压缩上下文（{compact_reason}）` | compact_reason, context_size |
| PostCompact | `{project} 压缩完成` | context_reduction_ratio |
| Elicitation | `{project} {server_name} 请求输入` | server_name, tool_name |
| ElicitationResult | `{project} {server_name} 已响应` | server_name |
| SessionEnd | `{project} 会话结束（{end_reason}）` | end_reason, duration_ms |

> 实现可微调措辞，但**每事件模板必须各自独立、用其专属入参**，禁所有事件共用一个统一模板。前后端目录逐字镜像。

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零项目 warning，删 notif_type 后所有引用处编译穷尽）+ `cargo test`（per_event 新结构 + dispatch event 路径 tts/popup 直控 + 向后兼容旧 per_event(含 notif_type) 反序列化 + Codex 回归 全过）。
- `yarn build` + `yarn check:i18n` 过；locale 无重复 key。
- 行为：
  - 设置页**无**「按类型配置」区；只有逐事件列表（每行 启用+TTS+弹窗+模板+专属入参提示）+ 全局开关/TTS backend/默认注入/授权引导。
  - 各事件默认模板**互不相同**、用各自入参；UI 入参提示每事件不同。
  - 启用事件 → 触发 → 该事件 tts/popup 独立生效；模板渲染用该事件实际 stdin 字段。
  - Codex notify 不受影响（回归）。
  - 旧配置（per_event 带 notif_type）加载不报错（忽略旧字段 + tts/popup 取默认）。

## 失败处理
- 删 notif_type 致编译错（EventSetting 各引用）→ 逐个改到 event 自含逻辑。
- 模板裸 `{占位}` 残留（事件未提供该字段）→ 默认模板尽量只用「该事件必有」字段；可选字段缺失时 substitute_vars 保留 `{x}` 字面会难看 —— **实现给 substitute_vars 一个「缺失占位替换为空串」选项 OR 默认模板只用高确定字段**（如 Stop 仅 {project}；SubagentStop 用 {agent_type} 但 CC 该事件确有）。择一，回报采用策略。
- 脚本通用透传遇嵌套对象 → 跳过（只塞标量）。
- 向后兼容：旧 per_event JSON 测试反序列化断言。
- 门禁红修到绿；范围外标 `需要:`。
