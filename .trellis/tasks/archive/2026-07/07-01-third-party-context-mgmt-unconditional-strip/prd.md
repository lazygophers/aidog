# PRD — 第三方 anthropic 端点首轮 context_management 致 GLM 1210

> 复现: request_id=3a76c2971fb441bd83ff982f36545ecf (group=glm, platform_id=38, 上游 https://open.bigmodel.cn/api/anthropic/v1/messages, 400 code 1210 "API 调用参数有误")。

## 根因

`forward.rs::strip_thinking_if_unmatched` 仅在 `has_unmatched_assistant` (历史有 assistant 轮缺 thinking block) 时剔 `context_management`。**首轮请求** (messages 仅 user, 无 assistant 历史) `has_unmatched_assistant`=false → 不触发 strip → `context_management.edits=[clear_thinking_20251015]` 直传第三方端点。

- DeepSeek (旧复现 request_id=1658bb4b): 认 `context_management` 字段 → 判 thinking mode → 报 "thinking must be passed back" (有 assistant 历史场景)
- **GLM (本次)**: 直接不认 `context_management` 字段 → 报 1210 "API 调用参数有误" (**首轮即触发**, 无需 assistant 历史)

两平台同字段不同表现, 共同根因: `context_management` 是官方 anthropic 服务端协商特性 (CC adaptive/summarized 模式 clear_thinking_20251015), 第三方 anthropic-compat 端点普遍不实现该协商, 保留该字段对第三方无益仅有风险。

## 修复

`context_management` 剔除**不再 gate 于 `has_unmatched_assistant`** —— 改为 thinking 开启时无条件剔 (context_management 与 thinking 同源, 仅 CC adaptive 模式发送; 第三方端点不认该协商, 保留无意义)。

`thinking` 字段保持原 `has_unmatched_assistant` 逻辑 (齐全时第三方能正常处理 thinking block, 不剔)。

### 改动点 (forward.rs::strip_thinking_if_unmatched)
- 把 `obj.remove("context_management")` 从 `if has_unmatched_assistant { ... }` 块内移出, 改为 `if thinking_on { obj.remove("context_management"); }` (紧跟 thinking_on 判定后, has_unmatched 计算前)
- `thinking` remove 仍留在 `if has_unmatched_assistant` 块内
- 更新 doc 注释: context_management 无条件剔 (第三方不认, 首轮 GLM 1210 + 有历史 DeepSeek 400 两类复现); thinking 仍按 unmatched 剔
- 函数名 `strip_thinking_if_unmatched` 语义不再完整覆盖 (context_management 无条件), 注释说明或改名 `strip_unsupported_anthropic_fields` (改名涉及调用点, 权衡: 单调用点 forward.rs:238, 改名成本低, 语义更准)

## 调用点 (不变)

`forward.rs:238-240`:
```rust
if matches!(target_protocol_enum, Protocol::Anthropic) && !is_official_anthropic_host(&url) {
    strip_thinking_if_unmatched(&mut req_body);
}
```
host-gated (仅第三方端点), 官方 anthropic 不受影响。

## 验收

1. 首轮请求 (messages 仅 user, 无 assistant) + thinking adaptive + context_management → 经第三方端点 → context_management 被剔 (单测断言)
2. 有 assistant 历史 + thinking 齐全 → context_management 仍被剔 (第三方不认), thinking 保留 (单测)
3. 有 assistant 历史 + thinking unmatched → 两字段皆剔 (现有行为不变, 单测)
4. 官方 anthropic 端点 → 不进 strip (host-gated, 两字段保留)
5. `cargo test` (strip_thinking 测试模块 + 全量) + `cargo clippy` + `yarn build` 全绿
6. memory `third-party-anthropic-thinking-strip` 更新 (第 3 次踩坑: 首轮 + GLM 1210)

## 非目标

- 不动 `thinking` 字段的 unmatched 逻辑 (齐全时保留, 已验证第三方能处理)
- 不动 host-gated 判定 (`is_official_anthropic_host`)
- 不剔 `output_config` (YAGNI, 未触发报错)

## 风险

- context_management 无条件剔后, 若未来官方 anthropic 端点也经此路径 → 误剔。缓解: host-gated (`!is_official_anthropic_host`) 保证官方端点不进 strip, 无条件剔仅在第三方端点生效
- 改名 `strip_unsupported_anthropic_fields` 涉及单测 mod 引用 + 调用点, 需同步

## 调度

- 排队: 槽位满 (deeplink-share + opencode-go-baseurl-fix in_progress), 等 opencode finish 释放槽位后 start
- bug fix 根因清晰, 无 brainstorm 决策点, PRD 直定
