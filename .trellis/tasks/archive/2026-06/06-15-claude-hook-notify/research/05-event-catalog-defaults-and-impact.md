# Research: Codex 牵连 + 事件全集/默认映射 + 实现影响清单

- **Query**: codex 不破坏、精选集可行性、事件→默认类型映射、实现影响汇总
- **Scope**: mixed
- **Date**: 2026-06-15

## Codex 牵连（确认本任务限 CC 不破坏 Codex）

hooks.rs codex refs：grep 见 `inject_codex_notify`/`remove_codex_notify`（hooks.rs:246/252）+ HookClient::Codex（:26/:33）+ 脚本注释（:13/:59/:74）。

lib.rs：`do_sync_group_settings` 的 Codex 块在 **CC per-group 循环之外**，独立按 marker 一次性 inject/remove 顶层 `notify=[complete脚本]`（lib.rs:1226-1243）。`inject_hooks`/`remove_hooks` 的 Codex 分支也独立（lib.rs:2382-2385/2420-2423）。

**结论**：Codex 路径与 CC 事件配置**完全解耦**。CC 侧泛化（per_event + 注入遍历）只要不动 `inject_codex_notify`/`remove_codex_notify` 调用与 Codex 分支即可不破坏 Codex。Codex 仅 `agent-turn-complete` 单事件，无逐事件需求，保持现状。

**注意点**：若改 `generate_hook_scripts` 返回结构（如方案 A 改成单通用脚本/command），Codex 注入依赖 `scripts.complete`（lib.rs:1232/2384）。泛化时须保证仍能给 Codex 提供一个「complete 脚本 command」（task_complete 语义），或单独为 Codex 生成 complete 脚本。**这是跨 CC/Codex 的耦合点，泛化 ScriptPaths 时勿丢 complete。**

## 官方 hook 事件全集（来源：任务 brief 引 code.claude.com/docs/zh-CN/hooks，约 30 个）

> ⚠️ 各事件 stdin 特有字段名**未在本次逐一核 docs**，实现脚本 stdin 解析前必须查官方 docs 确认字段名。下列默认映射为建议，待实现核可。

通用 stdin 字段：`session_id` / `cwd` / `hook_event_name` + 事件特有字段。

### 建议精选默认 ON 集 + 默认 notif_type 映射

| Event | 默认 notif_type | 理由 / 可用 body 字段 |
|---|---|---|
| Stop | task_complete | 主回合结束 = 任务完成（现有硬编码事件） |
| SubagentStop | task_complete | 子代理结束；stdin 推测有 agent 信息 → vars 可填 |
| Notification | waiting_input | CC 主动通知（含 message）= 等待输入（现有硬编码事件） |
| PermissionRequest | waiting_input | 等待用户授权决策 |
| SessionStart | task_complete（或新「info」语义，暂复用） | 会话开始；低价值，可考虑默认 off |
| SessionEnd | task_complete | 会话结束 |
| PreCompact | task_complete | 压缩前提示；低频 |

> brief 建议精选集 = Stop/SubagentStop/Notification/SessionStart/SessionEnd/PreCompact/PermissionRequest。SessionStart 噪音偏高，建议实现时评估是否降级默认 off。

### 建议默认 OFF（高频/无意义/无有效 body）

- **高频噪音**：PreToolUse / PostToolUse / PostToolUseFailure / PostToolBatch / PreToolBatch / MessageDisplay / UserPromptSubmit / UserPromptExpansion / FileChanged / CwdChanged —— 每次工具调用/每条消息触发，开了会通知轰炸。
- **失败类 → error 类型（默认 off，用户可开）**：PostToolUseFailure / StopFailure → notif_type=error。
- **低价值/内部**：Setup / InstructionsLoaded / ConfigChange / TaskCreated / TaskCompleted / TeammateIdle / WorktreeCreate / WorktreeRemove / PostCompact / Elicitation / ElicitationResult / SubagentStart / PermissionDenied。其中 TaskCompleted→task_complete、StopFailure→error 作为「用户可开时的默认 type」。

### 默认 type 推断规则（实现可固化为常量表）
- 名字含 `Stop`/`Complete`/`End` → task_complete
- 名字含 `Failure`/`Denied`/`Error` → error
- 名字含 `Notification`/`Permission`/`Elicitation`(请求) → waiting_input
- 其余 → task_complete（兜底，from_str_or_default 也兜底）

