# 修复 anthropic 入站请求解析失败 (ContentBlock 未覆盖类型致 400)

## 背景 / 复现
- request id `1da0efa618624022832f6abe4528be91`: 400, 8ms 返回, 未到上游
- group `glm-coding-plan-auto`, model `claude-opus-4-8`, path `/proxy/v1/messages?beta=true`, source_protocol=anthropic, status_code=400
- 同类失败多条: `opus` / `gpt-4o-mini` 全触发同一错误 `"failed to parse request for protocol"`

## 根因
- `adapter/converter.rs:84` anthropic 分支: `serde_json::from_value::<ChatRequest>(body).ok()`
- `ChatRequest.messages[].content` → `MessageContent::Blocks(Vec<ContentBlock>)`
- `ContentBlock`(`adapter/types.rs:55`, `#[serde(tag="type")]`) 仅覆盖 `text` / `tool_use` / `tool_result`(且 `tool_result.content: String`)
- Claude Code / Anthropic 真实请求(尤其 `?beta=true` + opus-4 扩展思考)含 **thinking blocks** / **image blocks** / **tool_result.content 为 array** → `from_value` 整体失败 → `.ok()` 吞掉 serde error → `None` → 400
- `proxy.rs:648` 仅记 `"failed to parse request for protocol"`, **无 serde 错误细节**(诊断盲点, 同类问题难定位)
- `CONVERSION_TODO.md` 阶段 E(thinking)/G(多模态) 待补; 但 TODO 描述是「block 全丢」(假设能解析只是降维), 实际是「整请求 400」(根本解析不了), 比 TODO 现状描述更严重

## 影响
- Claude Code 经 anthropic 入站 + 扩展思考 / 图片 / 富 tool_result → 全部 400, 不可用
- 任何含未覆盖 ContentBlock 类型的 anthropic 请求都失败

## 方案
- **P1 诊断**: parse 失败路径把 serde error 记入 `log.response_body`(替换无细节消息), `i18n::t(ParseRequest)` 文案可附 error
- **P2 容错(核心)**: anthropic 分支 `from_value::<ChatRequest>` 失败时降级 ——
  - 选项 a: `ContentBlock` 加 `Unknown { ... }` 变体(`#[serde(other)]` 不可用于带数据 tagged variant → 用自定义 `Deserialize` 或 `untagged` 兜底), 未知 block 原样保留(透传上游, 不丢)
  - 选项 b: 失败后手动清洗 `messages[].content` 数组, 滤掉非 text/tool_use/tool_result block 后重试解析(可能丢 thinking 内容, 但请求能过)
  - 倾向 a(不丢字段, 对后续 thinking 透传有利), b 作为兜底
- **P3(不在本 task)**: 补全 thinking/image 强类型变体 + signature 透传 → 属 `CONVERSION_TODO` 阶段 E/G, 单独排期

## 验证
- 复现单测: 构造含 `thinking` block + `tool_result.content=array` 的 anthropic body, 改前 `parse_incoming_request` 返回 `None`, 改后返回 `Some(..)` 且请求可路由上游
- 回归: 纯文本 anthropic 请求解析不变
- 集成: 起 proxy, 用 Claude Code 风格请求(带 thinking)命中, 不再 400

## 范围 / 文件
- `src-tauri/src/gateway/adapter/converter.rs` — `parse_incoming_request` 容错
- `src-tauri/src/gateway/adapter/types.rs` — `ContentBlock` 兜底变体(若选 a)
- `src-tauri/src/gateway/proxy.rs:640-655` — parse 失败日志补 serde error
- `src-tauri/src/gateway/adapter/CONVERSION_TODO.md` — 更新现状(解析失败 ≠ 仅丢字段)
- 测试: `adapter/converter.rs` 或 types 相关单测

## 非目标
- thinking signature 回传、image 双向转换、富流式 thinking delta(→ CONVERSION_TODO E/G)
