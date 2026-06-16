---
name: aidog-request-inspect
description: 按 request id 从 ~/.aidog/aidog.db 的 proxy_log 表读取单条 Aidog 代理请求的完整链路（入站请求→协议转换→上游请求→上游响应→回客户端），含状态码、token、est_cost、重试链；headers 自动脱敏，body 可 pretty-print。也支持列最近 N 条 / 按 group / 状态码筛选。触发词：request id、请求 id、日志 id、查请求、这次请求、上游发了什么、proxy_log、最近请求、失败请求、400/500 排查。
when_to_use: 用户给出 request id/日志 id（hex32）查请求详情；排查某次代理请求（请求/响应 body、上游 URL、协议转换、状态码、token、重试、耗时）；列最近或失败请求时
argument-hint: <request_id> [--full|--raw] | --recent [N] [--group <g>] [--status <code>]
arguments: 透传给 inspect.py 的参数。首参为 request id（hex32）查单条；或 --recent [N] 列最近，配 --group/--status 筛选；--full 不截断 body，--raw 输出原始 JSON。
---

# aidog-request-inspect

按 request id 从 Aidog 本地数据库读取单条代理请求的完整链路（入站请求 → 协议转换 → 上游请求 → 上游响应 → 回客户端），用于调试某次请求。

## 何时用

- 用户给出一个 request id（32 位 hex，如 `fcd9eb36533f4f0ba4344f3cdc6cf530`）要看详情。
- 排查某次请求为什么失败 / 慢 / token 异常 / 路由到哪个平台 / 协议怎么转的。
- 列最近 N 条请求，或筛失败请求（按状态码 / group）。

## 数据源

- 库：`~/.aidog/aidog.db`（SQLite，运行时开启 WAL）。
- 表：`proxy_log`，主键 `id` 即 request id；一次请求一行；`deleted_at=0` 为有效行。
- 脚本以**只读**连接（`mode=ro`），不锁库、不干扰运行中的 aidog。

## 用法

脚本在本 skill 目录下：`.claude/skills/aidog-request-inspect/inspect.py`

```bash
# 查单条详情（摘要 + 各 body，body 默认截断 4000 字符，headers 脱敏）
python3 .claude/skills/aidog-request-inspect/inspect.py <request_id>

# 不截断 body（看完整请求/响应体）
python3 .claude/skills/aidog-request-inspect/inspect.py <request_id> --full

# 原始 JSON（机读，不格式化不脱敏 —— 谨慎，含明文密钥）
# 🔴 CHECKPOINT：--raw 输出含明文 Authorization / api-key / cookie。执行前必须确认：
#   仅本地排查、不外传、不贴入任何对外汇报或第三方。默认改用脱敏模式（去掉 --raw）。
python3 .claude/skills/aidog-request-inspect/inspect.py <request_id> --raw

# 列最近 N 条（默认 10）
python3 .claude/skills/aidog-request-inspect/inspect.py --recent 20

# 筛失败请求 / 按 group
python3 .claude/skills/aidog-request-inspect/inspect.py --recent 20 --status 400
python3 .claude/skills/aidog-request-inspect/inspect.py --recent 20 --group glm-coding-plan-auto

# 覆盖库路径（非默认位置）
python3 .claude/skills/aidog-request-inspect/inspect.py <request_id> --db /path/to/aidog.db
```

## 输出包含

单条详情：
- **摘要**：时间 / group / model→actual_model / source→target 协议 / platform_id / client+upstream 状态码 / 耗时 / stream / retry / tokens / est_cost / client URL / upstream URL
- **重试链**（`attempts`）：每次尝试的 platform / status / 耗时 / error
- **入站请求** headers（脱敏）+ body
- **上游请求** headers（脱敏）+ body（看协议转换后实际发上游的内容）
- **上游响应** headers + 响应 body

## 约定与注意

- **脱敏**：headers 里 `authorization` / `x-api-key` / `x-goog-api-key` / `api-key` / `cookie` 等自动打码（头尾各留 4 字符）。`--raw` 模式**不脱敏**，仅在确需原始密钥排查时用，输出含敏感信息。
- **body 截断**：默认 4000 字符，`--full` 看全部。流式响应 body 可能很大。
- **WAL**：默认只读连接能读到未 checkpoint 的最新行；若读不到刚发生的请求，确认 aidog 在跑 + id 正确。
- **找不到**：报错提示 id 不存在或已被 retention 清理（aidog 的 retention 会清空字段或删行，见项目 CLAUDE.md「Proxy 日志」）。

## 禁止清单（反模式）

- 🔴 禁把 `--raw` 输出（含明文密钥）贴入对外汇报、issue、PR、聊天、截图或任何第三方/外部渠道。
- 🔴 禁粘贴任何未脱敏的 `authorization` / `x-api-key` / `x-goog-api-key` / `api-key` / `cookie` 值。
- 默认用脱敏模式（不带 `--raw`）排查；仅在确需原始密钥比对时才临时本地使用 `--raw`，用后不留存。
- 禁用本脚本以外的方式直连/写库；脚本为只读（`mode=ro`），禁改写 `proxy_log`。
- 引用请求详情对外说明时，只引脱敏后的字段（状态码 / 协议 / token / 耗时 / 平台 / 错误信息），不引原始 header 值。

## 字段速查（proxy_log 关键列）

| 类 | 列 |
|---|---|
| 标识 | `id`(=request id), `group_name`, `model`, `actual_model`, `platform_id` |
| 协议 | `source_protocol`, `target_protocol` |
| 入站 | `request_headers`, `request_body`, `request_url` |
| 上游请求 | `upstream_request_headers`, `upstream_request_body`, `upstream_request_url` |
| 上游响应 | `upstream_response_headers`, `upstream_status_code`, `response_body` |
| 回客户端 | `user_response_headers`, `user_response_body`, `status_code` |
| 指标 | `duration_ms`, `input_tokens`, `output_tokens`, `cache_tokens`, `est_cost`, `is_stream` |
| 重试 | `attempts`(JSON 数组), `retry_count` |
| 时间 | `created_at`, `updated_at`（epoch） |
