# Research: 当前 hook 注入机制全链路

- **Query**: hooks.rs build_hook_script / inject / 注入链路 / 泛化难点
- **Scope**: internal
- **Date**: 2026-06-15

## 核心文件

| File | 角色 |
|---|---|
| `src-tauri/src/gateway/hooks.rs` | 纯逻辑：脚本内容生成 + CC settings/Codex toml 改写（全文 487 行，已读） |
| `src-tauri/src/lib.rs` | command 层：脚本落盘 + DB 读写 + sync 物化（关键段 1126-1265 / 2280-2497） |
| `src-tauri/src/gateway/scripts.rs` | `ScriptInvoker`（uv / python3）`command_for()` |

## 脚本生成 `build_hook_script(notif_type)` — hooks.rs:64-123

- 生成一段 **Python（PEP723 头 + stdlib only urllib）** 脚本字符串。`notif_type` 直接内插到脚本体 `{{"type": "{notif_type}", "vars": {{"project": project}}}}`（hooks.rs:96-98）。
- POST payload 固定为 `{type, vars:{project}}`，**不带 content，不解析 stdin/$1**（hooks.rs:73-74 注释明确「stdin 为事件 JSON，忽略」）。
- endpoint：从 `ANTHROPIC_BASE_URL` 推导，依次剥离 `/proxy` `/v1` `/api/paas/v4` 后缀再拼 `/api/notify`（hooks.rs:89-93）。
- 鉴权：`ANTHROPIC_AUTH_TOKEN`（= group_name）作 Bearer（hooks.rs:105）。
- project：`os.path.basename(os.getcwd())`（hooks.rs:95）—— **CC hook 调用时 cwd = 项目目录**，故 project = 项目名。
- 异常静默吞（通知非关键路径）。

**关键限制**：当前每个 notif_type 各生成**一个独立脚本文件**，type 是**编译期内插**到脚本字节里，不是运行时参数。

## 脚本落盘 `generate_hook_scripts(invoker)` — lib.rs:2285-2319

- 落 2 个文件到 `~/.aidog/scripts/`：`SCRIPT_COMPLETE="aidog-notify-complete.py"`(type=task_complete) + `SCRIPT_WAITING="aidog-notify-waiting.py"`(type=waiting_input)（hooks.rs:40-42, lib.rs:2308-2317）。
- chmod 755；清理旧版 `.sh`（迁移）。
- 返回 `ScriptPaths { complete, waiting }`，每个值是 **command 串**（`uv run --script <path>` 或 `python3 <path>`，由 `invoker.command_for()` 决定，scripts.rs:48-51）。

`ScriptPaths` 结构 — hooks.rs:126-129：仅 `complete` / `waiting` 两字段。

## CC settings 注入 `inject_claude_code_hooks(config, scripts)` — hooks.rs:150-167

- 打 marker `_aidog_hooks={"enabled":true}`（MARKER_HOOKS=hooks.rs:50）。
- **硬编码两事件**：`set_event_hook(hooks_obj, "Stop", &scripts.complete)` + `set_event_hook(hooks_obj, "Notification", &scripts.waiting)`（hooks.rs:165-166）。

`set_event_hook(hooks, event, script_path)` — hooks.rs:184-200：
- CC hooks schema 结构：`{ "<Event>": [ { "hooks": [ { "type":"command", "command":"<path>" } ] } ] }`（hooks.rs:188-190）。**无 matcher 字段**（匹配全部）。
- 先 `remove_event_hook` 去重再追加（幂等，保留用户项）。

`remove_event_hook` — hooks.rs:204-222：按 command 串含 aidog 脚本文件名识别 aidog 项（`references_aidog_script` hooks.rs:226-231 含 `.py` 当前名 + `.sh` 旧名），混合组只剔 aidog 项保用户项，纯 aidog 组整删，空 Event 删 key。

`remove_claude_code_hooks` — hooks.rs:171-181：固定遍历 `["Stop","Notification"]` 移除。

## 谁调用注入

1. **总开关物化（主链路）** `do_sync_group_settings` — lib.rs:1126-1265：
   - 读 marker `hooks_marker_enabled(&base_config)`（lib.rs:1154），开则 `generate_hook_scripts` 一次（循环外，lib.rs:1155-1166）。
   - 每分组 config 注入 CC hooks（lib.rs:1193-1195），strip marker 之前（lib.rs:1201）。
   - Codex 全局 config.toml 一次性 inject/remove notify（lib.rs:1228-1243）。
2. **单 group 一键** `inject_hooks` / `remove_hooks` — lib.rs:2350-2427（API 仍在，UI 按钮已删，见 04）。
3. **总开关写入** `set_default_hooks_enabled` — lib.rs:2447-2478：写 marker + re-sync。
4. **前端片段** `build_notify_hooks_fragment` — lib.rs:2485-2497：空对象 inject 取 `hooks` 子对象返回（只读，不写 DB）。

注：`set_default_hooks_enabled`/`inject_hooks` 会调 `seed_default_templates`（lib.rs:2321-2343）物化 task_complete/waiting_input 默认模板。

## Codex 注入 — hooks.rs:241-268

- `inject_codex_notify(config, complete_script)`：写顶层 `notify=[complete_script]`（hooks.rs:246-249）。只挂 complete（Codex 仅 turn-complete 事件）。
- `remove_codex_notify`：仅当 notify 指向 aidog 脚本时移除，保用户项。
- **本任务限 CC，Codex 不动**（见 05）。

## 泛化难点（2 固定事件 2 固定脚本 → N 事件 × 每事件可选类型+文案）

1. **脚本通用化可行**：当前脚本仅用 cwd basename，不解析 stdin。要支持「每事件不同 type + 文案」有两条路：
   - **方案 A（推荐）单通用脚本 + 运行时参数**：一个脚本，事件名/type 作命令行参数传入（CC hook command 可写 `python3 x.py --event Stop --type task_complete`）。脚本读 stdin JSON 取事件特有字段塞进 vars。优点：N 事件共 1 文件，文案在后端模板渲染（见 02/03）。
   - **方案 B 每事件一脚本**：沿用现有「type 内插」，扩成「event+type 内插」，N 事件落 N 文件。文件数膨胀，但改动最小。
2. **stdin 解析新增**：要把事件特有字段（SubagentStop.agent_type、Notification.message 等）带入 vars，脚本必须解析 stdin JSON（当前显式忽略）。这是相对现状的**新增能力**（见 03）。
3. **注入泛化**：`inject_claude_code_hooks` 的硬编码 `Stop`/`Notification` 两行要改成「遍历启用事件配置 → 每事件 set_event_hook(event, 对应脚本/命令)」。`remove_claude_code_hooks` 的固定 `["Stop","Notification"]` 要改成「遍历全量事件名移除」或按 marker 记录的事件集移除。
4. **identifier 兜底**：`references_aidog_script` 靠脚本文件名识别 aidog 项 —— 方案 A 单脚本天然好识别；方案 B 多脚本需保证命名规律（如 `aidog-notify-<event>.py`）以便 remove 全量匹配。

## Caveats

- 脚本 endpoint 推导链已硬编码 3 个后缀，新平台前缀需同步（与本任务无关，记录）。
- CC hook command 串支持任意 shell，方案 A 传参完全可行；但需验证 CC 是否对 command 串做 shell 解析（推测: 是，CC hooks command 走 shell —— 未在 docs 核实，实现前需确认）。
