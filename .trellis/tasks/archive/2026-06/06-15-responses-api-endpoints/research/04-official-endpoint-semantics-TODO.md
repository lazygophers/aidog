# Research: 官方 Responses API 端点语义（待 main 用 WebFetch 核实）

- **Query**: 4 端点（create/compact/cancel/retrieve）真实存在性 + 方法 + body 形态
- **Scope**: external（**未完成——本 research agent 无 WebFetch / web search 工具**）
- **Date**: 2026-06-15

## 状态：需要 main agent 补做

本 agent 工具集仅 Read/Write/Bash/Skill，**无 WebFetch / mcp__exa__web_search**，无法核官方 docs。以下为**推测**（标注），main agent 必须用 WebFetch 核实后再据以实现。

## 待核清单（WebFetch 目标）

建议 WebFetch：
- OpenAI Responses API docs（`https://platform.openai.com/docs/api-reference/responses`）
- Codex CLI 源码/docs（codex 对 `wire_api=responses` 实际调用哪些端点）

逐端点核实点：

| 端点 | 待确认 | 推测（未验证，禁直接采信） |
|---|---|---|
| `POST /v1/responses` create | ✓ 确定存在；body 用 `input` | 已知存在（codebase 已支持），无需核 |
| `GET /v1/responses/{id}` retrieve | 是否存在？是否带 query（如 `?stream=true`）？ | **推测**存在（OpenAI Responses 有 retrieve）；GET 无 body |
| `POST /v1/responses/{id}/cancel` | 是否存在？方法 POST？是否有 body？ | **推测**存在（OpenAI 有 cancel）；POST 无 body 或空 body |
| `POST /v1/responses/compact` 或 `/v1/responses/{id}/compact`? | **是否真实存在？是 OpenAI 标准还是 Codex 特有？** path 形态？带 body？ | **强不确定**——「compact」未在 OpenAI 标准 Responses API 印象中；可能是 Codex 自有概念或本任务描述的别名。**必须核** |

## 额外需 main 核（本仓侧，可用 Bash/proxy_log）

- Codex 在 aidog 下**实际发出的 responses 子端点 path 全集**：跑一次 codex 会话后查
  ```
  sqlite3 ~/.aidog/aidog.db "SELECT DISTINCT request_url FROM proxy_log WHERE request_url LIKE '%/responses%';"
  ```
  这是判定「4 端点哪些真被 Codex 调用」最可靠的事实源，优先于 docs 推测。

## 影响

- 若 compact 不存在 / 非标准 → 砍掉 compact，只做 retrieve + cancel。
- 若 retrieve/cancel 带 query → URL 构造需保留 query（build helper 用 path_and_query 而非仅 path）。
- 若 Codex 实际根本不发子端点 → 本任务降级为「防御性兼容」，优先级下调。
