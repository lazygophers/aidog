# Research: GLM 平台 429→400 错误链诊断（request_id 链 + 根因 + 历史量化）

- **Query**: 定位两条 request_id 的 proxy_log 完整记录 + 前驱→错误因果根因 + 历史同类模式量化
- **Scope**: internal（DB 取证 + 代码读 + spec 交叉）
- **Date**: 2026-07-02
- **只读诊断，未改任何码**

---

## 1. DB 路径（实测命中，非猜测）

- **实测命中**: `/Users/luoxin/.aidog/aidog.db`
- **构造逻辑**: `src-tauri/src/shared.rs:118-123` `aidog_data_dir()` = `home_dir().join(".aidog")`；`src-tauri/src/commands/proxy.rs:38` 与 `src-tauri/src/app_setup.rs:28` 均取 `aidog_data_dir()?.join("aidog.db")`
- **schema 确认**: `sqlite_master` 含 `proxy_log` 表；表结构与 `src-tauri/src/gateway/db/proxy_log.rs:6` `PROXY_LOG_COLUMNS` 一致（注意 `group_key` 非 `group_name`，Migration 010 已 rename）

---

## 2. 两条记录完整字段（实测，DB 原文）

| 字段 | 前驱 `3e8b13f0` (429) | 错误 `cb3603ac` (400) |
|---|---|---|
| status_code | **429** | **400** |
| upstream_status_code | 429 | 400 |
| is_stream | **0**（非流式） | **1**（流式 — 由响应 content-type `text/event-stream` 推断，`forward.rs:351-352`） |
| ts_local | 2026-07-02 15:04:12 | 2026-07-02 15:04:14（**+2s**） |
| duration_ms | 1039 | 1312 |
| group_key | `glm` | `glm` |
| model / actual_model | `claude-opus-4-8` → `glm-5.2` | 同左 |
| source / target_protocol | `anthropic` / `anthropic` | 同左 |
| platform_id | 38（GLM-自用） | 38 |
| request_url | `/proxy/v1/messages?beta=true` | 同左 |
| upstream_request_url | `https://open.bigmodel.cn/api/anthropic/v1/messages` | 同左 |
| retry_count / attempts 数 | **0 / 1**（单次，未 failover） | **0 / 1**（单次，未 failover） |
| est_cost / tokens | 0 / 0 / 0 / 0 | 0 / 0 / 0 / 0 |
| request_body len / md5 | 353988 / `18c1fcb406...` | **353988 / 同 md5（用户请求体字节级一致）** |
| upstream_request_body len / sha1 | **387331 / `a9d4a7b75a9a91a97166fef827e9c1d1e915e7a4`** | **387331 / 同 sha1（上游出站体字节级一致）** |

**429 response_body（GLM 真限流，错误码 1302）**:
```json
{"type":"error","error":{"type":"rate_limit_error","code":"1302","message":"[1302][您的账户已达到速率限制，请您控制请求频率][2026070215041393f47df126784504]"},"request_id":"..."}
```
429 上游响应头关键：`retry-after: 14`、`anthropic-ratelimit-unified-status: rejected`、`x-should-retry: true`。

**400 response_body（GLM 参数错，错误码 1210）**:
```json
{"type":"error","error":{"type":"invalid_request_error","code":"1210","message":"[1210][API 调用参数有误，请检查文档。][20260702150415ea18e2bfea554ca3]"},"request_id":"..."}
```
400 上游响应头：`content-type: text/event-stream;charset=UTF-8`（SSE 流式响应头，即使报错也带）。

**两条 session_id 相同**（用户请求体内）：`d59ffa44-b63c-4a84-a836-a8ad8ccf082e` → 同一 Claude Code 会话发的同一对话。

---

## 3. 历史同类模式量化（"反复多次"有数字）

时间窗：2026-07-01 21:20 → 2026-07-02 15:09（约 18h），group=`glm`、model=`claude-opus-4-8`、path=`/proxy/v1/messages?beta=true`：

| 指标 | 数量 |
|---|---|
| **1210 错误（参数错）总数** | **27** |
| **1302 错误（真限流）总数** | **12** |
| 1210 中"前 6s 内有 1302 前驱"（配对，JOIN 精确） | **11** |
| 1210 中"无 1302 前驱"（独立失败） | **16** |
| 主会话 `d59ffa44` 关联的 1210 | 10 |
| 期间成功 (status=200) 同 group/model/path | 数百条（仅列 15 条样本，body 长度 385K–509K 均有） |

**5 组典型 429→400 链**（均在 1–2s 内）：

| 429 ts | 400 ts | 间隔 |
|---|---|---|
| 15:03:02 | 15:03:03 | 1s |
| 15:03:55 | 15:03:56 | 1s |
| 15:04:12 | 15:04:14 | **2s（用户指定对）** |
| 15:06:35 | 15:06:37 | 2s |
| 15:09:12 | 15:09:13 | 1s |

