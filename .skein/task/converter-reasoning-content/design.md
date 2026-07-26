# converter: 5 协议 5×5 互转矩阵 + reasoning_content 映射 — 详细设计

## 现状（根因）

转换层 `adapter/converter/response.rs::convert_response` 当前只覆盖 **openai(chat completion) → anthropic(messages)** 真转换，其余跨协议组合返回 `None`（调用方透传上游原文）。`parse_openai_response:13,19-23` 注释明说丢弃 `reasoning_content`。

本案 request `cd7ff24d`：source=anthropic, target=DB 落 `sensenova`（平台名，bug1），实际 endpoint 协议=openai（sensenova preset endpoint `{protocol:"openai",client_type:"codex_tui"}`）。上游 sensenova deepseek-v4-flash 把正文全放 `message.reasoning_content`（content:""），`parse_openai_response` 丢弃 → 转换后兜底空 text。

## 架构决策：路 A（内部归一，O(N) parse+render）

**不用路 B（点对点 5×5 函数）** —— 20 个函数爆炸，新增协议要加 5 个转换函数。

```
上游 body ──parse_<wire>(body)──▶ NonStreamResponse (内部归一)
                                      │
                                      ▼
                              render_<client>(parsed) ──▶ 客户端 body
```

5×5 矩阵 = 5 个 parse + 5 个 render。新增协议只加 1 parse + 1 render。

## 数据流（非流式）

`proxy/finish.rs:61` 调 `convert_response(body, wire_protocol, client_protocol, model)`：

1. `parse = PARSE_TABLE[wire_protocol](body, model)` → `Option<NonStreamResponse>`
2. `RENDER_TABLE[client_protocol](parse, model)` → `Option<Value>`
3. wire == client（同协议）→ 透传，跳过 parse/render
4. parse 或 render 任一缺该协议实现 → 回退透传（向后兼容，渐进覆盖）

## 数据流（流式）

`parse_sse(data, wire)` → `ChatStreamEvent`（已有，补全 5 协议分支）
`to_client_sse(event, client_protocol, model)` → SSE 串（已有，补全 5 协议分支）

## 内部归一扩展：NonStreamResponse

现状（`response.rs:23-35`）：text / tool_uses / stop_reason / usage。**加 `reasoning: Option<String>`**（思维链文本，按 wire 语义提取）。

各协议 parse 提取 reasoning：
| wire | reasoning 来源 |
|---|---|
| anthropic | content 数组 `{type:"thinking"}` 块的 thinking 字段（剥离 signature） |
| openai / openai_completions | `message.reasoning_content` |
| openai_responses | `output[].summary[]` 或 reasoning items（Responses API reasoning 语义） |
| gemini | `candidates[0].content.parts[{thought:true}].text`（Gemini 2.5 thinking） |

## render 矩阵（client_protocol → 输出格式）

| client | text | reasoning | tool_use | 主体结构 |
|---|---|---|---|---|
| anthropic | `{type:text}` | `{type:text}` 排首位（方案 B，禁 thinking 块避 signature 风险） | `{type:tool_use}` | messages 响应体 |
| openai | `message.content` | 拼 `message.content` 前缀（或 `reasoning_content` 回填） | `tool_calls[]` | chat completion 响应体 |
| openai_completions | `choices[0].text` | 拼 text 前缀 | 不支持（legacy 无 tool） | legacy completion 响应体 |
| openai_responses | `output[].text` | `output[].summary` | `output[].function_call` | responses 响应体 |
| gemini | `candidates[0].content.parts[{text}]` | `parts[{thought:true,text}]` | `parts[{functionCall}]` | generateContent 响应体 |

**reasoning 跨协议策略**：入站归一时保留为 `reasoning: Option<String>`，出站时按 client 协议语义放对应位置。**目标 anthropic 永远走 text 块（方案 B）**，禁生成 `{type:thinking}`（无真实 signature，CC 多轮校验拒）。

## bug1 修复：target_protocol 落平台名

`proxy/forward.rs:75`：`target_protocol_enum = matched_ep.map(|ep| &ep.protocol).unwrap_or(&route.platform.platform_type)`。

**本案证据**（request cd7ff24d）：
- DB: `source_protocol=anthropic, target_protocol=sensenova`（sensenova 是平台名非协议）
- upstream URL `https://token.sensenova.cn/v1/chat/completions` = sensenova preset 的 **openai endpoint**（base `https://token.sensenova.cn/v1`）
- upstream body = openai chat 格式（`messages[].role/content`）
- sensenova preset endpoints: `[{protocol:openai, client_type:codex_tui}, {protocol:anthropic, client_type:claude_code}]`

**矛盾**：upstream URL/base_url 用的是 openai endpoint，但 target_protocol 落 sensenova（=unwrap_or 的 platform_type）。说明 `matched_ep=None`（走 fallback），可实际出站却用了 openai endpoint 的 base_url。

**真相待 s2 深挖**（候选根因）：
- `select_endpoint_for_protocol(endpoints, source_protocol=anthropic)` 按 source_protocol 选 endpoint，若 source_protocol 推断异常（path `/proxy/chat/compate` typo 非标准）→ matched_ep 逻辑链断裂
- 或 matched_ep 命中但 ep.protocol 字段空/默认致 fallback
- 或 source_protocol 推断本身把 openai body 误判成 anthropic（path 推断 vs body 实际格式不一致）

**核心不变量（用户强调）**：endpoint 已声明 protocol 字段（真值源），target_protocol 必须落 5 协议之一（anthropic/openai/openai_responses/openai_completions/gemini），**禁落平台名**（sensenova/glm/kimi 等几十个 platform_type 别名）。matched_ep 命中 → 用 ep.protocol；matched_ep=None → 不静默 fallback platform_type，按平台 preset default endpoint[0].protocol 或显式 route fail。

s2 执行时先 trace 本案 matched_ep 为何 None / ep.protocol 为何没取到，定点修 select_endpoint 或 fallback 逻辑。

## 取舍

- **方案 B（reasoning→text）非 thinking 块**：调研佐证 anthropic thinking 块需 cryptographic signature（[TrueFoundry](https://www.truefoundry.com/docs/ai-gateway/chat-completions-extended-thinking) / [LiteLLM #8927](https://github.com/BerriAI/litellm/discussions/8927)），第三方 reasoning_content 纯文本无签名 → 转 thinking 块 signature 空串，CC 多轮校验拒。方案 B 最稳。
- **路 A 非路 B**：O(N) vs O(N²) 函数数，新增协议成本悬殊。
- **渐进覆盖非一步到位**：缺某协议 parse/render 时回退透传（现状行为），避免一次性大改破坏现有请求。5 协议全实现后无透传兜底。
- **openai_completions 最小兼容**：preset 无 endpoint 用它（legacy），parse 复用 openai chat 同构（choices[0].message.content），render 输出 `choices[0].text` 格式。不深究 legacy 差异（YAGNI）。

## 可能性分支（研究期留痕，不进正文/subtask）

- 若未来要支持真实 thinking signature 透传（官方 anthropic 上游 → anthropic 客户端）：NonStreamResponse.reasoning 加 `signature: Option<String>` 字段，anthropic parse 时保留 signature，anthropic render 时若 signature 非空才出 thinking 块。触发条件：用户接官方 anthropic 端点且要保留思维链签名。
- 若未来新增第 6 协议（如 bedrock）：路 A 只加 1 parse + 1 render，矩阵自动 6×6 覆盖。
