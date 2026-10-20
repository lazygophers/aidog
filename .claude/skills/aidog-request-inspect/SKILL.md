---
name: aidog-request-inspect
description: 按 request id 经 aidog-kernel 管理面 (/rpc/proxy_log_get) 读取单条 Aidog 代理请求的完整链路（入站请求→协议转换→上游请求→上游响应→回客户端），含状态码、token、est_cost、重试链；headers 自动脱敏，body 可 pretty-print。也支持列最近 N 条 / 按 group / 状态码筛选。触发词：request id、请求 id、日志 id、查请求、这次请求、上游发了什么、proxy_log、最近请求、失败请求、400/500 排查。
when_to_use: 用户给出 request id/日志 id（hex32）查请求详情；排查某次代理请求（请求/响应 body、上游 URL、协议转换、状态码、token、重试、耗时）；列最近或失败请求时
argument-hint: <request_id> [--full|--raw] | --recent [N] [--group <g>] [--status <code>] [--port <p>] [--token <t>]
arguments: 透传给 req_inspect.py 的参数。首参为 request id（hex32）查单条；或 --recent [N] 列最近，配 --group/--status 筛选；--full 不截断 body，--raw 输出原始 JSON；--port/--token 指定内核管理面端口与凭据（默认 127.0.0.1:9891、无凭据）。
---

# aidog-request-inspect

按 request id 读取单条 Aidog 代理请求的完整链路（入站请求 → 协议转换 → 上游请求 → 上游响应 → 回客户端），用于调试某次请求。

## 🔴 旧直查手段已失效（2026-10-20）

SQLCipher 全库加密落地（spec db-sqlcipher-full 票 01）后，`~/.aidog/log.db` / `aidog.db` / `platform.db` 整库密文，标准 `sqlite3` CLI 直查报 `file is not a database`。**任何「sqlite3 直查 log.db / SELECT proxy_log」的做法都已失效**，包括本 skill 2026-10-20 之前的版本与任何第三方 SQLite 工具。替代链路见下（经内核命令）。

## 何时用

- 用户给出一个 request id（32 位 hex，如 `fcd9eb36533f4f0ba4344f3cdc6cf530`）要看详情。
- 排查某次请求为什么失败 / 慢 / token 异常 / 路由到哪个平台 / 协议怎么转的。
- 列最近 N 条请求，或筛失败请求（按状态码 / group）。

## 数据源

- 内核命令（不碰 DB 文件）：`POST /rpc/proxy_log_get`（单条完整行）与 `POST /rpc/proxy_log_list_filtered`（列表+筛选），由 `aidog-kernel --ui` 管理面提供（`crates/aidog_kernel/src/rpc.rs`，与前端 invoke 同一批命令的 HTTP 形态）。内核用自己的 keychain 主密钥解密读库。
- **前置：需要 `aidog-kernel --ui` 在跑**（默认端口 9891，只绑 127.0.0.1）。桌面 app 不带 /rpc 管理面——排查时先起一个内核：
  ```bash
  aidog-kernel --ui --port 9891          # 与桌面 app 共用 ~/.aidog 与 keychain 密钥，WAL 并发读安全
  ```
  Linux headless / keychain 不可用环境：内核启动前 `export AIDOG_DB_MASTER_KEY_FILE=<key 文件>`（与既有 DB 同一把钥）。
- 管理面配了凭据（KernelSettings auth_token）时脚本需 `--token <t>`（Bearer）。
- 信息量与旧直查等价且为其超集：`proxy_log_get` 返回整行（含 headers/body/tokens/est_cost/attempts，另多 `blocked_by` / `blocked_reason` / `done` / `field_trace` / `body_omitted`）。

## 用法

脚本在本 skill 目录下：`.claude/skills/aidog-request-inspect/req_inspect.py`