**关键否定**：
1. **不是固定 id 重发** — 每对都是新 uuid（规律是"类型"，非"固定 id"）
2. **不是大小问题** — 失败 body 307K–404K，成功 body 最大 **509K** 也过；大小重叠，非阈值
3. **不是 proxy 内部 failover 重试** — `retry_count=0`、`attempts` 数组长度=1（两个 id 均单次）；429 是 retryable，但候选耗尽/单平台组无下一个候选 → 直接返客户端；400 是 hard-non-retryable（`non_success.rs:98` 决策 A）→ 也不重试
4. **2 个 proxy_log 行 = 客户端（Claude Code）发了 2 次独立 HTTP 请求**（429 后客户端自重试）

---

## 4. 根因（代码 + 日志双证据）

### 4.1 表面因果：客户端重试链
1. Claude Code 发请求 A（15:04:12）→ GLM 真限流 → **429 (1302)**，`retry-after: 14s`
2. Claude Code 不等满 14s，2s 后重试（同 payload，新 request_id）→ GLM 这次不限流但报**参数错 → 400 (1210)**

### 4.2 深层根因：proxy 的 `is_stream` 判定被"客户端漏发 stream 字段"误导，触发 hoist 规整后 GLM 仍拒

**代码证据链**：

- `src-tauri/src/gateway/proxy/handler.rs:282-283`:
  ```rust
  let is_stream = chat_req.stream.unwrap_or(false);
  log.is_stream = is_stream;
  ```
- `src-tauri/src/gateway/proxy/forward.rs:272`（host-gated 规整分支）:
  ```rust
  if matches!(target_protocol_enum, Protocol::Anthropic) && !is_official_anthropic_host(&url) {
      strip_thinking_if_unmatched(&mut req_body);  // thinking 开则无条件剔 context_management
      if !is_stream {                                // ← 关键门控
          hoist_mid_messages_system(&mut req_body); // messages 内 role=system 提到顶层 system
      }
  }
  ```
- `forward.rs:575-584` 代码注释（作者自述根因）:
  > 失败全集 = `{no_stream, has_assistant, messages 含 role=system}` —— 9/9 命中；
  > 同结构流式 PASS=1166，非流式 PASS=3。
  > 故仅非流式触发规整：流式同结构当前工作正常，不动避免回归。

**DB 取证（推翻代码注释的"流式必过"假设）**：

对比同会话、同结构、同 2 分钟内的 SUCCESS vs FAIL（用户原始请求体结构完全相同）：

| | SUCCESS `676bdbfc` (15:04:10, 200) | FAIL `cb3603ac` (15:04:14, 400) |
|---|---|---|
| 用户请求 `.stream` 字段 | **`true`** | **`<MISSING>`**（客户端漏发） |
| 用户请求 msg_count | 231 | 231 |
| 用户请求 role 分布 | system:40, assistant:95, user:96 | 同左 |
| `chat_req.stream.unwrap_or(false)` → `is_stream` | true → **不 hoist** | **false → 触发 hoist** |
| 上游出站体 role=system in messages | **40（原样保留）** | **0（被 hoist 提走）** |
| 上游出站体 top-level `system` 数组长度 | 2 | **42**（2 + 40 hoisted） |
| 上游出站体 `.stream` 字段 | true（透传） | **无** |
| 上游出站体 sha1 | `297e6381...` | `a9d4a7b7...`（不同） |

→ **FAIL 的上游体结构被 hoist 重排**：messages 内 40 条 role=system 合并到顶层 system 数组（2→42 块），messages 缩到 191 条，且 `.stream` 字段缺失。**hoist 本意是救 GLM 拒绝 role=system-in-messages 的场景，但对"客户端漏发 stream + 大上下文（40 system + 95 assistant turn）"的 body，规整后 GLM 仍报 1210。**

**为何 18/27 失败无 1302 前驱**：1210 是 GLM 端**非确定性拒绝**——同结构 body 有时 200 有时 400（作者注释里"非流式 PASS=3"也印证 GLM 间歇接受）。客户端漏发 `.stream` → 触发 hoist → 概率性被拒。

### 4.3 已排除（带证据）
- ❌ 上游体大小超限：成功 body 509K 过，失败 307K 也挂 → 非大小
- ❌ thinking/context_management 未剔：两体均已剔（`strip_thinking_if_unmatched` 运行，因 94 unmatched assistant + thinking.type=adaptive）
- ❌ proxy 内部重试致双行：`retry_count=0` / `attempts.len()=1` → 双行是客户端两次独立 HTTP
- ❌ auto_disable 误触：429 不触发 auto_disable（spec `platform-error-handling.md` C1），platform 38 `status=enabled`、`auto_disable_strikes=0`

