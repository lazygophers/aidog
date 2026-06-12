# PRD: Proxy 错误消息多语言适配 + 托盘实时更新

## 背景

proxy 层返回给客户端的 HTTP 错误响应中的 message 全部是硬编码英文字符串（如 "no matching group"、"read body: ..."、"Manual budget exhausted..."），没有根据 app 当前语言返回对应语言的错误信息。

另外用户反馈系统托盘的信息没有实时更新。

## 需求

### P0: Proxy 错误消息 i18n

**目标**: proxy 返回的错误 message 与 app 当前 UI 语言一致。

**方案**: 
- 在 Tauri command 层读取当前 app 语言设置（已存在 `app_settings.language`）
- 将语言标识存入 AppState 或通过 request header 传递给 proxy
- proxy 层根据语言标识选择对应的消息模板

**涉及文件**:
- `src-tauri/src/gateway/proxy.rs` — 12+ 处硬编码错误消息
- 可能需要新增 `src-tauri/src/gateway/i18n.rs` — 错误消息翻译表

**硬编码消息列表**:
1. `401` — 无消息体（仅状态码）
2. `400` — `"read body: {e}"`
3. `404` — `"no matching group"`
4. `400` — `"parse json: {e}"`
5. `400` — `"failed to parse request"`
6. `400` — `"route: {e}"`
7. `402` — `"Manual budget exhausted (kind=..., unit=..., amount=...). {recover_hint}"`
8. `502` — `"upstream: {e}"`
9. Mock errors — `"mock http_error"`, `"mock rate limit"`, `"mock timeout"`

**关键约束**:
- 错误消息中的结构化字段（`error.type`, `budget_kind` 等）保持英文不变（这些是机器可读的标识符）
- 仅 `message` 字段需要翻译
- 支持 7 种语言: zh-CN, en-US, ar-SA, fr-FR, de-DE, ru-RU, ja-JP

### P1: 托盘信息实时更新

**目标**: 系统托盘状态栏信息随请求实时更新。

**现状**: 前端 TrayConfigTab.tsx 中 todayStats 每 30s 轮询刷新，但后端 tray_segments 可能不随新请求更新。

**需要排查**: Rust 端 tray 更新机制是否在每次请求完成后刷新 tray。

## 验收标准

- [ ] 所有 proxy 返回的错误 message 与 app UI 语言一致
- [ ] 切换语言后，后续请求的错误消息立即使用新语言
- [ ] 结构化错误字段（type/kind 等）保持英文
- [ ] 托盘信息在每次请求后实时刷新

## 不做

- 不改上游平台返回的错误消息（直接透传）
- 不改 Mock 平台的错误消息（Mock 本身是调试用）