## 精选集可行性结论
- 7 个精选事件均有意义可做通知；SessionStart 噪音偏高建议二次评估。
- 全量 30 事件「可开」可行：UI 列全量，默认仅精选 on，其余 off。每事件默认 notif_type 用上表/规则。
- 事件特有字段进 body 依赖脚本 stdin 解析（新增）+ 模板写 `{字段}`（substitute_vars 已支持，见 03）。

---

# 实现影响清单（汇总）

## 要改的文件

| 文件 | 改动 |
|---|---|
| `src-tauri/src/gateway/hooks.rs` | `build_hook_script` 泛化（单通用脚本 + 事件/type 参数 + stdin 解析）；`inject_claude_code_hooks` 改为遍历启用事件 set_event_hook；`remove_claude_code_hooks` 改为全量事件名遍历移除；`ScriptPaths` 结构调整（保留 complete 供 Codex） |
| `src-tauri/src/gateway/models.rs` | `NotificationSettings` 加 `per_event: HashMap<String,EventSetting>`；新增 `EventSetting{enabled,notif_type,template}`（全 serde default 兼容） |
| `src-tauri/src/lib.rs` | `generate_hook_scripts` 适配新脚本生成；`do_sync_group_settings` 注入按 per_event 遍历；`inject_hooks`/`set_default_hooks_enabled`/`build_notify_hooks_fragment` 适配；Codex 分支保 complete 不动 |
| `src-tauri/src/gateway/notification.rs` | **基本不改**（vars 透传已支持事件特有字段）；如需事件→type 解析可加 helper |
| `src-tauri/src/gateway/proxy.rs` | **不改**（NotifyReq 已含 type/content/vars，事件字段走 vars） |
| `src-tauri/src/gateway/db.rs` | **不改**（整 blob 存，零 migration） |
| `src/services/api.ts` | 加 `EventSetting` 类型 + `NotificationSettings.per_event` 字段 |
| `src/components/settings/NotificationSettings.tsx` | 挂载逐事件区块（**建议拆新文件 NotificationEventList.tsx 避开并行任务的 template combobox 改动**） |
| i18n 7 语言 | 新区块文案 key（check-i18n.mjs 防线） |
| docs/（Rspress 7 语言） | 视情补新功能文档（memory docs-site-i18n-coverage 规约） |

## 泛化方案建议：脚本通用化 vs 多脚本

**推荐方案 A：单通用脚本 + 运行时参数**
- 一个 `aidog-notify.py`，CC hook command 写 `<invoker> aidog-notify.py --event <Event> --type <notif_type>`。
- 脚本读 stdin JSON 解析事件特有字段塞 vars，POST `{type, vars}`。
- 优点：N 事件共 1 文件；type/event 运行时参数化；`references_aidog_script` 单文件名好识别 remove。
- 风险：CC hook command 是否走 shell 解析参数需核 docs（推测: 是，未核实）。

**方案 B：每事件一脚本**（沿用现有 type 内插模式扩成 event+type 内插）
- 改动最小，但 N 文件膨胀，命名须规律以便 remove 全匹配。

Codex 兼容：无论 A/B，须保留「task_complete 语义的 complete 脚本/command」供 `inject_codex_notify`（lib.rs:1232）。

## 风险/兼容点
1. **并行任务 merge 冲突**（NotificationSettings.tsx template combobox 在改）—— 拆新文件最小交叉（memory 多次记录并行改同文件灾难）。
2. **CC hook command shell 解析**未核实 —— 实现前确认能否传参（决定方案 A 可行性）。
3. **各事件 stdin 字段名**未核 docs —— 脚本解析前必查官方 docs。
4. **向后兼容**：旧 NotificationSettings 无 per_event → 空 map → 需「精选默认集」展示层兜底（不写死进 DB，见 02）。
5. **Codex complete 脚本依赖** —— 泛化 ScriptPaths 勿丢 complete。
6. **remove 全量事件遍历** —— marker 应记录已注入事件集，或固定遍历全量事件名，避免改配置后残留旧事件 hook。
7. **i18n / docs** 全覆盖硬规。

## 建议拆 subtask（资源互斥串行化）
1. **S1 后端模型+持久化**：models.rs per_event/EventSetting（无依赖，先行）。
2. **S2 后端脚本+注入泛化**：hooks.rs + lib.rs（依赖 S1 的事件→type 语义；含 Codex 不破坏验证 cargo test）。
3. **S3 前端类型+UI**：api.ts + 新 NotificationEventList.tsx（依赖 S1 字段；**与并行 template 任务资源互斥，串行或拆新文件**）。
4. **S4 i18n + docs**：7 语言 key + Rspress（依赖 S3 UI 文案定稿）。

S1→S2、S1→S3 可并行（S2/S3 不共文件）；S4 收尾。