### 4.4 spec 盲区
- `.trellis/spec/backend/platform-error-handling.md` 只覆盖 auto_disable / 429 分类 / purge，**未覆盖"客户端漏发 stream 字段 → is_stream 误判 → hoist 规整后仍失败"**
- `.trellis/spec/backend/proxy-connect-relay.md` 是 CONNECT 隧道，与本错无关

---

## 5. 修复方案候选（≥1，附代价/风险）

### 方案 A（最小改动，治标）【推荐先验证】
**门控改用"用户请求是否带 `.stream=true`"而非"`.stream.unwrap_or(false)`"**：若客户端漏发 stream，按"等效非流式"路径但**跳过 hoist**（hoist 是为 role=system-in-messages 救场，但本场景 hoist 后 GLM 仍拒，徒增结构变化）。
- 位置：`forward.rs:272` 把 `if !is_stream` 改为 `if !is_stream && body_has_role_system_in_messages(&req_body)`（仅当真有 role=system 在 messages 内才 hoist）
- 代价：低；需验证是否回归历史"9/9 no_stream+system 失败"场景（作者已验过 hoist 能救那 9 条，仍保留）
- 风险：若 GLM 在某些 case 必须靠 hoist 才过，跳过会回归；**需 DB 复核**

### 方案 B（治本，需 spec 沉淀）
**为 GLM/DeepSeek 等第三方 anthropic-compat 端点增加"客户端漏发 stream 时，强制透传原始 body"**：不跑任何规整（thinking strip / hoist 全跳），让客户端原生协议自纠（GLM 自己拒就让客户端拿到原始错，而非规整后变种的错）。
- 代价：中；需加配置开关 + 全样本回归
- 风险：第三方拒绝率可能上升，但错误更可预测

### 方案 C（客户端侧，超 proxy 范围）
**建议用户检查 Claude Code 客户端为何漏发 `.stream` 字段**（部分请求带、部分不带，疑似 count_tokens 探测或预热请求）。proxy 无法控客户端，但可在 last_error 里标注"建议客户端补 stream 字段"。
- 代价：0 代码；沟通成本

### 推荐执行序
1. **先跑 DB 复核**：统计"no_stream + hoist 后 1210"占比 vs "no_stream + hoist 后 200"占比，确认 hoist 是否真的无效
2. 若 hoist 对该结构无效 → 方案 A（加 body_has_role_system 守卫，避免无谓 hoist）
3. 沉淀 spec：`platform-error-handling.md` 加 C6 "客户端漏发 stream 字段的 is_stream 误判"

---

## 6. Caveats / Not Found

- 配对总数用 JOIN 精确算得 **11 配对 / 16 独立**（27 条 1210 中，11 条前 6s 内有 1302 前驱；16 条独立失败）
- `json_extract(metadata, ...)` 失败（该表无 `metadata` 列，是 body 内的 `metadata` 对象）→ 用 `upstream_request_body LIKE '%d59ffa44%'` 替代，主会话关联 10 条 1210
- **未实测** GLM 直接 curl 同 body 验证 1210 是否复现（需用户提供 GLM key 或在 grill 阶段确认是否要实跑）
- 方案 A/B 的具体回归影响需在 grill 阶段让用户确认是否接受可能的"hoist 跳过致历史 9 条救场案例回归"

## Files Found (证据文件:行)

| File | 关键证据 |
|---|---|
| `src-tauri/src/shared.rs:118-123` | `aidog_data_dir()` = `~/.aidog` |
| `src-tauri/src/commands/proxy.rs:38` | db 路径 `~/.aidog/aidog.db` |
| `src-tauri/src/gateway/db/proxy_log.rs:6` | `PROXY_LOG_COLUMNS`（32 列） |
| `src-tauri/src/gateway/proxy/handler.rs:282-283` | `is_stream = chat_req.stream.unwrap_or(false)` |
| `src-tauri/src/gateway/proxy/forward.rs:272` | `if !is_stream { hoist_mid_messages_system() }` 门控 |
| `src-tauri/src/gateway/proxy/forward.rs:351-352` | `is_stream` 被响应 content-type 覆盖 |
| `src-tauri/src/gateway/proxy/forward.rs:529-567` | `strip_thinking_if_unmatched` / `hoist_mid_messages_system` 实现 |
| `src-tauri/src/gateway/proxy/forward.rs:575-584` | 作者 DB 全样本注释（9/9 失败 = no_stream+system） |
| `src-tauri/src/gateway/proxy/non_success.rs:90-100` | 决策 A：400/422 硬错不重试 |
| `.trellis/spec/backend/platform-error-handling.md` | C1-C5（未覆盖本场景） |
| DB `/Users/luoxin/.aidog/aidog.db` | 27 条 1210 + 12 条 1302，platform 38 last_error 已存最新 1210 |
