# Pending Backlog（排队任务，等 group-env-vars finish 后启动）

> 不提前 create（task.py create 偷 current pointer，见 [[trellis-create-steals-current-task]]）。
> group-env-vars archive 后，按顺序启动。每 task 独立走 /trellisx-flow 全流程。

---

## Task A（先到）：编程工具 tab 重构
详见 `pending-next-task.md`。改名「CLI 集成」+ 加语言设置 + 移 Claude Code 语言配置。

---

## Task B（本批）：平台「最近错误」展示优化（bug fix）

### 现象
平台卡片「最近错误」展示完整 JSON：
`HTTP 429: { "error": { "code": "429", "message": "quota exhausted", "type": "limitation" } }`
期望：有 error.message 时只展示 message 内容 → `HTTP 429: quota exhausted`

### 现状（已 grep）
- 提取逻辑已存在：`src-tauri/src/gateway/proxy/retry.rs:71 extract_error_message`（JSON → error.message → 顶层 message → trim → None 若空）
- 调用点：`src-tauri/src/gateway/proxy/non_success.rs:42-49`，存 `HTTP {code}: {extracted_msg or attempt_err}`
- 提取失败时 fallback `truncate_attempt_error(body)`（完整 body 截断 500 字符）= 用户看到的完整 JSON
- 记忆 [[platform-last-error-persisted]]：last_error DB 持久化是 afcd6fb 刚加的功能

### 待诊断根因（brainstorm 首问 / subagent 诊断）
1. **DB 残留旧值**：afcd6fb 提取逻辑后加，旧错误存完整 body 未重写 → 验：查 DB 该平台 last_error 实际值 + 时间戳 vs afcd6fb commit 时间。若是，需迁移清空或等新错误覆盖
2. **body 非 JSON**：上游返 HTML 错误页 / 解压乱码 / chunked 拼接坏 → 验：查 proxy_log.response_body
3. **body 结构异常**：数组包裹 `[{...}]` / message 字段值非 string（对象）/ 前后非 JSON 字符 → extract_error_message 漏 case

### scope 预判
- 若根因 1（旧数据）→ 可能仅需迁移脚本/清空逻辑，或文档说明「下次错误自动修正」
- 若根因 2/3 → extract_error_message 加 case（单文件 retry.rs + 测试 test_retry.rs），可能 ≤20 行
- bug fix，优先级可能高于 Task A（用户实际看到错误）

### 启动时
- create `platform-last-error-message-extract` 
- brainstorm 先诊断根因（grep DB / proxy_log 看实际 body），再定 scope
- 关联 spec：backend/platform-error-handling.md（记忆 [[auto-disable-only-401-403]] 提到错误处理契约已沉淀）

---

## Task C（本批）：导出 UX — 自动展开 + 条目级展示

详见 `pending-export-ux.md`。两交付项：① 去「预览导出项」按钮改 debounce 自动拉 + 条目级展示（每 mcp/skill 单独行 label 可见可控）；② setting 条目 label 本地化（`app:theme`→「主题」，根因 `apply/mod.rs:150` build_items setting label=裸 key，前端映射层修复）。纯前端（ImportExport.tsx），后端无需改。

**排队**：等 group-env-vars finish（共享 api.ts + locales×7）。与 Task A 也共享 locales×7 → 须串行，顺序 group-env-vars finish 后裁定。
