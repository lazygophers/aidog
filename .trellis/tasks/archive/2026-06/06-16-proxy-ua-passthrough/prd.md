# PRD: 按 User-Agent 选择透传协议 + 多级回退

- **Task**: `06-16-proxy-ua-passthrough`
- **Date**: 2026-06-16
- **状态**: planning（PRD 定稿 — 4 决策已拍板）
- **交付判定**: 单交付（一个内聚改动点集中在 proxy.rs 出站决策段，可一次实现）

---

## 0. 已定决策（用户拍板，本节为权威结论）

> 以下 4 项已由用户拍板，覆盖原 §6 语义分叉与 §9「需要:」开放项。实现以本节为准。

- **已定 D1（核心语义）**：仅当 path 推断的入站协议在目标平台**无对应 endpoint**（即 `proxy.rs:1000-1009` 的 `matched_ep == None`，当前会落入 `proxy.rs:1010-1012` 的 `platform_type + ClientType::Default` 有损兜底）时，**才用 UA 推断透传协议**。即在进入"默认代理有损兜底"之前插入 UA 推断分支：命中则走 same-protocol passthrough 路径（复用 `proxy.rs:1023-1025, 1135-1142, 1414-1512` + `converter.rs:54 passthrough_api_path`）。**path 已被平台支持时不介入**，保持现状（级别 1 行为零变更）。
- **已定 D2（UA 来源）**：复用现有出站合成 UA 的子串特征规则（`proxy.rs:2882-2904` 的 `claude_code_ua` / `codex_ua`），应用到**入站** User-Agent 匹配：`claude-cli` → `anthropic`，`codex` → `openai_responses`。**无需用户抓包**，先按现有子串规则实现。
- **已定 D3（MVP 范围）**：仅 Claude Code（`claude-cli` 子串）+ Codex（`codex` 子串）两个客户端家族。其余 UA（含 Cursor / Windsurf / gemini-cli）**不识别 → 回退现有处理方案**（现有 default 兜底转换），不纳入 MVP。
- **已定 D4（可观测）**：复用 proxy_log **不改表结构**；UA 推断命中信息尽量填入现有可填字段 + 打 `tracing` 日志（`ua-passthrough` 标记）。**禁加列、禁迁移**。

> 原 §6「解读 A / 解读 B」之争 → **已定按 D1**：UA 推断仅在 `matched_ep == None`（平台无 path 协议端点）时介入，命中后将客户端身份协议作为透传 wire 协议，复用现有 passthrough 路径。等价于原研究倾向的「解读 B」精确化版本，且严格限定触发条件为 `matched_ep == None`。

---

## 1. 背景与目标

### 用户原始需求（拆解）

所有到 `/proxy/` 前缀的代理请求：

1. 目标平台**不支持**入站协议 → **默认透传**（不做有损协议转换）。
2. 透传目标协议**按 User-Agent 推断**（Claude Code UA → anthropic；Codex UA → openai_responses）。
3. **UA 不识别** 或 **目标平台不支持**推断出的协议 → **回退现有处理方案**（当前的 `convert_request` 转换 + 平台主协议兜底链）。

### 目标

在现有"同协议精确透传"（`same_protocol_passthrough`，见 [protocol-same-proto-passthrough]）之外，**新增一条 UA 驱动的透传路径**：当客户端入站协议不被目标平台显式支持时（`matched_ep == None`），不立即降级到有损转换，而是先按 UA 推断客户端"原生协议"，若该协议被平台某 endpoint 显式支持则走透传，最大限度保留客户端原始请求体形态。

### 非目标（MVP 边界见 §7）

- 不改 `/api/` 本地端点、健康端点 `GET /` `GET /proxy`、count_tokens / responses 子端点 / models 列表分流（均在 `parse_incoming_request` 之前前置分流，本功能不触碰）。
- 不引入新的 Protocol 枚举臂或 ClientType 臂（复用现有）。
- 不改 ClaudeCode 平台的纯字节透传（`handle_passthrough`，platform_type == ClaudeCode 的 1:1 relay，`proxy.rs:956-973`）——那是平台级订阅透传，与本功能正交。
- **不改 proxy_log 表结构**（已定 D4）。

