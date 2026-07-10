# Research: client-types.json 特征 header 补全（proxy_log 真实入站样本）

- **Query**: 探 `~/.aidog/aidog.db` proxy_log 真实入站 header，提取每 client_type UA 的特征 header 清单，补全 `src-tauri/defaults/client-types.json` 的 `simulation.auth[protocol].headers`
- **Scope**: internal（DB 取样 + 源码核对）
- **Date**: 2026-07-10

## 数据源 & 复现 SQL

**DB**：`~/.aidog/aidog.db`（12 G，24578 行 `proxy_log`，全部 `deleted_at=0`）

**复现 SQL**（一次性 dump 24k 行 request_headers JSON 到本地文件后离线 Python 解析；12G 库不建议反复 LIKE 全表扫）：

```sql
.mode list
.output /tmp/all_headers.txt
SELECT request_headers FROM proxy_log WHERE deleted_at=0;
```

**Python 解析骨架**（按 UA family 分桶 → 提特征 header）：

```python
import json
from collections import defaultdict
STRIPPED = {'host','content-length','connection','keep-alive','accept-encoding',
            'authorization','user-agent','content-type','x-api-key','x-goog-api-key','accept'}
fam = defaultdict(lambda: defaultdict(list))
for line in open('/tmp/all_headers.txt'):
    d = json.loads(line)
    ua = d.get('user-agent')
    f = classify(ua)             # 见下「family 判定」
    for k,v in d.items():
        if k.lower() in STRIPPED: continue
        if v not in fam[f][k]: fam[f][k].append(v)
```

**schema 字段**：`request_headers` TEXT = 入站原始 header JSON（小写 key，Authorization 已 `[REDACTED]` 入库）。`upstream_request_headers` TEXT = 透传 + `apply_client_headers` 覆盖后的镜像（日志用）。

---

## 透传 vs 注入 语义（源码核对）

引用 `src-tauri/crates/aidog_core/src/gateway/proxy/headers.rs`：

- **L47-64 `STRIPPED_ON_CONVERT_PASSTHROUGH`**（透传底座剔除）：`host` / `content-length` / 标准 hop-by-hop（`connection`/`keep-alive`/`proxy-authenticate`/`proxy-authorization`/`te`/`trailer`/`trailers`/`transfer-encoding`/`upgrade`）/ `authorization` / `x-api-key` / `x-goog-api-key` / `user-agent` / `content-type`
- **L94-96 `strip_anthropic_beta_for_third_party`**：上游 host ≠ `api.anthropic.com` 时剔 `anthropic-beta`（第三方 Anthropic 兼容端点如 GLM 不认新 beta token，原样透传触发 400 code 1210）
- **L283-314 `apply_client_headers`**：仅覆盖 `User-Agent` + `simulation.auth[protocol].headers` 列表（含 placeholder 填充），**不注入其他任何 header**
- **结论**：入站客户端发的 `x-stainless-*` / `anthropic-version` / `anthropic-dangerous-direct-browser-access` / `x-app` / `x-claude-code-session-id` **全部原样透传到上游**（除 `anthropic-beta` 在非官方端点被剔）。simulation 注入的特征 header 仅在「入站缺这些 header」时（如 model_test、curl 直接打代理、或客户端不发指纹）生效，覆盖到上游。
- `simulation.auth[*].headers` 中的 entry 经 `fill_placeholder`（`{api_key}`/`{uuid}`）后 `.header(name, value)` 单值覆盖入站同名。`api-key`/`Authorization`/`x-api-key`/`x-goog-api-key` 已在各 client_type 现有 auth 列表内（凭证），**特征 header 添加不冲突**。

---

## UA 分布（全表 24578 行）

