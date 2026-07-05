# MITM h2 stream CANCEL 真根因 (env proxy 已修仍 CANCEL)

## Goal

env proxy 递归根因已修（`http_client.rs` `build_http_client` `use_proxy=false` 分支显式 `.no_proxy()`，commit `e6fba678`），但用户报 h2 stream 仍 CANCEL。本任务定位**真根因**并最小修复。

## What I already know

- 前任务 `07-05-h2-passthrough-stream-cancel` 定位根因：reqwest 读 env proxy 指向 AirDog 自身 → CONNECT 隧道无限递归 → 资源耗尽 → h2 stream RST → 客户端 `HTTP/2 stream ... CANCEL (err 8)`
- 修复点：`src-tauri/src/gateway/http_client.rs` `build_http_client` 的 `use_proxy=false` 分支加 `.no_proxy()`
- spec 已沉淀：`.trellis/spec/backend/http-client-forward.md`
- 用户反馈：修复后仍 CANCEL（具体复现细节待补）

## Assumptions (temporary, 待用户复现细节验证)

- 修复确实落地（已 grep 确认 http_client.rs:67 `.no_proxy()` 在 `use_proxy=false` 分支）
- CANCEL 现象与新根因相关，非旧根因残留（如用户未重启 / 旧 binary / 缓存）
- 现象可能场景（待 user 确认）：
  - h2 直通转发（connect.rs auto Builder）流控 / 超时 / 连接管理问题
  - passthrough 上游响应流被中途切断（stream.rs）
  - forward 路径超时级联 / reqwest 默认 timeout
  - MITM TLS 协商 / ALPN 选 h2 后流处理

## Open Questions (Blocking)

- 复现路径：什么客户端（claude-code / codex / 其他）+ 什么上游 host + AI 协议（anthropic / openai）
- 错误现象：客户端报错原文 / proxy_log status / 上游响应是否部分返回
- 复现频率：必现 / 偶发 / 特定请求
- 是否已重启验证修复生效（确认非旧 binary）

## Requirements (evolving)

- [ ] 拿到 CANCEL 复现的完整链路证据（client error + proxy_log + 上游响应）
- [ ] 定位 env proxy 之外的真根因
- [ ] 最小修复 + 验证

## Acceptance Criteria (evolving)

- [ ] 修复后用户原复现路径不再 CANCEL
- [ ] 新增覆盖该根因的回归测试
- [ ] spec 沉淀新根因（更新 `http-client-forward.md` 或新建 spec）

## Definition of Done

- cargo test / clippy 绿
- 用户 dev-app 实测确认 CANCEL 消失
- spec 更新

## Out of Scope

- env proxy 旧根因（已修，不再覆盖）

## Technical Notes

- 修复点 file: `src-tauri/src/gateway/http_client.rs:67`
- 相关模块: `gateway/proxy/passthrough.rs` / `forward.rs` / `connect.rs` / `stream.rs`
- spec: `.trellis/spec/backend/http-client-forward.md`、`proxy-connect-relay.md`
- 前任务归档: `.trellis/tasks/archive/2026-07/07-05-h2-passthrough-stream-cancel/`
