# 通知中心展示项目名

## Goal

通知中心（inbox 页 + 系统弹窗）当前显示「任务完成 / Task Complete」等类型标题，**不含项目名**，用户在多项目并行时无法区分通知来源。修复让通知行展示「`<项目名> · <类型>`」，项目名 = hook 执行时的 cwd basename。

## 根因（诊断已完成）

数据链路：
1. `hooks.rs` Python 脚本：`project = os.path.basename(os.getcwd())` → POST `/api/notify` body `{"vars": {"project": project}}` ✅ 已传
2. `proxy.rs::handle_notify_inner` L409-413：解析 body，注入 `{group}`/`{time}` 内置变量，调 `dispatch(..., vars)` ✅
3. `notification.rs::dispatch` → `render(type, template, content, vars)`：
   - **L129**：`let title = default_title(notif_type).to_string()` —— title 写死 `default_title()` 静态英文常量（"Task Complete" 等），**不读取 vars["project"]** ← **根因**
   - L131-141：body 走 template + `substitute_vars(vars)`，但 task_complete/waiting_input 之外默认模板为空
4. `db.rs::insert_notification` L1838-1857：表 `notification(id, notif_type, title, body, created_at)`，**无 project 列**
5. 前端 `Notifications.tsx` L83：`{item.title || notifTypeLabel(type)}` 主文本 + L93 type chip — 重复显示类型，不带项目

## Decision (ADR-lite)

**Context**：用户选 B 变种 — title 前缀含项目名。形如 `aidog · 任务完成 [task_complete chip]`。

**Decision**：
- **后端**：`notification.rs::render()` 让 title 字段语义变为「项目名」（仅 vars["project"]），无项目时为空字符串。body 保持现状（template + vars）。popup/TTS 也用新 title（项目名作弹窗标题清晰可辨）。
- **前端**：`Notifications.tsx` 主文本拼 `${title} · ${notifTypeLabel}`（无 title 时仅 typeLabel）；保留 type chip。

**Consequences**：
- DB 表结构不动（title 列复用，含义变化），历史旧数据（title="Task Complete"）仍能显示 — 前端 fallback 自然处理。
- 弹窗 title 从「Task Complete」改为「aidog」— 项目名更直观，但失去类型信息；body 仍含类型语义。
- 不涉及 i18n 后端化（type 标签前端 `t()` 处理已有）。

## Requirements

- 后端 `render()` 改：title = `vars.get("project").cloned().unwrap_or_default()`（非空 → 替换 vars 字面占位）。
- 前端 `Notifications.tsx` 渲染拼接 `${title ? `${title} · ` : ""}${typeLabel}`。
- 单测：`render()` 现有测试调整 + 加 project 注入/不注入两路径；前端无 test 框架，靠 UI 检视。
- `cargo clippy` / `cargo test`（notification + db 模块）/ `tsc` / `check-i18n` 全绿。

## Acceptance Criteria

- [ ] Claude Code Stop hook 触发后，aidog 通知中心新行显示「`<cwd basename> · 任务完成`」
- [ ] vars 无 project 时（如手动 curl /api/notify 不传），inbox 行显示 `任务完成`（无前缀），不崩
- [ ] 历史数据（title="Task Complete"）仍能显示为「`Task Complete · 任务完成`」（旧 title 当 project 拼接） — 兼容退化，可接受
- [ ] 系统弹窗标题 = project 名（无 project → 类型默认名兜底）
- [ ] cargo test / clippy / yarn build / check-i18n 全绿

## Definition of Done

- 代码 + 测试更新
- 后端 `cargo clippy -- -D warnings` 0 warning
- 前端 `yarn build` + `yarn check:i18n` 0 error
- 提交 conventional commit
- 自检通过 + 落 cortex

## Out of Scope

- db 表加 project 列（A 方案）
- 通知中心按项目筛选 / 分组
- 默认模板含 project（C 方案）
- popup 标题保留类型（弹窗 title 仅 project）

## Technical Notes

### 后端改动

`src-tauri/src/gateway/notification.rs::render()` (L123-142)：

```rust
fn render(
    notif_type: NotifType,
    template: &str,
    content: Option<&str>,
    vars: &HashMap<String, String>,
) -> (String, String) {
    // title 字段语义改为「项目名」：vars["project"] 非空 → 项目名，否则空字符串
    // 兜底：vars 无 project 时 popup 标题用 default_title（前端 inbox 用 typeLabel）
    let title = vars
        .get("project")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let raw_body = if !template.is_empty() { template } else { content.unwrap_or_default() };
    let body = if raw_body.is_empty() {
        // body 仍含项目名 + 类型名（substitute 在 vars 含 project 时替换）
        substitute_vars(default_title(notif_type), vars)
    } else {
        substitute_vars(raw_body, vars)
    };
    (substitute_vars(&title, vars), body)
}
```

`dispatch()` L201-205 popup：若 title 空 → 用 default_title 作弹窗标题（避免空弹窗）。
```rust
let popup_title = if title.is_empty() { default_title(notif_type) } else { title.as_str() };
show_popup(app, popup_title, &body);
```

### 前端改动

`src/pages/Notifications.tsx` L82-94：
```tsx
<span style={{ fontSize: 13, fontWeight: 600 }}>
  {item.title ? `${item.title} · ${notifTypeLabel(item.notif_type, t)}` : notifTypeLabel(item.notif_type, t)}
</span>
{/* type chip 保留 — 显示类型标签 */}
```

### 测试

`notification.rs::tests` 已有 `substitute_known_and_unknown` / `substitute_all_vars`。新增 / 改造：
- `render_title_uses_project_var` — vars["project"]="aidog" → title="aidog"
- `render_title_empty_when_no_project` — vars 无 project → title=""
- `render_body_substitutes_vars` — body 模板 "{project} 完成" + project="x" → "x 完成"

`dispatch_inbox_persistence` 测试已存（db.rs:4236），insert 后 title 字段需是新语义（项目名）。

## Files

- `src-tauri/src/gateway/notification.rs` — render + dispatch popup fallback + tests
- `src/pages/Notifications.tsx` — 拼接渲染
- 不改 db.rs / models.rs / proxy.rs（vars 注入已正确）

## Research References

无外部研究 — 项目内 bug，全部信息来自源码直接读取（无网络 deps）。
