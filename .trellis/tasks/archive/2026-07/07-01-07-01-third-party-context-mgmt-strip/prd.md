# PRD — 第三方 anthropic 端点 context_management 致 400 thinking must be passed back

> request_id=1658bb4b09e84e3d8fbfd79379ed2b61 复现。free group → platform 232 (DeepSeek, anthropic 端点), model claude-opus-4-8 → deepseek-v4-flash, source/target=anthropic, upstream 400, retry 6 次全失败。

## 根因 (调查定位)

Claude Code adaptive/summarized 新模式 (thinking.type=adaptive, display=summarized) 不回传 assistant 轮 thinking block, 改用新字段协商:
- `thinking: {type:"adaptive", display:"summarized"}`
- `context_management: {edits:[{type:"clear_thinking_20251015", keep:"all"}]}` — 服务端自动清历史 thinking
- `output_config: {effort:"medium"}`

跨路由到第三方 anthropic-compat 端点 (DeepSeek) 时, 上一次修复 `7219f52` (`strip_thinking_if_unmatched` in `forward.rs`) 只剔 `body.thinking`, **漏剔 `context_management`**。DeepSeek 不认官方 context_management 协商, 见该字段判 "thinking mode", 严格要求每个 assistant 轮 content 回传 thinking block, 缺失即 400:
```
The `content[].thinking` in the thinking mode must be passed back to the API.
```

实测 body: thinking 字段已剔 (null), 但 context_management 保留 → 仍 400。52 条 messages, 所有 assistant 轮均无 thinking block (adaptive 模式特性)。

## 目标
扩展 `strip_thinking_if_unmatched` (或等价位置): 命中 thinking unmatched 时, **同时剔 `context_management` 字段**, 让第三方端点不误判 thinking mode。

## scope
- 改 `src-tauri/src/gateway/proxy/forward.rs` `strip_thinking_if_unmatched` (或紧邻 forward 处理段)
- 命中条件不变 (thinking on + assistant 轮缺 thinking block + 非官方 anthropic host): 现剔 thinking → 改剔 thinking **且**剔 context_management
- 加单测: thinking adaptive + context_management + assistant 无 thinking block → 两字段皆剔
- 不动 output_config (错误未提及, YAGNI; 若后续报错再补)

## 非目标
- 不改 Claude Code 行为 (adaptive 模式是官方特性)
- 不改官方 anthropic host 路径 (官方认 context_management, 不受影响)
- 不实现 output_config / 其他新字段兼容 (本次未触发)

## 验收
1. 复现场景 (thinking adaptive + context_management + assistant 无 thinking block + 第三方 anthropic host) → 上游 body 不含 thinking 也不含 context_management
2. 官方 anthropic host 路径不剔 context_management (协商正常)
3. thinking 齐全 (assistant 轮带 thinking block) 保留 context_management + thinking
4. `cargo test --lib` 全绿 (含新单测)
5. `cargo clippy --all-targets` 0 err (block accepted warning 排除)
6. `yarn build` 0 err (本修复纯 Rust, 前端无改, 预期无影响)

## 风险
- DeepSeek 可能还认其他 thinking 信号 (output_config / metadata 等) → 本修复若不够, 二次复现再补
- context_management 未来可能有非 thinking 相关 edits → 剔整个字段过于粗暴时改为只剔 clear_thinking_* edits (exec 据上游语义定)

## 调度
- 单文件单函数修复, 轻量模式 (单 subtask)
- 复用 7219f52 测试结构 (test_strip_thinking mod)
