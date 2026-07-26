# converter: reasoning_content 映射到 anthropic thinking block — PRD (主入口)

## 目标
- [ ] endpoint 协议层 5 种(anthropic/openai/openai_responses/openai_completions/gemini)支持全 5×5 相互转换(25 格, 减对角 20 格真转换)
- [ ] 修复 bug1: endpoint 匹配失败 fallback 致 target_protocol 落平台名(如 sensenova)而非真协议(如 openai)
- [ ] 修复 bug2: parse_openai_response 丢弃 reasoning_content 致转换后 content 空(本案 sensenova deepseek-v4-flash 正文全在 reasoning_content)
- [ ] reasoning_content 映射方案 B: 转 text 块排 content 首位(调研佐证 anthropic thinking 块需 cryptographic signature, 第三方纯文本无签名转 thinking 块会被 CC 多轮拒)
- [ ] 流式(parse_sse + to_client_sse)与非流式(convert_response)对称覆盖 5×5
## 边界
- 转换层: gateway/adapter/converter/(request|response|sse).rs + adapter/{anthropic,openai,gemini}/ 各协议 parse/render 函数
- 路由层: gateway/proxy/forward.rs (target_protocol 赋值 + endpoint 匹配 fallback)
- 内部归一: NonStreamResponse (adapter/converter/response.rs) + ChatStreamEvent (adapter/types.rs)
- 非目标: 不改 platform-presets.json 协议别名(sensenova 等平台名保留, 只修 wire protocol 路由)
- 非目标: 不引入第三方转换库(LiteLLM 等), 自研内部归一
- 非目标: openai_completions(legacy /v1/completions) preset 无 endpoint 配置, 按最小兼容处理(parse 复用 openai chat 同构, render 补 completions 格式), 不深究 legacy 差异
## 验收标准
- [ ] 5 协议(anthropic/openai/openai_responses/openai_completions/gemini)任意 source→target 组合, convert_response 返回 Some(真转换体), 非透传(20 格真转换全覆盖)
- [ ] parse_sse + to_client_sse 覆盖 5 协议全组合, 流式响应格式正确
- [ ] bug1: target_protocol 不再落平台名(sensenova/glm/kimi 等), endpoint 匹配失败时仍落真协议或显式报错
- [ ] bug2: reasoning_content 非空时, 转换后 anthropic content 数组首位含 reasoning text 块, 正文 text 块随后
- [ ] 本案 request cd7ff24d 重放: 上游 reasoning_content 内容出现在回客户端响应
- [ ] cargo test 全绿(test_response.rs 现有断言 reasoning 被丢的需改 + 新增 5×5 转换测试)
- [ ] cargo clippy 0 warning
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list converter-reasoning-content`)