| UA | 行数 | family |
|---|---:|---|
| `claude-cli/2.1.204 (external, cli)` | 9410 | claude_code (cli) |
| `claude-cli/2.1.199 (external, cli)` | 6459 | claude_code (cli) |
| `claude-cli/2.1.201 (external, cli)` | 4857 | claude_code (cli) |
| `claude-cli/2.1.202 (external, cli)` | 2317 | claude_code (cli) |
| `<none>`（含 `{"source":"quota"}` 健康探测） | 758 | default |
| `Python-urllib/3.13`（statusline 配额探测） | 538 | default |
| `claude-cli/2.1.201 (external, sdk-cli)` | 115 | **claude_code_sdk_ts** ← 见下数据缺口 |
| `claude-cli/2.1.204 (external, sdk-py, agent-sdk/0.2.113)` | 48 | claude_code_sdk_py |
| `claude-cli/2.1.202 (external, sdk-py, agent-sdk/0.2.111)` | 22 | claude_code_sdk_py |
| `claude-cli/2.1.202 (external, sdk-cli)` | 13 | **claude_code_sdk_ts** ← 见下数据缺口 |
| `claude-cli/2.1.202 (external, sdk-py, agent-sdk/0.2.113)` | 11 | claude_code_sdk_py |
| `curl/8.7.1` | 5 | default |
| `axios/1.15.2` | 3 | default |

**数据缺口**（DB 无样本，需用户采集或参照同家族推断）：

| client_type | 期望 UA 后缀 | DB 实测 | 处置 |
|---|---|---|---|
| `claude_code_vscode` | `claude-vscode` | 无样本 | 参照 `claude_code` 家族（UA 后缀 `cli`→`claude-vscode, agent-sdk/X`） |
| `claude_code_gh_action` | `claude-code-github-action` | 无样本 | 参照 `claude_code` 家族 |
| `codex_cli` | `codex_cli_rs/0.38.0` | 无样本 | 参照现有 client-types.json codex 入口（已含 `OpenAI-Beta`/`conversation_id`/`session_id`） |
| `codex_tui` / `codex_desktop` / `codex_vscode` | `Codex/0.38.0` / `codex desktop/0.38.0` / `codex-vscode/0.38.0` | 无样本 | 同上 |
| `cursor` | `Cursor/0.50.7` | 无样本 | 无 |
| `windsurf` | `Windsurf/1.5.0` | 无样本 | 无 |

**⚠️ UA 后缀 discrepancy（重要发现）**：`claude_code_sdk_ts` 在 client-types.json 中 `user_agent` 配 `sdk-ts` 后缀，但**实测 TS SDK 发的后缀是 `sdk-cli`**（claude-cli/2.1.201 (external, sdk-cli) 115 行 + 2.1.202 13 行）。`sdk-ts` 后缀在 DB 0 命中。建议把 `claude_code_sdk_ts` 的 `user_agent` 改为 `claude-cli/2.1.204 (external, sdk-cli)` —— **此为独立 bug，需 main 决策是否本任务一并修**。

---

## 真实入站特征 header 清单（剔 stripped）

### claude_code 家族（cli / sdk-cli / sdk-py 实测合并）

5 个 client_type 实测**特征 header 集完全一致**（仅 UA 字符串后缀不同），所有差异只在 `anthropic-beta` 的 token 组合（与客户端启用能力有关，非家族指纹）。**结论：claude_code 家族 5 entry 可共用同一组特征 header 清单。**

| header name | 真实样本值 | 类别 | protocol 适用性 |
|---|---|---|---|
| `anthropic-version` | `2023-06-01` | 静态快照 | **anthropic only** |
| `anthropic-beta` | `claude-code-20250219,interleaved-thinking-2025-05-14,thinking-token-count-2026-05-13,context-management-2025-06-27,prompt-caching-scope-2026-01-05,mid-conversation-system-2026-04-07,advanced-tool-use-2025-11-20,effort-2025-11-24,extended-cache-ttl-2025-04-11`（cli 主流组合；sdk 实测另含 `context-1m-2025-08-07`） | 静态快照（**月级腐化**，新 beta token 频繁加） | **anthropic only** + **仅官方 `api.anthropic.com` 生效**（headers.rs L94 第三方端点剔） |
| `anthropic-dangerous-direct-browser-access` | `true` | 静态快照 | **anthropic only** |
| `x-app` | `cli` | 静态快照 | anthropic（实测仅 anthropic 协议下出现） |
| `x-stainless-arch` | `arm64` | 静态快照（机器架构相关，跨机器需 `x86_64` 兜底） | 全协议（SDK 自带，跨协议不变） |
| `x-stainless-lang` | `js` | 静态快照 | 全协议 |
| `x-stainless-os` | `MacOS` | 静态快照（OS 相关：`Linux`/`Windows` 兜底） | 全协议 |
| `x-stainless-package-version` | `0.94.0`（anthropic SDK 版本） | 静态快照（**月级腐化**，跟 SDK 升级） | 全协议 |
| `x-stainless-retry-count` | `0` | 静态快照（首次请求固定 0） | 全协议 |
| `x-stainless-runtime` | `node` | 静态快照 | 全协议 |
| `x-stainless-runtime-version` | `v26.3.0`（node 版本） | 静态快照（**跟随本机 node**） | 全协议 |
| `x-stainless-timeout` | `600`（秒；sdk 实测另见 `300`） | 静态快照 | 全协议 |
| `x-claude-code-session-id` | `f1552905-c54d-4dd8-b96f-cf19cb7f4a1d` | **透传类禁注入**（uuid，每 session 唯一；simulation 应 `{uuid}` 占位符） | anthropic |

