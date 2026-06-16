# claude code hook 事件通知：可配置多事件触发系统通知

## 需求（用户确认）
让用户为 claude code 的 hook 事件配置系统通知。
- **范围**：全量约 30 个官方 hook 事件**均可开**；UI 列全量，**默认仅精选 on，其余默认 off**。
- **映射**：**每个启用事件独立选通知类型**（复用现有 3 类型 task_complete/waiting_input/error 的 tts/popup/form 通道配置）**+ 可自定义文案**（per-event 模板，留空回退该类型模板）。
- 限 **claude code**（Codex 路径不动，仅 turn-complete 单事件，无逐事件需求）。

## 调研结论（research/ 已落 5 文件，关键引用）
- 现状：`hooks.rs::build_hook_script(type)` 生成 Python 脚本，type **编译期内插**，POST `{type,vars:{project}}` 不解析 stdin；`generate_hook_scripts` 落 2 脚本 complete(task_complete)/waiting(waiting_input)；`inject_claude_code_hooks` **硬编码** Stop→complete、Notification→waiting（hooks.rs:165-166）。
- 持久化：`NotificationSettings`（models.rs:1740，per_type:HashMap 全 serde default）整 JSON blob 存 DB（scope=notification/key=settings），**零 migration** 可加字段。
- /api/notify：`NotifyReq{type,content?,vars?}`（proxy.rs），dispatch→render→substitute_vars，**vars 透传任意 key**（substitute_vars 未知占位保留）。
- **Codex 耦合点（勿破坏）**：`inject_codex_notify`（hooks.rs:246）写 `notify=[complete_script]`，lib.rs:1232/2384 依赖 `ScriptPaths.complete`。泛化 ScriptPaths **必须保留 complete 脚本/command**。

## 设计（定）

### 数据模型（models.rs）
- 新增 `EventSetting { enabled: bool, notif_type: NotifType, template: String }`（全 `#[serde(default)]`）。
- `NotificationSettings` 加 `#[serde(default)] per_event: HashMap<String, EventSetting>`（key = 事件名如 "Stop"/"SubagentStop"）。
- 新增「全量事件目录」常量 + 默认 type 映射 + 默认 on 集（见下「事件目录」）。旧配置无 per_event → 空 map → 前端按默认目录展示、用户开启才写入（默认集不硬写进 DB，展示层兜底）。

### 后端通知解析（notification.rs dispatch — 小改）
- `NotifyReq` 加 `event: Option<String>`（proxy.rs handler 透传给 dispatch）。
- dispatch 逻辑：
  - 若 `event` 存在且 `settings.per_event.get(event)` 命中且 `enabled`：`notif_type = es.notif_type`；通道/tts/popup 仍取 `type_setting(notif_type)`（复用类型通道配置）；body 模板 = `es.template` 非空则用它，否则回退 `type_setting(notif_type).template`，再回退 default_template（沿用 dfb7671 品牌兜底链）。
  - 若 event 不存在/未命中/未启用：维持现有按 `type` 路径（向后兼容，未知 type→TaskComplete）。
  - **禁空**沿用 render 既有 default_template 兜底（commit dfb7671），事件 body 永不空。

### 脚本（hooks.rs + lib.rs）—— **单通用脚本方案**
- 新增**一个**通用脚本 `aidog-notify.py`（落 ~/.aidog/scripts/）：
  - 读 **stdin JSON**，取 `hook_event_name`（→ event）、`cwd` basename（→ vars.project）、+ 一组已知事件特有字段若存在塞入 vars（`message`/`agent_type`/`agent_id`/`tool_name`/`reason`/`source`/`end_reason` 等，官方 docs 字段；缺失则跳过）。
  - POST `/api/notify` body `{"event": <hook_event_name>, "vars": {...}}`（**不传 type**，后端按 per_event 解析）。endpoint/鉴权推导沿用现脚本逻辑（ANTHROPIC_BASE_URL 剥后缀 + ANTHROPIC_AUTH_TOKEN Bearer）。异常静默吞。
  - **不需要命令行传参**（event 来自 stdin hook_event_name）→ 绕开「CC command 是否 shell 解析参数」风险。
- `ScriptPaths`：保留 `complete`（Codex 仍用，task_complete 语义）+ 新增 `event_notify`（通用脚本 command 串）。`generate_hook_scripts` 同时生成 complete + aidog-notify.py（waiting 可保留或并入；**Codex 依赖 complete 必须在**）。
- `inject_claude_code_hooks`：改为**遍历 `settings.per_event` 中 enabled 的事件** → 每个 `set_event_hook(event, event_notify_command)`（全部指向同一通用脚本 command）。不再硬编码 Stop/Notification。
- `remove_claude_code_hooks`：改为**遍历全量事件目录**移除 aidog 项（或按 marker 记录的已注入事件集）；`references_aidog_script` 靠单脚本文件名 `aidog-notify.py` 识别，天然全匹配。
- 注入调用方（lib.rs `do_sync_group_settings` :1126-1265 / `inject_hooks` / `set_default_hooks_enabled` / `build_notify_hooks_fragment`）适配新签名；**Codex 块（lib.rs:1226-1243 顶层 notify=[complete]）保持不动**。
- `do_sync_group_settings` 需把 `settings.per_event` 传入注入函数（当前只传 ScriptPaths）。

