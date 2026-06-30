# PRD: 平台最近错误展示提取 error.message

## 现象
平台卡片「最近错误」展示完整 JSON：
`HTTP 429: { "error": { "code": "429", "message": "quota exhausted", "type": "limitation" } }`
期望：有 `error.message` 时只展示 message → `HTTP 429: quota exhausted`

## 现状（已 grep）
- 提取逻辑已存在：`src-tauri/src/gateway/proxy/retry.rs:71 extract_error_message`
  - JSON → `error.message`（嵌套）→ 顶层 `message` → trim → None 若空
- 调用点：`src-tauri/src/gateway/proxy/non_success.rs:42-49`
  - `extracted_msg.unwrap_or_else(|| attempt_err)` → 存 `HTTP {code}: {detail}`
- 提取失败 fallback = `truncate_attempt_error(body)`（完整 body 截断 500 字符）= 用户看到的完整 JSON
- 记忆 [[platform-last-error-persisted]]：last_error DB 持久化是 afcd6fb 刚加

## 根因假设（subagent 须先诊断，再修）
1. **DB 残留旧值**（最可能）：afcd6fb 提取逻辑后加，旧错误存完整 body 未重写。验：查 DB platform 表 last_error 实际值 + last_error_at 时间戳 vs afcd6fb commit 时间
2. **body 非 JSON**：上游返 HTML 错误页 / 解压乱码 / chunked 拼接坏。验：查 proxy_log.response_body 该失败请求实际内容
3. **extract_error_message 漏 case**：body 数组包裹 `[{...}]` / message 字段值非 string（对象）/ 前后非 JSON 字符。验：构造对应 body 跑 test_retry.rs

## 修复方向（据根因定）
- 根因 1（旧数据）：选项 A 写一次性迁移清空不符格式 last_error；选项 B 文档说明「下次错误自动修正」不迁。推荐 A（用户体验即时修复）
- 根因 2（body 非 JSON）：extract_error_message 加 HTML/纯文本 fallback 提取（如 `<title>...</title>` / 首行）；或维持 None 但改 fallback 不展示完整 body（截断更短）
- 根因 3（漏 case）：extract_error_message 补 case（数组首元素 / message 是对象时取 .text 或 to_string）

## 验收
1. 根因诊断有据（DB 查询结果 / proxy_log body / 测试输出，附引用）
2. 修复后：标准 OpenAI body `{error:{message:"..."}}` → 只展示 message
3. cargo test（test_retry.rs extract_error_message 相关）全绿；新增 case 覆盖诊断到的漏点
4. cargo clippy 零 warning
5. 若改 extract_error_message，关联 spec backend/platform-error-handling.md 同步

## 非目标
- 不改前端展示组件（后端存对即可，前端只读 last_error 字符串）
- 不改 429 分类逻辑（classify_429 不动）

## 风险
- 根因 1 若是旧数据，迁移清空可能误删用户想看的真实错误 → 迁移只清"完整 JSON 含 error.message"的（可二次提取的），保留其它