**关键观察**：
- `sdk-py` 也发 `x-stainless-runtime: node` + `x-stainless-lang: js` —— Claude Code SDK (Python) 实际经 node 子进程转发，**指纹和 TS SDK 完全一致**。家族内 5 entry 共享清单无副作用。
- `x-claude-code-session-id` 是 uuid，按任务规则**不注入 simulation**（标占位符候选 `{uuid}`，若要注入则需 Rust 侧 `fill_placeholder` 支持 `{uuid}` —— 当前 `apply_client_headers` 仅 `fill_placeholder(value, api_key)` 把 `{api_key}` 换 api_key，**未支持 `{uuid}`**；codex 现有 entry 已用 `{uuid}` 字面量，意味着字面量字符串 `{uuid}` 会被当 header 值发上游。需 main 决策是否扩 `fill_placeholder` 支持 `{uuid}`（uuid_sim() 已存在于 headers.rs L317-330）。

### default / quota 探测

无特征 header。`{"source":"quota"}` 是内部健康探测（不带 UA），`Python-urllib/3.13` 是 statusline bash 脚本 GET `/v1/models`，仅含 `anthropic-version: 2023-06-01`（部分）或 `proxy-connection: Keep-Alive`。**default entry 不需要补特征 header**（任务要求跳过 default）。

### codex 家族 / cursor / windsurf

DB 无样本。参照 `client-types.json` 现状（codex 4 entry 的 openai 协议已含 `OpenAI-Beta`/`conversation_id`/`session_id`，cursor/windsurf 仅 auth）。**未实测，不臆造**。

---

## 跨家族对比

| 维度 | claude_code 家族（5 entry） | codex 家族（4 entry） | cursor / windsurf |
|---|---|---|---|
| 实测样本 | 有（cli/sdk-cli/sdk-py 合计 23375 行） | **无** | **无** |
| 家族内 header 差异 | **无差异**（仅 UA 后缀异，特征 header 集相同） | 未知（无样本） | 未知 |
| 指纹类别 | anthropic Stainless SDK 指纹（x-stainless-* 11 项 + anthropic-* 3 项 + x-app） | OpenAI 指纹（OpenAI-Beta + conversation/session id） | 无（仅 UA + auth） |
| 注入紧迫性 | **高**（CLI 主流量，最易被上游识别） | 中（已有 OpenAI-Beta 基础指纹） | 低（无指纹历史） |

---

## 建议 simulation 补全清单（per client_type × per protocol）

> 剔除 stripped（host/content-length/auth/UA/CT/hop-by-hop）+ 透传类（uuid session-id 不注入或占位符）。
> 仅列**建议新增**项（现有 auth headers 保留不动）。

### claude_code / claude_code_vscode / claude_code_sdk_ts / claude_code_sdk_py / claude_code_gh_action（5 entry 共用）

**anthropic 协议**（核心场景）：