### 前端（拆新文件避冲突）
- **新建** `src/components/settings/NotificationEventList.tsx`：列全量事件目录，每事件一行 = 开关 + notif_type 下拉（3 类型）+ 可选文案输入（留空=用类型模板）。默认集展示为 on（未存 per_event 时按默认目录初始态）。改动经现有 notificationApi.setSettings 持久化（per_event 字段）。
- `NotificationSettings.tsx`：仅**挂载** `<NotificationEventList>`（最小改动，避开刚提交的 template combobox 区）。
- `api.ts`：`NotificationSettings` 加 `per_event?: Record<string, EventSetting>`；新增 `EventSetting` 类型。
- 事件名/默认映射前端需一份目录常量（镜像后端事件目录；**跨层注释指向后端**）。

### i18n + docs
- 新区块文案 key 7（实为 8 含 es-ES）语言全补，`yarn check:i18n` 过；**加 key 后用 Counter 查重**（防本会话反复出现的重复 key 坑）。
- 事件名本身用英文原样（CC 官方事件名，非翻译）；说明性文案走 i18n。
- docs Rspress 通知页视情补一句「hook 事件通知」（memory docs-site-i18n-coverage；无强制全语言，主语言 zh/en 优先）。

## 事件目录（全量 + 默认）
全量事件名（官方 docs，约 30）：SessionStart, Setup, InstructionsLoaded, UserPromptSubmit, UserPromptExpansion, MessageDisplay, PreToolUse, PermissionRequest, PermissionDenied, PostToolUse, PostToolUseFailure, PostToolBatch, Notification, SubagentStart, SubagentStop, Stop, StopFailure, TeammateIdle, TaskCreated, TaskCompleted, ConfigChange, CwdChanged, FileChanged, WorktreeCreate, WorktreeRemove, PreCompact, PostCompact, Elicitation, ElicitationResult, SessionEnd。

**默认 ON 精选集**（6 个）：`Stop` / `SubagentStop` / `Notification` / `PermissionRequest` / `SessionEnd` / `PreCompact`。
> SessionStart **默认 OFF**（research：噪音偏高），但在目录中可手动开。

**默认 notif_type 映射规则**（实现固化为常量表）：
- 含 Stop/Complete/End → task_complete
- 含 Failure/Denied/Error → error
- 含 Notification/Permission/Elicitation → waiting_input
- 其余 → task_complete

## 实现计划（单一交付，trellis-implement 内部按序，文件依赖故串行为主）
- **S1 后端模型**：models.rs EventSetting + per_event + 事件目录常量/默认映射。
- **S2 后端脚本+注入+dispatch**：hooks.rs 通用脚本 + 注入遍历泛化 + ScriptPaths 保 complete；lib.rs 调用方适配 + per_event 传入；notification.rs dispatch event 解析；proxy.rs NotifyReq.event。（依赖 S1）
- **S3 前端**：api.ts 类型 + 新 NotificationEventList.tsx + NotificationSettings 挂载。（依赖 S1 语义）
- **S4 i18n(+docs)**：8 locale 文案 key + 查重 + 可选 docs。（依赖 S3 文案定稿）
> S2、S3 文件不重叠可并行；S1 先行；S4 收尾。trellis-implement 自行编排。

## 验收
- `cd src-tauri && cargo build && cargo clippy --quiet`（零项目 warning）+ `cargo test`（全过，含新增 per_event/dispatch event 解析测试 + **Codex 注入不破坏**回归）。
- `yarn build`（tsc）+ `yarn check:i18n` 过；locale 无重复 key。
- 行为：
  - 设置页列全量事件，默认精选 6 个 on；可逐事件开关 + 选类型 + 填文案。
  - 启用事件 → settings.{group}.json 注入 `hooks.{Event}` 指向 aidog-notify.py；禁用 → 移除该事件 aidog 项（保用户项）。
  - 触发某事件 → /api/notify 收到 {event,vars} → 按 per_event 解析 type+模板 → 通知非空（default 兜底）。
  - Codex notify=[complete] 不受影响（回归验证）。
  - per-event 文案留空 → 回退类型模板/default。
- 向后兼容：旧无 per_event 配置不报错（空 map + 默认展示）。

## 失败处理
- 各事件 stdin 字段名与 docs 不符 → 脚本只塞「存在的已知字段」，缺失跳过（不硬依赖），标注实际采用字段。
- 注入遍历移除残留（改配置后旧事件 hook 未清）→ remove 遍历全量事件目录 + 单脚本名识别确保清净；加测试。
- ScriptPaths 改动误伤 Codex → cargo test 加 Codex 注入回归；保 complete 脚本。
- 前端与已提交 combobox 冲突 → 已拆 NotificationEventList.tsx，NotificationSettings 仅挂载，最小交叉。
- 任意门禁红 → 修到绿；范围外/卡住标 `需要:`。
