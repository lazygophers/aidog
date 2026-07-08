# 修复 trace 81dc4466 火山引擎回退 400 parameter_error

## Goal

请求 `trace_id=81dc4466075846c0a504d1367db6affb`（`claude-opus-4-8`, group=白嫖）回退链终止于火山引擎 400 parameter_error，用户请求失败。需诊断根因并最小修复。

## 现象（trace 实证，源 ~/.aidog/logs/aidog.2026-07-08-09.log:477-491）

```
17:34:03.625  POST /proxy/v1/messages  group=白嫖  model=claude-opus-4-8
17:34:03.663  candidates: 11 候选, mode=HealthAware, first=MiniMax Coding Plan-EWOQ
17:34:03.666  → MiniMax Coding Plan-EWOQ (platform_id=266)  coding_plan=true  remap=false
17:34:03.995  ← 429 (369ms)
17:34:03.999  → MiniMax Coding Plan-d4bs (platform_id=265)  coding_plan=true  remap=false
17:34:04.467  ← 429 (841ms)
17:34:04.468  → 火山引擎 (platform_id=214)  actual_model=doubao-seed-2-0-code  coding_plan=false  remap=true  stream=true
17:34:04.484    upstream POST https://ark.cn-beijing.volces.com/api/coding/v1/messages
17:34:04.747  ← 400 (1121ms)
17:34:04.750  non_success: error_rule#8「内置·参数错误」category=parameter_error retryable=false
17:34:04.750  decision-A: hard request error (400/422), NOT retrying next platform
17:34:04.750  final status=400 tokens=0/0/0 cost=0.0
```

## 根因（main 实证，bug-hunt teammate 静默 idle 弃用）

火山 400 响应体（proxy_log `81dc4466...` response_body 实证）：
```
{"error":{"code":"InvalidParameter","message":"messages.content.type ... invalid value: `redacted_thinking`, supported values: `text`,`thinking`,`image`,`tool_use`,`tool_result`"}}
```

**不是** prd 初版假设的 coding_plan/URL 矛盾（coding_plan=false + /api/coding/ 路径经查是火山 doubao coding 端点的正常形态，非 bug）。

**真根因**：Claude Code 多轮请求含 `redacted_thinking` content block（Claude 4.x extended thinking 加密块，客户端回传关联上轮 protected thinking）。aidog same-protocol passthrough（anthropic→anthropic, remap=true）**不走 to_anthropic 转换**（后者 anthropic.rs:58 已 filter Unknown 含 redacted_thinking），content 原样透传 → 火山引擎 doubao coding 端点不支持该 type → 400。

**交叉验证**（DB 第二样本 `87e3c500...`）：claude-opus-4-8 → deepseek-v4-pro-260425 同样 400 redacted_thinking。非火山独有，**所有第三方 anthropic-compat 端点共性**。

**修复点缺位**（forward.rs:245-250 host-gated 块）：
```rust
if matches!(target_protocol_enum, Protocol::Anthropic) && !is_official_anthropic_host(&url) {
    strip_thinking_if_unmatched(&mut req_body);   // 处理 thinking block 缺失
    if !is_stream { hoist_mid_messages_system(&mut req_body); }  // role=system 规整
    // 🔴 缺：剥离 redacted_thinking content block（第三方必不支持）
}
```

`error_rule#8 参数错误 retryable=false` 把 400 判为硬错误终止回退 → 正确分类（参数确实错），根治在 converter 层剥离 redacted_thinking，非改分类。

## Requirements

1. 新增 `strip_redacted_thinking_blocks(body: &mut Value)` 函数（forward.rs，strip_thinking_if_unmatched 附近）：遍历 `messages[].content[]`（数组形态），过滤 `type == "redacted_thinking"` 的 block。
2. 在 forward.rs:245 host-gated 块内调用（与 strip_thinking_if_unmatched 并列，**无条件剥**——第三方 anthropic 端点必不支持，redacted 内容加密 opaque 剥离安全；官方 host 走 else 分支不受影响）。
3. 加单测（test_strip_thinking mod 内，复现 redacted_thinking block 被剥离 + 其他 block 保留）。
4. 验证：cargo clippy 0 warning + cargo test 通过。

## Open Questions

（无 —— 根因实证闭环）

## Acceptance Criteria

- [ ] 根因定位（file:line + 响应体实证）
- [ ] 最小修复（说明改了什么、为什么）
- [ ] cargo clippy 0 warning
- [ ] cargo test 通过
- [ ] 若涉及 spec 契约（platform-error-handling / coding_plan flag 一致性）→ sediment 判定

## Out of Scope

- 不改 MiniMax Coding Plan 429（那是上游限流，回退正确）
- 不改 HealthAware 候选选择（11 候选排序正常）
- 不改 error_rule#8 分类逻辑（除非根因证明分类错）

## Technical Notes

- trace 源: `~/.aidog/logs/aidog.2026-07-08-09.log:477-491`
- 关键代码:
  - `src-tauri/src/gateway/proxy/non_success.rs:25,101,107`（400 分类 + decision-A 终止）
  - `src-tauri/src/gateway/proxy/forward.rs:101,286`（路由 + upstream 请求）
  - 火山引擎 coding_plan flag 与 base_url 一致性查 platform 214 配置
- spec 相关: `.trellis/spec/backend/platform-error-handling.md`（C6/C7 + 429 分类 + retryable 语义）, `db-conventions.md`
