# 协议转换完善计划（adapter）

> 目标：把现有「纯文本对话」转换骨架，逐步补齐 **工具调用 / thinking / 多模态 / 富流式 / 入站参数**，
> 让 aidog 的 5 协议互转达到生产级（参考 CLIProxyAPI `sdk/translator/`，但贴合 aidog 的 N+N 中枢架构）。
>
> 维护方式：每完成一项勾掉 `[ ]` → `[x]`，并在末尾「进度日志」追加一行。

---

## 0. 架构现状（已确认）

**转换拓扑：N+N 中枢（canonical hub），不是 N×N 直转**
```
入站 body → parse_incoming_request(incoming) → ChatRequest(中立模型) → serialize_to_target(target) → 出站 body + api_path
                                                       ↓ 流式
                       parse_sse(target) → ChatStreamEvent → to_sse(incoming)
```
调度全在 `converter.rs`（5 个函数）。字段级转换在各协议文件：
`anthropic.rs` / `gemini.rs` / `openai.rs` / `openai_responses.rs` / `openai_completions.rs`。

**已支持协议（converter.rs match 分支）**：Anthropic(+Beta) / Gemini / OpenAIResponses / OpenAICompletions / OpenAI(默认)。

**中立模型现状（models.rs）**：
- `ChatRequest`(:230)：`model/messages/temperature/max_tokens/top_p/stream/tools(Option<Vec<Value>>)/extra(Map)/tool_calls...`
- `Role`(:176)：System/User/Assistant/Tool/Function
- `Content`(:186)：`Text(String)` | `Parts(Vec<Value>)`  ← **已能装 block 数组，但 adapter 没用**
- `ChatMessage`(:194)：`role/content/tool_calls(Option<Vec<Value>>)/tool_call_id` ← **字段已在，adapter 写死 None**
- `ChatStreamEvent`(:330)：`TextDelta/MessageStart/Usage/Done` ← **缺工具/思考变体**

---

## 1. 当前实现的局限（这就是要补的）

| 局限 | 位置 | 后果 |
|---|---|---|
| content 全降维成纯文本 | `anthropic.rs parse_anthropic_content`、`gemini.rs parse_gemini_request` 只抽 `text` | tool_use/image/thinking block 全丢 |
| 工具调用没接 | `anthropic.rs parse_anthropic_message` 写死 `tool_calls: None`；各 serialize 不输出 tools | 不能用 function calling |
| 入站没读 max_tokens/temperature | anthropic/gemini 的 parse 没提取 | 出站只能给默认值，客户端设置丢失 |
| 流式只有文本 delta | `ChatStreamEvent` 仅 TextDelta | 工具/思考的流式无法透传 |
| thinking/reasoning 没处理 | 全协议 | 思考链丢失 |
| 多模态没处理 | 全协议 | 图片输入不支持 |

---

## 2. 实施原则（贴合 aidog）

1. **不照抄 CLIProxyAPI 的 bytes→bytes(gjson/sjson)** —— aidog 是中枢架构 + serde 强类型，照抄会破坏架构。吸收它的**字段映射清单**和**block 保留思想**即可。
2. **守卫式搬运** —— 每字段 `if let Some(...)` 判断存在才写，缺失不强加（学 CLIProxyAPI 的 `.Exists()`）。
3. **稀疏/未建模字段塞 `ChatRequest.extra`** —— thinking signature、各家特有字段不丢。
4. **每阶段配 fixture 测试**（converter.rs 已有 `#[cfg(test)] mod tests`，往里加）。
5. **一次只动一个能力 + 一个协议对**，编译+测试通过再下一个。
6. 复用现有 `Content::Parts(Vec<Value>)` 装 block，别新造结构（除非确有必要）。

---

## 3. 分阶段任务清单（从简到难）

### 阶段 A — 地基：中立模型升级（必须最先做）
- [ ] A1. `Content` 增强：让 `Parts(Vec<Value>)` 真正承载 block（text/tool_use/tool_result/image/thinking）。评估是否需要把 `Vec<Value>` 升级为强类型 `Vec<ContentBlock>` enum，还是保持 `Value` 灵活（建议先 `Value` + helper，降低改动面）。
- [ ] A2. 给 `Content` 加 helper：`as_text()`（已存在？确认）、`blocks()`、`push_block()`，让 adapter 统一走。
- [ ] A3. `ChatMessage.tool_calls` / `tool_call_id` 确认序列化语义（已有字段，定义清楚每协议怎么填）。
- [ ] A4. `ChatStreamEvent` 扩展变体（先定义，后面流式用）：
  - `ToolCallStart { index, id, name }`
  - `ToolCallArgsDelta { index, partial: String }`
  - `ThinkingDelta(String)`
  - `Finish { reason: String }`