| name | value | 说明 |
|---|---|---|
| `anthropic-version` | `2023-06-01` | 静态 |
| `anthropic-beta` | `claude-code-20250219,interleaved-thinking-2025-05-14,thinking-token-count-2026-05-13,context-management-2025-06-27,prompt-caching-scope-2026-01-05,mid-conversation-system-2026-04-07,advanced-tool-use-2025-11-20,effort-2025-11-24,extended-cache-ttl-2025-04-11` | 静态快照，需定期手更；**注意：headers.rs L94 第三方 anthropic 端点会剔此头，注入仅对官方 api.anthropic.com 生效** |
| `anthropic-dangerous-direct-browser-access` | `true` | 静态 |
| `x-app` | `cli` | 静态 |
| `x-stainless-arch` | `arm64` | 静态快照（OS/机器相关，建议兜底 `arm64` 或参数化） |
| `x-stainless-lang` | `js` | 静态 |
| `x-stainless-os` | `MacOS` | 静态快照 |
| `x-stainless-package-version` | `0.94.0` | 静态快照（跟 SDK 版本） |
| `x-stainless-retry-count` | `0` | 静态 |
| `x-stainless-runtime` | `node` | 静态 |
| `x-stainless-runtime-version` | `v26.3.0` | 静态快照（跟 node 版本） |
| `x-stainless-timeout` | `600` | 静态 |

**openai 协议**（claude_code 走 cross-protocol 到 OpenAI 兼容平台时）：

实测 DB 全是 anthropic 协议入站，无 openai 协议入站样本。`x-stainless-*` 由 anthropic JS SDK 发，**OpenAI 客户端不发**，故 cross-protocol 时这些 stainless 头在 OpenAI 端是「跨协议透传」噪音（headers.rs L112-114 注释明确说「跨协议也带，上游忽略未知头不报错」）。

**保守建议**：claude_code 的 openai/gemini/default 协议**只加 UA 已有的 auth headers**（现状），**不补 stainless 指纹**（避免对 OpenAI 平台注入无意义指纹）。如需补全对称性，`x-stainless-*` 可放 openai/default 协议但标注「跨协议噪音，仅 anthropic 平台真识别」。

**gemini 协议**：现状仅 `x-goog-api-key`，不建议补 stainless（Gemini 端不认）。

### codex 家族 / cursor / windsurf

**无 DB 样本**，建议保留现状（codex 已含 `OpenAI-Beta`/`conversation_id`/`session_id`），**本任务不臆造**。若需补全需用户采集 codex_cli / cursor / windsurf 真实样本（建议抓 1 条 curl -v 流量）。

---

## Caveats / 数据缺口

1. **codex 家族 / cursor / windsurf 0 样本**：DB 中无任何 codex_cli / Codex/ / codex-vscode / Cursor / Windsurf UA 命中（严格 UA 字段扫描已确认）。补全需用户采集真实流量。
2. **claude_code_vscode / claude_code_gh_action 0 样本**：但同家族（claude_code cli/sdk）指纹完全一致，按家族共用清单推断风险低。
3. **`claude_code_sdk_ts` UA 后缀 discrepancy**：JSON 配 `sdk-ts`，实测发 `sdk-cli`。需 main 决策是否同步修 UA 字段（独立 bug）。
4. **`anthropic-beta` 月级腐化**：beta token 频繁增减，注入值需定期手更（CLAUDE.md `STATIC_MODEL_IDS` 同模式）。
5. **`anthropic-beta` 第三方端点剔除**：headers.rs L94-96 自动剔，simulation 注入仅对 `api.anthropic.com` 生效；如平台是 GLM/中转站等 anthropic 兼容端点，注入无效（被剔）。
6. **`{uuid}` placeholder 未实现**：`apply_client_headers` L310 的 `fill_placeholder` 仅替换 `{api_key}`，codex 现有 entry 的 `conversation_id: {uuid}` / `session_id: {uuid}` 实际发字面量字符串 `{uuid}` 到上游（**已存在的 bug，非本任务引入**）。若 claude_code 要注入 `x-claude-code-session-id`，需先扩 `fill_placeholder` 支持 `{uuid}`（`uuid_sim()` 已在 headers.rs L317 备好）。
7. **`x-stainless-runtime-version: v26.3.0` 跨机器差异**：实测为本机 node 版本，跨用户机器不同；若 simulation 注入固定值会偏离真实指纹，建议占位符 `{node_version}` 或固定一个保守值。
8. **`x-stainless-os / arch` 跨平台**：实测 `MacOS/arm64`，Linux/Windows 用户不同。建议按运行时探测或固定 macOS 值（多数 Claude Code 用户在 mac）。
