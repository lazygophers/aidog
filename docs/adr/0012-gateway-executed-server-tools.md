# ADR 0012: 网关代执行服务端工具，并以 HTTP MCP 取代 stdio MCP 进程

## 背景

Claude Code 的 WebSearch 发出一个独立子请求：工具只有服务端工具 `web_search_20250305`，强制模型调用，
客户端读响应中的 `server_tool_use` 与 `web_search_tool_result` 块（Claude Code 2.1.318 程序文本）。
服务端工具只有 Anthropic 官方能执行；AiDog 转往其它平台时丢弃它（`aidog_adapter/src/types.rs:468`），
经 AiDog 的网页搜索因此拿不到结果。同理，Claude Code 的工具按需加载依赖上游理解 `defer_loading` 与
`tool_reference`（https://code.claude.com/docs/zh-CN/mcp#tool-search-through-a-gateway），非官方上游不支持。

AiDog 已有的搜索能力只在 `aidog-kernel mcp` 这个 stdio 子进程里，调用不经网关，无从记录。

## 决策

1. **网关代执行**：请求含服务端工具 `web_search_*` / `web_fetch_*`，且选中的平台不是 Anthropic 官方时，
   AiDog 不调用模型，直接执行一次该工具，按官方结果块格式应答（流式与非流式）。
2. **代执行的请求不进代理日志，只进工具日志**（用户 2026-11-09 拍板）。代理日志的定义随之加例外；
   流量统计不计这些请求。
3. **工具按需加载模拟**：所有非 Anthropic 官方平台上，延迟加载的工具只在被 `tool_reference` 点名后才发往上游，
   `tool_reference` 块降级为文字。
4. **HTTP MCP 取代 stdio MCP**：代理端口提供 `POST /mcp`（Bearer 分组名 + 只许本机），删除 `aidog-kernel mcp`
   子命令；AiDog 写入客户端的 mcpServers 配置改为该地址。
5. **搜索引擎内置**：免 key 引擎以 Rust 实现于 AiDog 内，出站使用浏览器指纹模拟；与 key 服务并列为可开关的引擎列表。

## 后果

- 代执行时搜索质量取决于 AiDog 的引擎，与官方搜索不同；官方平台不受影响。
- 代执行请求在代理日志与流量图中不可见，排查需看工具日志。
- 工具在 AiDog 未运行时不可用（网关本身也需运行，无额外约束）。
- 新增一个浏览器指纹模拟依赖。

## 落选

- 由上游模型循环决定搜索次数：流式拼装复杂、产生模型费用；Claude Code 侧提示本就要求只搜一次。
  仅当出现需要多轮搜索的客户端时重开。
- 代执行请求同时写代理日志：被用户否决。若流量统计需要计入这些请求，重开。
- 保留 stdio MCP 与 HTTP 并存：两套入口，stdio 调用进不了工具日志。仅当出现必须脱离网关运行工具的场景时重开。
- 展开全部延迟工具：等同 Claude Code 2.1.305 起的自动退回，网关无增益。
