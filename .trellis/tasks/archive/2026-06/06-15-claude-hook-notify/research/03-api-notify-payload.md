# Research: /api/notify 入站 + dispatch + vars

- **Query**: proxy /api/notify handler、payload 结构、事件特有字段进 vars
- **Scope**: internal
- **Date**: 2026-06-15

## 端点 `/api/notify` — proxy.rs:345-432

路由注册 proxy.rs:71 `.route("/api/notify", post(handle_notify))`（仅 POST，localhost-only 体系）。

### 请求体 `NotifyReq` — proxy.rs:347-356
```rust
struct NotifyReq {
    #[serde(rename="type")] notif_type: String,
    #[serde(default)] content: Option<String>,
    #[serde(default)] vars: HashMap<String,String>,
}
```
即 `{type, content?, vars?}`。**type 任意字符串**（dispatch 内 from_str_or_default 兜底 task_complete）。

### handler 流程 — proxy.rs:370-431
1. Bearer group_name 鉴权（proxy.rs:376-384），校验分组存在（proxy.rs:386-396），否则 401。
2. 解析 body（proxy.rs:398-404），失败 400。
3. **注入内置变量**（proxy.rs:406-411）：`vars["group"]` 默认填鉴权 group_name；`vars["time"]` 默认当前本地 `%H:%M:%S`。脚本可覆盖。
4. 调 `notification::dispatch(db, app, &req.notif_type, req.content.as_deref(), &vars)`（proxy.rs:413-420）。
5. 返回 `DispatchResult` JSON（proxy.rs:431）。

## dispatch + render — notification.rs

### dispatch — notification.rs:185-239+
- `get_notification_settings` → `from_str_or_default(type_str)` → `settings.type_setting(notif_type)`（notification.rs:192-194）。
- 总开关 OFF 旁路（notification.rs:197-209）。
- `render(notif_type, &setting.template, content, vars)` → (title, body)（notification.rs:211）。
- `channels_for_form(setting.form)` 决定 tts/popup/inbox/sound 通道（notification.rs:212, channels_for_form 在 notification.rs:94）。
- TTS：`ch.tts && setting.tts && settings.tts_enabled`（notification.rs:215）。popup：`ch.popup && setting.popup`（216）。inbox 落库（220-227）。

### render — notification.rs:132-170
- **title = `vars["project"]`**（notification.rs:138-141），无则空 → 前端 fallback 类型标签。
- **body 兜底链**：`setting.template`(非空) > `content` > `default_template`（template+content 都空时）> `default_title`（末位）（notification.rs:143-167）。
- 全程走 `substitute_vars`（notification.rs:24-58）：线性扫描 `{key}` 替换，**未知 key 保留原文**，支持任意 key。

### substitute_vars 支持的内置变量
模板可用：`{project} {status} {time} {session} {group}`（前端 TEMPLATE_VARS NotificationSettings.tsx:25 + 测试 notification.rs:421-429 验证）。

## 新事件脚本 POST payload 怎么填

### type 字段
- **方案 A 单通用脚本**：脚本按命令行参数（事件名/type）填 `type`。CC hook command 传 `--type <notif_type>`，脚本读取后填 payload。
- 或：脚本固定按事件传 type（每事件命令不同 type 参数）。

### content + vars 注入事件特有字段
- 脚本**读 stdin JSON**（当前脚本忽略 stdin，新增解析），取事件特有字段塞 vars：
  - SubagentStop → `vars["agent_type"]`（stdin 含 agent 信息）
  - Notification → `vars["message"]`（stdin 含 message）
  - 通用字段 `session_id`/`cwd`/`hook_event_name` → 可填 `{session}` 等
- **vars 透传无需改后端**：substitute_vars 已支持任意 key，模板写 `{agent_type}` 即可渲染（notification.rs:24-58 + 测试 408 证明未知 key 保留、已知 key 替换）。
- content 可由脚本填事件摘要文案，作为 body 兜底链一环。

### 模板能用事件特有字段的前提
1. 脚本解析 stdin 并塞进 vars（新增）。
2. 模板里写 `{agent_type}` 等（用户/默认模板）。
3. 后端 substitute_vars 自动渲染（已支持，零改动）。

## 实现影响（本主题）
- **后端 dispatch/render/substitute_vars 基本不用改** —— vars 透传机制天然支持事件特有字段。
- 主要新增在**脚本层**（解析 stdin + 按事件填 type/vars）和**注入层**（见 01）。
- 若想让事件特有字段有「内置变量提示」，前端 TEMPLATE_VARS 可按事件扩展（见 04）。

## Caveats
- CC 各事件 stdin 实际字段名需以官方 docs 为准（见 05），脚本解析前要核字段名（如 agent_type 还是 subagent_type —— **未在本次核实，实现前查 docs**）。
- vars 值类型 HashMap<String,String>，事件特有字段若是嵌套对象需脚本拍平成字符串。