---

## 2. 现有 /proxy 请求处理链（实证 file:line）

入口：Axum `fallback(handle_proxy)`（`proxy.rs:77`）→ `handle_proxy_inner`（`proxy.rs:692`）。`GET /` `GET /proxy` 被 `handle_root` 拦截（`proxy.rs:75-76, 151`），不进本链。

完整链路（`handle_proxy_inner`）：

| 步骤 | 行为 | file:line |
|---|---|---|
| 1. 捕获请求头 | Authorization REDACT 入库；`orig_headers` clone 原始量（含真实 UA） | `proxy.rs:742-773` |
| 2. 读 body | 10MB 上限 | `proxy.rs:775-787` |
| 3. 解析分组 | `resolve_group`：Bearer token == group_name 优先，回退 path 前缀 | `proxy.rs:800-820, 2822` |
| 4. **判定入站协议** | `detect_source_protocol(&path)` —— **纯 path 判定，不读 UA** | `proxy.rs:824-826, 2789` |
| 5. 前置子端点分流 | models 列表 / responses 子端点 / count_tokens（均在 parse 之前） | `proxy.rs:830-857` |
| 6. parse 入站 | `adapter::parse_incoming_request(source_protocol, req_value)` → `ChatRequest` | `proxy.rs:870, converter.rs:78` |
| 7. 入站中间件 | global/group 层规则 | `proxy.rs:886-896` |
| 8. 路由选候选 | `select_candidates_ctx` → 有序候选平台列表（含熔断/粘性/调度） | `proxy.rs:898-930` |
| 9. Mock / ClaudeCode 拦截 | platform_type 命中则本地生成 / 1:1 relay，不进重试 | `proxy.rs:932-974` |
| 10. **重试循环 + 端点匹配** | 逐候选 forward；**`matched_ep` 决定 target 协议与是否透传** | `proxy.rs:985-1146` |
| 11. 出站转换/透传 | `same_protocol_passthrough ? passthrough_api_path : convert_request` | `proxy.rs:1135-1146` |
| 12. 响应处理 | `passthrough_response` 原样 relay SSE / 否则 parse_sse→to_client_sse | `proxy.rs:1414-1512` |

### 入站协议判定（`detect_source_protocol`，`proxy.rs:2789-2817`）

纯 path-based，剥掉组前缀后按尾段映射：

- `/v1/messages*` → `anthropic`
- `/v1/responses*` → `openai_responses`
- `/v1/chat/completions` `/v1/completions` `/v1/embeddings` `/v1/images` `/v1/audio` `/v1/models` → `openai`
- `/v1beta/*` → `gemini`
- 其它兜底 → `anthropic`

**关键事实：全链路无任何地方读 User-Agent 做协议/路由决策**（grep `user.agent` 全仓确认）。UA 当前仅两处使用：
1. **出站合成**（`apply_client_headers` 系列，`proxy.rs:2854+`）—— 按 endpoint 的 `client_type` 伪造上游 UA。
2. **sticky 会话键回退**（`proxy.rs:904-911`）—— x-session-id / session_id 缺省时用 UA 当客户端标识（`orig_headers.get("user-agent")`，`proxy.rs:907`）。

---

## 3. "平台不支持入站协议" 的精确判定（实证）

判定发生在重试循环内，每候选独立判定（`proxy.rs:996-1025`，已逐行核对）：

```
ep_proto(ep) = format!("{:?}", ep.protocol).to_lowercase()   // proxy.rs:999

matched_ep = platform.endpoints.find(ep_proto == source_protocol)   // 精确匹配  proxy.rs:1000-1002
    .or_else(|| if source_protocol == "openai_responses"
                  { endpoints.find(ep_proto == "openai") }          // 唯一跨协议回退  proxy.rs:1003-1009
                else { None })

// matched_ep == None → 取 platform_type + ClientType::Default 有损兜底       proxy.rs:1010-1012
same_protocol_passthrough = matched_ep.map(ep_proto == source_protocol).unwrap_or(false)  // proxy.rs:1023-1025
```

