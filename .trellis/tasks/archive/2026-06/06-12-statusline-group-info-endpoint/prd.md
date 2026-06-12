# StatusLine 分组信息端点 + 段

## Goal

StatusLine 脚本能按当前分组请求 aidog 自身 HTTP 接口，拿该分组（仅单平台分组）的本地预估信息（余额/已花费/coding plan/请求·成功率/缓存率/总 tokens）用于展示。每个 group 的 `settings.{group}.json` 注入独立 env（API 地址 + 平台 api_key 作密钥 + 分组名），statusline 段据此 curl 各自信息。

## Decisions（brainstorm 已定）

| 维度 | 决策 |
|------|------|
| 触发范围 | **仅单平台分组**显示；多平台分组不注入 / 段无数据 |
| 鉴权 | **复用平台 api_key**：env 注入该分组单平台的 api_key；端点比对该分组平台 api_key 一致才返回 |
| 数据源 | **aidog 本地预估值**：est_balance_remaining / est_coding_plan / est_cost 聚合 + usage 统计，不去上游真查 |
| 新增段 | 余额 / 已花费 / Coding Plan 利用率 / 请求·成功率 / 缓存率 / 已使用总 tokens |

## What I already know（现状）

- proxy 是 Axum 服务器 `127.0.0.1:{port}`（proxy.rs:36 Router，自动选端口）。已有 `/proxy` 路由。
- `do_sync_group_settings(db, port)`（lib.rs:761）生成 `settings.{group}.json` 到 `~/.aidog/`，已注入 `env`：`ANTHROPIC_BASE_URL=http://127.0.0.1:{port}/proxy` + 分组名 env。
- statusline 段系统在 editors.tsx（SEGMENT_DEFS / toBash 生成 bash / generateStatusLineScript）；段 toBash 用 `jq` 解析 `$input`（Claude Code 传入 JSON）。
- 平台本地预估字段：est_balance_remaining(f64) / est_coding_plan(JSON tiers) / 平台 usage stats（platformApi.usageStats / est_cost 聚合）。

## Requirements

### 后端端点（R1）
- R1.1 proxy Axum 加路由（如 `GET /__aidog/group-info`），入参：分组名 + api_key（query 或 header）。
- R1.2 查该分组关联平台：**仅当恰好 1 个平台**时返回该平台信息；否则返回标识「不适用/多平台」（空或 204/明确字段）。
- R1.3 鉴权：传入 key 必须等于该分组单平台的 api_key，否则 401。
- R1.4 返回 JSON（本地预估值）：`{ balance, spent(累计/今日 cost), coding_plan: tiers[], requests, success_rate, cache_rate, total_tokens, currency }`。数据取 est_balance_remaining + est_coding_plan + 该平台 usage 统计（复用现有 db 查询）。
- R1.5 端点只读、低开销，不触发上游真查。

### 分组 settings 注入（R2）
- R2.1 `do_sync_group_settings` 对**单平台分组**额外注入 env：`AIDOG_INFO_URL=http://127.0.0.1:{port}/__aidog/group-info`、`AIDOG_GROUP=<name>`、`AIDOG_KEY=<该平台 api_key>`。多平台分组不注入这些。
- R2.2 不破坏既有 env 注入（ANTHROPIC_BASE_URL 等）。

### StatusLine 段（R3）
- R3.1 新增段类型：`group-balance` / `group-spent` / `group-coding` / `group-requests` / `group-cache` / `group-tokens`。
- R3.2 各段 toBash：用 env（`$AIDOG_INFO_URL` 等）curl 端点（带 key），`jq` 提取对应字段渲染；env 缺失（非分组 settings / 主 settings）→ 优雅降级（空/占位，不报错）。建议端点结果缓存到临时文件避免一行多段多次 curl（同一次渲染只查一次）。
- R3.3 段可参与现有颜色/对齐/分隔符/多行体系；preview 用 mock 值。
- R3.4 文案走 i18n t()，补 7 语言。

## Acceptance Criteria

- [ ] 单平台分组的 settings.{group}.json 注入 AIDOG_INFO_URL/GROUP/KEY；多平台分组不注入。
- [ ] 端点对正确 key 返回该分组单平台本地预估信息；错误 key 401；多平台/无平台分组返回不适用。
- [ ] 新增 6 段可在 statusline 配置、生成 bash curl 端点 + jq 提取，env 缺失优雅降级。
- [ ] 同一行多 group 段只 curl 一次（缓存）。
- [ ] 段兼容颜色/对齐/分隔符/多行；preview 正常。
- [ ] cargo check 通过；tsc 0；新文案 t() 补 7 语言。

## Definition of Done

- 端点只读本地预估、不上游真查、鉴权正确；不破坏既有 proxy 路由 / env 注入。
- 段 env 缺失不报错；脚本 bash 语法正确（curl/jq 转义）。
- 跨层一致（env 名 / 端点路径 / 字段名 前后端统一）。

## Out of Scope

- 不去上游真查余额（仅本地预估）。
- 多平台分组聚合展示（本期仅单平台）。
- 不改既有 proxy 转发 / 手动预算 / 真查校准逻辑。

## Technical Notes

- 端点落 proxy.rs Router（与 /proxy 同 server/port）；查询复用 db.rs（group→platforms、est 字段、usage 统计）。
- env 注入在 lib.rs do_sync_group_settings（单平台分支）。
- 段在 editors.tsx SEGMENT_DEFS + toBash；缓存可用 `${AIDOG_INFO_CACHE:-/tmp/aidog_info_$$}` 思路（一次渲染 curl 一次，后续段读缓存）。
- 复用记忆 [[group-stats-aggregation]] [[quota-service]] [[pricing-resolve-single-source]]。
