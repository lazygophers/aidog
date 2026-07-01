# PRD — GLM anthropic 端点多轮 tool_use 请求 1210 参数有误

> 用户报 (/trellisx-flow fix:request_id=7c8629eadb074648a71858ae388ea550): GLM anthropic 端点 400 code 1210「API 调用参数有误, 请检查文档」。复现 request_id=7c8629 + 历史多条。

## 现状 (main DB 取证, ~/.aidog/aidog.db)

端点: `https://open.bigmodel.cn/api/anthropic/v1/messages` (GLM 官方 anthropic-compat)
报错: `{"type":"error","error":{"type":"invalid_request_error","code":"1210","message":"[1210][API 调用参数有误...]"}}`

### 已排除字段层根因 (字段存在性与成败无关)

GLM 历史请求 (近 20 条) 字段矩阵, 成功 200 与失败 400 **字段集完全一致**:

| 字段 | 成功请求 | 失败请求 | 结论 |
|---|---|---|---|
| `context_management` | 修复后已剔 (7c8629 顶层 keys 无此字段) | — | 前任务修复生效, 非本次根因 |
| `output_config:{effort}` | 成功请求也有 (8886d4/640983/ff7d) | 失败也有 | ❌ 非根因 |
| `metadata:{user_id}` | 有 | 有 | ❌ |
| `system[].cache_control.ttl:"1h"` | 有 | 有 | ❌ |
| `thinking:{summarized,adaptive}` | 有, sig_len=0 | 有, sig_len=0 | ❌ 结构一致 |
| `messages[].role=system` (非标) | 成功也有 (b59309/80c206) | 失败也有 | ❌ GLM 实际接受 |
| `tool_reference` content block | 成功也有 (8886d4/640983/b39ffb) | 失败也有 | ❌ |

### 失败请求唯一共同模式

- **≥2 个 assistant 轮 + 多轮 tool_use/tool_result** (7c8629: 8 msgs, 2 assistant 轮, 含 ToolSearch tool_use + tool_result 带 tool_reference)
- 首轮 / 简单轮请求 (80c206/b59309/9671c0) 均 200
- 但**非确定性**: 成功请求 8886d4/640983/ff7d 同样多轮 + tool_reference 却 200

→ 字段二分已穷尽, 成败由 **messages 实际内容/结构组合** 决定, 非 aidog 当前 strip 规则覆盖。需 agent deep diff 失败 vs 成功 body 实际内容定位 GLM 校验触发点。

## 目标

定位 GLM anthropic 端点对**复杂多轮 tool_use 请求**判 1210 的具体触发条件, 在 aidog 转换层 (`forward.rs::strip_thinking_if_unmatched` 邻近或新 strip 函数) 加最小规整/剔除, 使复现请求 200。不破坏官方 anthropic + 其他第三方 (DeepSeek 等) 端点。

## agent 任务 (定位 + 修复合一)

1. **取证**: 读 ~/.aidog/aidog.db (sqlite3) 对比失败请求 (7c8629, 840437, 99e90b, 9505c5, 24a941, 3a76c297, 7bbe8eb) vs 成功请求 (8886d4, 640983, ff7d5, e82047, 78ace7, 6ad0a1, b39ffb) 的 upstream_request_body, 全文 diff 定位决定性差异 (非字段存在性, 而是值/内容/嵌套结构)。重点:
   - tool_use 的 `input` 内容 (失败请求 ToolSearch input vs 成功请求别的 tool input)
   - tool_result 的 content block 组合 (tool_reference 数量 / 顺序 / 是否混 text)
   - assistant thinking 文本内容是否含触发校验的 token
   - messages role 交替合法性 (user→system→assistant→user→system→... 是否 GLM 要求严格 user/assistant 交替, system 必须顶层)
2. **假设验证**: 基于 diff 结论形成最小假设 (如「GLM 要求 messages 内 role 严格 user/assistant 交替, system 必须提升到顶层 system 字段」或「tool_result 内非 text/image block 需规整」), 用成败两组请求交叉验证
3. **最小修复**: 在 `src-tauri/src/gateway/proxy/forward.rs` host-gated strip 区 (`is_official_anthropic_host(url)==false` 分支) 加规整逻辑; 复用现有 `strip_thinking_if_unmatched` 模式
4. **回归**: 改造后的 body 用 curl 打 GLM 端点验证 (需 main 提供 api_key, 或 agent 用现有测试架构 mock); `cargo test` (forward/test_strip_thinking 等) + `cargo clippy` 0 warning
5. **不破坏**: 官方 anthropic host 不进 strip 分支 (host-gated); DeepSeek 等其他第三方同 host-gated, 行为不变或同步改善

## 验收

1. 失败请求模式 (≥2 assistant + 多轮 tool_use) 经修复后规整 body 打 GLM 200 (或至少不报 1210)
2. `cargo test` + `cargo clippy` 全绿
3. 官方 anthropic host 路径不受影响 (host-gated 分支隔离)
4. 新增测试覆盖定位到的根因 (test_strip_thinking 模块扩展或新 test fn)

## 非目标

- 不改 GLM 后端 (第三方, 无法改)
- 不剔除 GLM 实际支持的字段 (避免误伤)
- 不处理 429/限流 (757021 等, 非本任务)
- 不重构 forward.rs (仅加最小规整)

## 风险

- 根因可能是 GLM 后端间歇性/内容敏感 (非 deterministic 字段问题), 若 diff 无决定性差异 → agent 标 `需要:` 报告, main 裁定是否接受「GLM 后端 bug, aidog 侧兜底 strip 多余内容」或放弃
- curl 实测需 api_key (敏感), agent 用 mock 测试架构验证即可, 禁硬编码 key

## 调度

- bug fix, 槽位 1/2 (deeplink-share parent 占 1), 本 task 占第 2 槽
- 定位 + 修复合一, 单 bug-hunt agent
