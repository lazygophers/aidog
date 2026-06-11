# PRD: 添加 mock 平台类型

## 背景
新增 `mock` 平台类型（Protocol 变体），用于测试代理链路 —— 路由到 mock 平台时不转发真实上游，本地生成可控假响应。

## 需求

### 已确认（brainstorm）
- **核心行为**: 可配多场景模拟（返回内容 / HTTP 状态码 / 延迟 ms / 错误 / 429 / 超时 / 流式开关）
- **响应格式**: 按入站协议返对应格式（group.source_protocol = anthropic→anthropic 格式 / openai→openai / gemini→gemini）
- **流式 + token**: 支持流式 SSE（stream=true 时返 SSE 分块）+ 填假 token 用量进 proxy_log

### 细化（用户补充，待澄清具体机制）
- mock 响应的各可控字段值**可逐项指定**：`input_tokens` / `output_tokens` / `cache_tokens` / `status_code`
- 用户原话「根据 role 判断，如果 role = input token 就返回指定的 input token 值，其它类似」「status code / cache tokens / output tokens 都一样（同机制）」
- **[待澄清 A]** 指定机制：通过什么通道在请求里指定这些值？候选：
  - (a) 请求 messages 里某 role/content 约定（如 system message 写 `input_tokens:100`）
  - (b) mock 平台配置（platform.extra）里固定配置每字段值
  - (c) 请求自定义 header / body 字段
  - "role 判断" 的 role 指请求 message 的 role，还是字段名标识？

## 涉及面（待 research agent a92f3ae 补充精确插入点）
- 后端: models.rs Protocol enum + converter.rs 响应构造 + proxy.rs 转发拦截点 + router.rs 平台选择 + proxy_log token 填充
- 前端: api.ts Protocol union + PROTOCOLS 数组 + 平台配置 UI

## 验收标准（初稿）
- mock 平台类型可在前端选择 + 配置
- 路由到 mock 平台返回按入站协议格式的假响应（非流式 + 流式 SSE）
- 各字段（input/output/cache tokens, status_code）按指定机制可控
- proxy_log 记录假 token
- cargo build + tsc + 测试通过

## 待办
1. research agent 完成 → 补插入点
2. 澄清 [待澄清 A] role 判断机制
3. 写 design + 拆 subtask
