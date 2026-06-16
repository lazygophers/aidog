# IMPLEMENT_REPORT — 06-16-proxy-ua-passthrough

按 UA 选择透传协议 + 多级回退（PRD §0 D1–D4，单交付）。

## 改动文件

- `src-tauri/src/gateway/proxy.rs`（唯一改动文件，converter.rs 无需改）

## 新增函数

```rust
fn infer_passthrough_protocol_from_ua(ua: &str) -> Option<&'static str>
```

- 位置：`detect_source_protocol` 之后（原 ~2818 行尾）。
- 规则（大小写不敏感）：含 `claude-cli`→`Some("anthropic")`；含 `codex`→`Some("openai_responses")`；其它→`None`。
- 返回字面量与 `detect_source_protocol` / `ep_proto` 产出协议名一致，便于直接比对 endpoint。

## 插入点（重试循环内，每候选独立判定）

1. `matched_ep` 解析后（原 `proxy.rs:1009` 末），新增 UA 透传分支重绑定 `matched_ep` + 产出 `passthrough_proto`：
   - 仅 `matched_ep.is_none()` 时尝试：读 `orig_headers.get("user-agent")` → `infer_passthrough_protocol_from_ua`。
   - `Some(p)` 且平台 endpoints 含 `ep_proto == p` → `matched_ep` 改指向该 UA-endpoint，`passthrough_proto = Some(p)`，打 `tracing::info!("ua-passthrough: ...")`（含 platform/source_protocol/ua_protocol，复用现有日志，不改 proxy_log 表）。
   - UA 命中但平台无该 endpoint（级别 2）/ UA 不识别（级别 3）→ `matched_ep` 保持 `None`，`passthrough_proto = None`，回退现有兜底。
   - `matched_ep` 已命中（path 被支持，级别 0）→ 不介入，`passthrough_proto = None`。
2. `(target_protocol_enum, target_base_url, client_type, coding_plan)` 派生量原样从（已可能重绑定的）`matched_ep` 取，UA 命中后自动取 UA-endpoint 的值（满足 PRD §8 "target/client_type 在 UA 命中后重算"）。
3. `same_protocol_passthrough` 判定改为按 `passthrough_proto` 分流：
   - `Some(p)`（UA 透传）→ `ep_proto(matched_ep) == p` → true。
   - `None`（现状）→ `ep_proto(matched_ep) == source_protocol`（级别 0 行为零变更）。

下游完全复用现有 same-protocol passthrough 路径：`req_value` 原始 body 仅 patch model（原 1135-1142）+ `passthrough_api_path(target_protocol_enum, ...)`（converter.rs:54）+ 流式 `passthrough_response` 原样 relay（原 1414-1512）。无新出站/响应代码。

## 新增单测（proxy.rs #[cfg(test)]）

- `infer_passthrough_protocol_from_ua_mapping`：claude-cli/各 codex 子串命中 + Cursor/Windsurf/gemini-cli/curl/空→None。
- `ua_passthrough_three_level_fallback`：级别 0（path 支持不介入）/级别 1（UA 命中+平台有 endpoint 透传）/级别 2（UA 命中+平台无 endpoint 回退）/级别 3（UA 不识别回退）。

## 门禁实际输出（worktree/src-tauri）

```
cargo build:  0 errors, 1 warnings (382 crates)   # 唯一 warning = 第三方 block v0.1.6 future-incompat（已知接受，非本改动）
cargo clippy: 0 errors, 1 warnings                # 同上，grep 过滤后无本仓 warning
cargo test:   315 passed (3 suites)               # 含新增 2 测试，现有 313 不回归
```

## 验收对齐（PRD §10）

1. claude-cli UA + 平台无 path endpoint 但有 anthropic endpoint → 透传（tracing `ua-passthrough`，body 仅 patch model）✅ 逻辑覆盖。
2. codex UA → openai_responses 透传同理 ✅。
3. UA 命中但平台无推断协议 endpoint（级别 2）→ 回退现有 convert_request ✅ matched_ep 保持 None。
4. UA 不识别（级别 3）→ 回退 ✅。
5. 级别 0（精确同协议 + openai_responses→openai）零变更 ✅ passthrough_proto=None 走原分支。
6. 子端点分流（count_tokens/responses/models/健康端点）在 parse 前前置分流，本改动不触碰 ✅。
7. proxy_log 表结构未变更 ✅ 仅复用现有字段 + tracing。
8. cargo clippy 无本仓 warning + cargo test 全绿（含新单测）✅。

## 遗留项 / 已知限制

- UA 透传分支复用同协议透传出站路径，继承其已知限制（platform 层中间件 mask/inject 改写 chat_req 对透传分支不生效，max_tokens 出站裁剪不作用于透传 body — 与现有 same-protocol passthrough 一致，非本改动引入）。
- 无新「需要:」开放项；PRD §0 D1–D4 全部可直接实现。