```bash
# 查单条详情（摘要 + 各 body，body 默认截断 4000 字符，headers 脱敏）
python3 .claude/skills/aidog-request-inspect/req_inspect.py <request_id>

# 不截断 body（看完整请求/响应体）
python3 .claude/skills/aidog-request-inspect/req_inspect.py <request_id> --full

# 原始 JSON（机读，不格式化不脱敏 —— 谨慎，含明文密钥）
# 🔴 CHECKPOINT：--raw 输出含明文 Authorization / api-key / cookie。执行前必须确认：
#   仅本地排查、不外传、不贴入任何对外汇报或第三方。默认改用脱敏模式（去掉 --raw）。
python3 .claude/skills/aidog-request-inspect/req_inspect.py <request_id> --raw

# 列最近 N 条（默认 10）
python3 .claude/skills/aidog-request-inspect/req_inspect.py --recent 20

# 筛失败请求 / 按 group（--status: 200=仅成功, -1=仅失败, 其它=精确状态码）
python3 .claude/skills/aidog-request-inspect/req_inspect.py --recent 20 --status 400
python3 .claude/skills/aidog-request-inspect/req_inspect.py --recent 20 --group glm-coding-plan-auto

# 管理面不在默认端口 / 带凭据
python3 .claude/skills/aidog-request-inspect/req_inspect.py --recent 20 --port 9891 --token <t>
```

## 输出包含

单条详情：
- **摘要**：时间 / group / model→actual_model / source→target 协议 / platform_id / client+upstream 状态码 / 耗时 / stream / retry / tokens / est_cost / client URL / upstream URL
- **重试链**（`attempts`）：每次尝试的 platform / status / 耗时 / error
- **入站请求** headers（脱敏）+ body
- **上游请求** headers（脱敏）+ body（看协议转换后实际发上游的内容）
- **上游响应** headers + 响应 body

## 约定与注意

- **脱敏**：headers 里 `authorization` / `x-api-key` / `x-goog-api-key` / `api-key` / `cookie` 等自动打码（头尾各留 4 字符）。`--raw` 模式**不脱敏**，仅在确需原始密钥排查时用，输出含敏感信息。注意 DB 入库的鉴权头本身已是 `[REDACTED]`（ProxyLogSettings 开关控制，见项目 CLAUDE.md「Proxy 日志」）——脚本再脱敏一层是兜底。
- **body 截断**：默认 4000 字符，`--full` 看全部。流式响应 body 可能很大。
- **找不到**：报错提示 id 不存在或已被 retention 清理（aidog 的 retention 会清空字段或删行）。若读不到刚发生的请求，确认内核在跑（`curl http://127.0.0.1:<port>/healthz`）+ id 正确 + 读写的是同一个 HOME。
- **连不上管理面**：脚本会提示先起 `aidog-kernel --ui`；纯内核形态（无 `--ui`）没有 /rpc。
- **记录开关**：headers/body 入库受 ProxyLogSettings 的 log_user_request / log_upstream_request 控制，关掉后只能读到元数据——这不是链路坏了。

## 禁止清单（反模式）

- 🔴 禁把 `--raw` 输出（含明文密钥）贴入对外汇报、issue、PR、聊天、截图或任何第三方/外部渠道。
- 🔴 粘贴任何未脱敏的 `authorization` / `x-api-key` / `x-goog-api-key` / `api-key` / `cookie` 值。
- 默认用脱敏模式（不带 `--raw`）排查；仅在确需原始密钥比对时才临时本地使用 `--raw`，用后不留存。
- 🔴 禁再用 sqlite3 / 任何标准 SQLite 工具直查 `~/.aidog/*.db`（全库已加密，只会得到 `file is not a database`）；也禁为绕过加密自行解密 DB 文件——读日志一律走内核命令链路。
- 🔴 禁经 `/rpc` 调写类命令改库（proxy_log_clear / settings_set 等）；本 skill 只读（proxy_log_get / proxy_log_list_filtered）。
- 引用请求详情对外说明时，只引脱敏后的字段（状态码 / 协议 / token / 耗时 / 平台 / 错误信息），不引原始 header 值。

## 字段速查（proxy_log 关键列，经 /rpc 返回的 JSON 键同名）

| 类 | 列 |
|---|---|
| 标识 | `id`(=request id), `group_key`, `model`, `actual_model`, `platform_id` |
| 协议 | `source_protocol`, `target_protocol` |
| 入站 | `request_headers`, `request_body`, `request_url` |
| 上游请求 | `upstream_request_headers`, `upstream_request_body`, `upstream_request_url` |
| 上游响应 | `upstream_response_headers`, `upstream_status_code`, `response_body` |
| 回客户端 | `user_response_headers`, `user_response_body`, `status_code` |
| 指标 | `duration_ms`, `input_tokens`, `output_tokens`, `cache_tokens`, `est_cost`, `is_stream` |
| 重试 | `attempts`(对象数组), `retry_count` |
| 拦截/终态 | `blocked_by`, `blocked_reason`, `done`, `body_omitted` |
| 时间 | `created_at`, `updated_at`（epoch） |