**"平台不支持入站协议" == `matched_ep == None`**（无任何端点协议等于 source_protocol，且非 openai_responses→openai 回退）：

- `matched_ep == None` → 当前直接取 `route.platform.platform_type` 当 target 协议 + `ClientType::Default`（`proxy.rs:1010-1012`），`same_protocol_passthrough = false` → **走 `convert_request` 有损转换**。**这就是本功能要插入 UA 透传的节点（已定 D1）。**
- `matched_ep == Some` 但 `ep_proto != source_protocol`（仅 openai_responses→openai 回退命中）→ target = openai，`same_protocol_passthrough = false` → 走 convert_request（跨协议真转换，回归安全）。**本功能不介入此分支**（已有匹配端点，不是 `matched_ep == None`）。

> 即：现状只有"入站协议被平台**显式**声明为某 endpoint 协议"才透传；否则一律有损转换或跨协议转换。本功能仅在 `matched_ep == None` 分支前插入 UA 推断尝试。

---

## 4. UA → 协议映射表

### 4.1 UA 特征串（来源：本仓**出站合成** UA，`proxy.rs:2882-2904`，已核对）

> 这些是 aidog **伪造给上游**的 UA。已定 D2：直接复用其子串特征规则作为**入站** UA 匹配的起点，无需抓包真实入站样例。`claude-cli` / `codex` 前缀稳定。

| 客户端家族 | 出站合成 UA（特征子串） | 推断 wire 协议 | 备注 |
|---|---|---|---|
| Claude Code CLI/VSCode/SDK/GhAction | `claude-cli/...`（`claude_code_ua`，`proxy.rs:2884-2893`） | **anthropic** | 全部含 `claude-cli` 前缀 |
| Codex CLI (rust) | `codex_cli_rs/0.38.0 ...` | **openai_responses** | 含 `codex` |
| Codex TUI | `Codex/0.38.0` | openai_responses | 含 `codex`（不区分大小写） |
| Codex Desktop | `codex desktop/0.38.0` | openai_responses | 含 `codex` |
| Codex VSCode | `codex-vscode/0.38.0` | openai_responses | 含 `codex` |
| Cursor / Windsurf / gemini-cli / 其它 | `Cursor/...` `Windsurf/...` 等 | **不识别** | 已定 D3：回退现有处理 |

**MVP 特征子串匹配规则（不区分大小写）：**

- UA 含 `claude-cli` → `anthropic`
- UA 含 `codex`（覆盖 `codex_cli_rs` / `Codex/` / `codex desktop` / `codex-vscode`）→ `openai_responses`
- 其它（含 Cursor / Windsurf / gemini-cli / 未知 / 缺失）→ 不推断（返回 `None`，进回退链级别 3）

### 4.2 协议合法性约束

推断出的协议必须是合法 wire 协议（`models.rs` Protocol 枚举），且必须能被平台某 endpoint 显式支持（`ep_proto == ua_protocol`）才走透传，否则进回退链（§5 级别 3）。

---

## 5. 三级回退链（核心设计 — 已定 D1）

在 `matched_ep` 解析后（`proxy.rs:1009` 末）、计算 `same_protocol_passthrough`（`proxy.rs:1023`）之前插入。决策顺序：