- [ ] A5. `ChatRequest` 入站参数：确保 parse 时填充 `max_tokens/temperature/top_p`（现在没填）。

### 阶段 B — 测试框架（先搭，后面每步都用）
- [ ] B1. 在 `converter.rs` tests 模块建 fixture：`{协议}-req.json` / 期望输出 json 成对。
- [ ] B2. 覆盖现有纯文本路径（回归基线）：OpenAI↔Anthropic↔Gemini 各方向纯文本。

### 阶段 C — 入站参数提取（最简单，先练手）
- [ ] C1. `anthropic.rs parse_anthropic_request`：读 `max_tokens/temperature/top_p/stop` 填入 ChatRequest。
- [ ] C2. `gemini.rs parse_gemini_request`：读 `generationConfig.{maxOutputTokens/temperature/topP}` 填入。
- [ ] C3. 出站补全：`gemini serialize` 把 `max_tokens` → `generationConfig.maxOutputTokens`（现在没映射）。

### 阶段 D — 工具调用 tool calling（核心，收益最大）
> 字段映射见文末附录。三件套必须配套：tools 定义 + assistant tool_use + user tool_result，用 id 关联。
- [ ] D1. `ChatRequest.tools`：定义中立工具格式（建议用 OpenAI function 格式作中立基准）。
- [ ] D2. Anthropic 入站：parse `tools`(input_schema)、assistant content 里的 `tool_use` block、user 里的 `tool_result` block → 中立模型。
- [ ] D3. Anthropic 出站：中立 tools/tool_calls → Anthropic `tools`/`tool_use`/`tool_result`。
- [ ] D4. Gemini 入站：parse `tools[].functionDeclarations`、`functionCall`/`functionResponse` part → 中立。
- [ ] D5. Gemini 出站：中立 → Gemini `functionDeclarations`/`functionCall`/`functionResponse`。
  - ⚠️ **Gemini 无 tool id**，靠 `name` 关联。Anthropic/OpenAI→Gemini 丢 id；Gemini→它们时需自生成 id 并按顺序配对。
  - ⚠️ OpenAI `arguments` 是 **JSON 字符串**，Anthropic `input` / Gemini `args` 是**对象** → parse/stringify 转换。
- [ ] D6. `tool_choice` 映射（auto/none/required/具名）。

### 阶段 E — thinking / reasoning
- [ ] E1. 请求侧开关映射：Anthropic `thinking:{type,budget_tokens}` ↔ Gemini `generationConfig.thinkingConfig.thinkingBudget` ↔ OpenAI `reasoning_effort`/`reasoning.effort`。
- [ ] E2. 响应侧内容：Anthropic `thinking` block ↔ Gemini `thought` part ↔ OpenAI `reasoning_content`(扩展字段)。
- [ ] E3. ⚠️ **signature 处理**：Anthropic thinking block 带加密 `signature`，回传 Anthropic 必须带回否则报错。塞 `ChatRequest.extra` 缓存，或该轮不回传 thinking（参考 CLIProxyAPI signature cache）。

### 阶段 F — 富流式 SSE（依赖 A4 的事件变体）
- [ ] F1. 各协议 `parse_*_sse` 产出新事件变体（工具/思考）。
- [ ] F2. 各协议 `to_*_sse` 消费新事件。
- [ ] F3. ⚠️ **流式状态机**：工具参数是分片增量（Anthropic `input_json_delta` / OpenAI `tool_calls[].function.arguments` 累加）。`to_sse` 需跨 chunk 维护状态（ToolIndex/ToolID/ToolName）。当前 `to_sse` 是无状态函数，需引入状态载体（参考 CLIProxyAPI `ConvertState`）。

### 阶段 G — 多模态
- [ ] G1. 图片：OpenAI `image_url`(data: URL / http url) ↔ Anthropic `image.source`(base64/url) ↔ Gemini `inlineData`/`fileData`。
- [ ] G2. data URL 拆解 media_type + base64。

---

## 4. 推荐执行顺序

```
A(地基) → B(测试) → C(入站参数，练手) → D(工具，核心) → E(thinking) → F(富流式) → G(多模态)
```
**先做 A+B+C**：地基 + 测试 + 最简单的参数提取，把链路盘活、回归网建好，再进 D 工具调用（收益最大的硬骨头）。

