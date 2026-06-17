# CHECK_REPORT — 06-16-proxy-ua-passthrough

总判定: **PASS**

变更范围: 仅 `src-tauri/src/gateway/proxy.rs`（+130 -3）。无 db.rs / migrations / models.rs / 前端改动。

## 门禁复跑（worktree/src-tauri）

| 项 | 结果 | 证据 |
|---|---|---|
| cargo build | PASS | `0 errors, 1 warnings`（仅 block v0.1.6 future-incompat，第三方接受） |
| cargo clippy | PASS | `0 errors, 1 warnings`，过滤后确认仅 block future-incompat |
| cargo test | PASS | `315 passed`（3 suites）。新增 2 测试均 ok：`proxy::tests::ua_passthrough_three_level_fallback`、`proxy::tests::infer_passthrough_protocol_from_ua_mapping`（/tmp/ua_test_out.txt L20-21） |

## PRD §0 决策合规

- **D1（仅 matched_ep==None 介入）PASS**：proxy.rs:1018 `if matched_ep.is_none()` 守卫；path 已支持（matched_ep==Some）走 else 分支 `(matched_ep, None)`，§5 级别 0 行为零变更。
- **D2（UA 映射）PASS**：`infer_passthrough_protocol_from_ua`（proxy.rs:2855+）— `claude-cli`→`anthropic`、`codex`→`openai_responses`、其余→None；`to_lowercase()` 大小写不敏感。单测覆盖 claude-cli/全 codex 变体/Cursor/Windsurf/gemini-cli/curl/空。
- **级别 1/2/3 PASS**：UA 命中且平台有该协议 endpoint（proxy.rs:1024 find）→ matched_ep 重绑定 + passthrough_proto=Some(p)（L1031）；命中但平台无 endpoint→(matched_ep=None, None)（L1034，级别 2 回退）；UA 不识别→(matched_ep=None, None)（L1037，级别 3 回退）。
- **D4（proxy_log 表结构未变）PASS**：diff 内无 ALTER/CREATE TABLE/ADD COLUMN；无 db.rs/migrations 改动。UA 命中走 tracing::info!("ua-passthrough...")（L1026-1030）。

## CLAUDE.md 约束

- **URL 构造 PASS**：UA 命中后 target_base_url/target_protocol_enum 取自 UA-endpoint（L1043-1045），api_path 经 `passthrough_api_path`（L1179），URL=base_url.trim_end('/')+api_path（L1195），无额外拼接。
- **健康端点 PASS**：`GET /`、`GET /proxy`→handle_root（L75-76,151）未触碰。
- **same-protocol passthrough 未回归 PASS**：same_protocol_passthrough（L1060-1063）passthrough_proto==None 时退回原 `ep_proto==source_protocol` 判定，含 openai_responses→openai 跨协议回退（仍 false→convert_request）零变更。

## 跨层一致 PASS

UA 透传重绑定后 target_protocol_enum/target_base_url/client_type/coding_plan 统一从 matched_ep 派生（L1043），same_protocol_passthrough=true → 复用现有 passthrough 出站（L1170+）/响应（passthrough_response）路径，继承 5 项旁路改写。

## 新单测质量 PASS

- `infer_passthrough_protocol_from_ua_mapping`：直接调用生产 fn，全分支 + 大小写 + 空串，非空壳。
- `ua_passthrough_three_level_fallback`：逻辑镜像 try_passthrough（调用真实 infer fn），覆盖级别 0/1/2/3。镜像而非走全 HTTP 路径，属可接受弱形式（真实路径需完整 HTTP 桩）。

## 自修内容

无（未发现需修复问题）。