```
【级别 0】平台支持 path 协议 → 走现有逻辑（不介入）
  matched_ep 精确命中 source_protocol（proxy.rs:1000-1002）
  → same_protocol_passthrough = true，走 passthrough_api_path（proxy.rs:1135-1142）
  → 或 openai_responses→openai 回退命中（proxy.rs:1003-1009）→ 跨协议 convert（回归安全）
  本功能不介入，行为零变更。

【级别 1（本功能新增）】平台不支持 path 协议 + UA 命中 + 平台有该协议 endpoint → same-protocol 透传
  仅当 matched_ep == None 时尝试：
    a. ua_protocol = infer_protocol_from_ua(orig_headers["user-agent"])  // claude-cli→anthropic / codex→openai_responses
    b. 若 ua_protocol == Some(p) 且 平台存在 endpoint 精确支持 p（ep_proto == p）：
       → matched_ep 改指向该 UA-endpoint
       → target_protocol_enum / target_base_url / client_type / coding_plan 取自该 endpoint（同 proxy.rs:1010-1011 路径）
       → 令 same_protocol_passthrough = true
       → 复用现有 passthrough 出站/响应路径（proxy.rs:1135-1142, 1414-1512 + converter.rs:54）
       → tracing 标记 "ua-passthrough"（已定 D4）

【级别 2】平台不支持 path 协议 + UA 命中 但平台无该协议 endpoint → 回退现有 default 兜底
  ua_protocol == Some(p) 但平台无 ep_proto == p 的端点
  → 保持 matched_ep == None → platform_type + Default → convert_request 有损转换（现状，零变更）

【级别 3】平台不支持 path 协议 + UA 不识别 → 回退现有 default 兜底
  ua_protocol == None（含 Cursor/Windsurf/gemini-cli/未知/UA 缺失）
  → 保持 matched_ep == None → platform_type + Default → convert_request 有损转换（现状，零变更）
```

> 简记：**平台支持 path 协议 → 不介入（级别 0）；平台不支持 → ① UA 命中且平台有该协议 endpoint → 透传（级别 1）；② UA 命中但平台无该协议 endpoint → 回退（级别 2）；③ UA 不识别 → 回退（级别 3）。**

### 衔接点（精确 file:line，已核对）

- **插入位置**：`proxy.rs:1009`（`matched_ep` 的 `.or_else()` 链末尾）与 `proxy.rs:1023`（`same_protocol_passthrough` 计算）之间。需把 `let matched_ep` 由不可变改为可被 UA 分支重绑定（或新增中间变量保存 UA 命中的端点引用）。
- **UA 读取**：`orig_headers.get("user-agent")`（已 clone，见 `proxy.rs:773, 907` 同款读法），注意 `orig_headers` 须在重试循环内可见（确认作用域）。
- **复用**：透传成功后完全复用现有 `same_protocol_passthrough = true` 路径（`passthrough_api_path` + `passthrough_response` SSE relay，`proxy.rs:1135-1142, 1414-1512`），无需新出站/响应代码。继承 5 项旁路改写（model remap / 鉴权 / URL / coding_plan / usage 提取）。
- **回退**：级别 2/3 不命中则保持现有 `matched_ep == None` 兜底与 `same_protocol_passthrough` 计算原样，零行为变更。

---

## 6. 语义结论（原分叉已由 D1 拍板）

> 原 §6 列出「解读 A / 解读 B」歧义。**已定按 D1**：

- UA 推断**仅在 `matched_ep == None`**（平台对 path 协议无任何端点）时触发，避免与级别 0 冲突。
- 命中后将 UA 协议作为透传 wire 协议，**要求平台确有该协议 endpoint**（否则级别 2 回退），不做"赌上游兼容"的强行透传。
- 本质：用 UA 身份在"平台缺 path 协议端点"时，找平台另一个客户端原生协议端点做透传，比 platform_type 有损兜底更保真。

---

## 7. MVP 边界（已定 D3）

**纳入：**
- 仅 `handle_proxy_inner` 主链（POST chat 类请求），不碰前置分流的子端点。
- 仅 Claude Code（`claude-cli`→anthropic）与 Codex（`codex`→openai_responses）两类 UA 子串匹配。
- UA 推断协议必须被平台 endpoint 显式支持才透传，否则回退（级别 2）。
- 复用现有 passthrough 出站/响应路径，不新增 SSE 处理。
- 复用 proxy_log 现有字段 + tracing 日志（已定 D4，不改表结构）。

**暂不纳入（后续迭代）：**
- Cursor / Windsurf / gemini-cli 等其它客户端映射。
- "强行透传赌上游兼容"模式。
- proxy_log 新增专用列（已定 D4 明确禁止）。