每个阶段内：**一个协议对做通再做下一个**（如 D 先 OpenAI↔Anthropic，绿了再 OpenAI↔Gemini）。

---

## 5. 参考资料

**官方 schema（字段语义唯一权威）**
- Anthropic Messages：docs.anthropic.com/en/api/messages ＋ tool-use 页
- Gemini generateContent：ai.google.dev/api/generate-content ＋ function-calling 页
- OpenAI Chat：platform.openai.com/docs/api-reference/chat

**现成实现（抄字段映射）**
- CLIProxyAPI（本次对标，clone 在 /tmp/CLIProxyAPI）：`sdk/translator/claude/openai-to-claude.go`(656 行，最完善样板) / `claude/gemini-to-claude.go` / `gemini/claude-to-gemini.go`。手法：gjson 读 + sjson 写 + `.Exists()` 守卫 + 流式 `ConvertState` 状态机。
- LiteLLM（最全）：`litellm/llms/<provider>/chat/transformation.py`
- Vercel AI SDK：`packages/<provider>/src/convert-to-*`

---

## 附录：核心字段映射表

### OpenAI ↔ Anthropic（请求）
| 概念 | OpenAI | Anthropic |
|---|---|---|
| system | messages 里 role=system/developer | 顶层 `system` |
| max_tokens | `max_tokens`/`max_completion_tokens` | `max_tokens`(必填,默认 4096) |
| 工具定义 | `tools[].function{name,description,parameters}` | `tools[]{name,description,input_schema}` |
| 模型调工具 | `message.tool_calls[]{id,function{name,arguments(JSON字符串)}}` | content `{type:tool_use,id,name,input(对象)}` |
| 工具结果 | role=tool, `tool_call_id`, content | content `{type:tool_result,tool_use_id,content}` |
| 图片 | `image_url{url}` | `{type:image,source{base64/url}}` |
| tool_choice | auto/none/required/`{type:function,function{name}}` | `{type:auto/any/tool,name}` |

### OpenAI ↔ Anthropic（响应）
| Anthropic | OpenAI |
|---|---|
| content text block | `message.content` |
| content tool_use block | `message.tool_calls[]`(input→arguments 字符串) |
| content thinking block | `message.reasoning_content`(扩展) |
| stop_reason(end_turn/max_tokens/tool_use) | finish_reason(stop/length/tool_calls) |
| usage.input/output_tokens | usage.prompt/completion/total_tokens |

### Anthropic ↔ Gemini（结构差异）
| 概念 | Anthropic | Gemini |
|---|---|---|
| 消息数组 | `messages` | `contents` |
| role | user/assistant | user/model |
| 文本 | content(str/block[]) | `parts[].text` |
| system | 顶层 `system` | 顶层 `systemInstruction.parts[].text` |
| 参数 | 顶层 max_tokens 等 | `generationConfig.{maxOutputTokens/temperature/topP}` |
| 工具定义 | `tools[]{name,input_schema}` | `tools[].functionDeclarations[]{name,parameters}` |
| 工具调用 | `{type:tool_use,id,name,input}` | `parts[].functionCall{name,args}`（无 id） |
| 工具结果 | `{type:tool_result,tool_use_id,content}` | `parts[].functionResponse{name,response}`（靠 name 关联） |
| thinking 开关 | `thinking{type,budget_tokens}` | `generationConfig.thinkingConfig.thinkingBudget` |
| thinking 内容 | `{type:thinking,thinking,signature}` | `parts[].{thought:true,text}`（无 signature） |

### 流式事件对照
| 中立 ChatStreamEvent(拟扩展) | Anthropic SSE | OpenAI SSE |
|---|---|---|
| MessageStart | message_start | role delta |
| TextDelta | content_block_delta/text_delta | content delta |
| ToolCallStart | content_block_start(tool_use) | tool_calls delta(id/name) |
| ToolCallArgsDelta | content_block_delta/input_json_delta | tool_calls[].function.arguments 增量 |
| ThinkingDelta | content_block_delta/thinking_delta | reasoning_content delta |
| Finish | message_delta(stop_reason) | finish_reason 终块 |
| Done | message_stop | data: [DONE] |

---

## 进度日志
- 2026-06-13 创建本计划。现状：5 协议「纯文本对话」双向骨架已完成；工具/thinking/多模态/富流式/入站参数待补。