---

## 8. 改动点（file:line 已核对）

| 文件 | 改动 | 说明 |
|---|---|---|
| `proxy.rs` 新增 fn | `infer_protocol_from_ua(ua: &str) -> Option<&'static str>` | UA 子串匹配（claude-cli→"anthropic"，codex→"openai_responses"，其它 None）；放 `detect_source_protocol` 附近（`proxy.rs:~2817`） |
| `proxy.rs:1009-1025` | 在 `matched_ep` 解析后插入 UA 透传尝试分支 | 仅 `matched_ep == None` 时触发；命中则把 matched_ep 重绑定到 UA-endpoint，并令 `same_protocol_passthrough = true`；同步 `target_protocol_enum / base_url / client_type / coding_plan` 取自该端点（同 `proxy.rs:1010-1011`） |
| `proxy.rs:1014` 附近 | `target_protocol` / `client_type` 等派生量需在 UA 分支命中后重算 | 确保 `apply_client_headers`、`passthrough_api_path` 用到的是 UA-endpoint 的值 |
| `proxy.rs` 重试循环内 | 读 `orig_headers.get("user-agent")` | 确认 `orig_headers` 在循环内可见；不可见则在循环前提取 `ua: Option<String>` |
| tracing | UA 命中打 `tracing::info!(... "ua-passthrough")` + 填入现有 log 可填字段 | 已定 D4，不加列 |
| 单测 | `infer_protocol_from_ua` 各 UA 子串 + 未知/缺失回退；三级回退分支 | proxy.rs `#[cfg(test)]` 已有测试惯例 |

> 实现需服从 [url-construction-rule]：透传 path 由 `passthrough_api_path`（`converter.rs:54`）产出，禁额外拼前缀。

---

## 9. 与 protocol-same-proto-passthrough 的关系

本功能是 [protocol-same-proto-passthrough] 的**扩展**，非替代：

- 级别 0（现状）：`same_protocol_passthrough = matched_ep.protocol == source_protocol`（path 判定的 source）。
- 级别 1（本功能）：当 path-source 无匹配端点（`matched_ep == None`）时，用 UA 推断候选协议，若平台有该协议端点则同样触发透传（复用同一 `same_protocol_passthrough=true` 出站/响应路径 + 5 项旁路改写：model remap / 鉴权 / URL / coding_plan / usage 提取，全部继承）。
- 级别 2/3（回退）：保持现状的 `convert_request` 有损转换 + openai_responses→openai 跨协议回退，零变更。

---

## 10. 验收标准

1. Claude Code UA（`claude-cli` 子串）入站、path 判定协议平台**无端点**但平台**有 anthropic endpoint** 时 → 走透传（tracing 可见 `ua-passthrough`，出站 body == 客户端原始 body 仅 patch model）。
2. Codex UA（`codex` 子串）→ openai_responses 透传同理（平台有 openai_responses endpoint 时）。
3. UA 命中但平台**无**推断协议 endpoint（级别 2）→ 回退现有 convert_request，行为与改动前完全一致。
4. UA 不识别（curl / Postman / Cursor / Windsurf / 无 UA，级别 3）→ 回退现有 convert_request，行为零变更。
5. 级别 0（精确同协议透传 + openai_responses→openai 回退）行为零变更。
6. 子端点分流（count_tokens / responses / models / 健康端点）不受影响。
7. proxy_log 表结构未变更（无新列、无迁移）。
8. `cargo clippy` 无 warning、`cargo test` 通过（含新增 `infer_protocol_from_ua` 单测 + 三级回退分支测试）。

---

## 附：决策溯源

- 4 决策由用户于 2026-06-16 拍板，见 §0。
- 原 §6「解读 A/B」歧义与原 §9「需要:」（Cursor/Windsurf 协议、抓包样例、是否纳入 gemini-cli、proxy_log 新列）全部已由 D1–D4 闭合，MVP 不再有开放项。
