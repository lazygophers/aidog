---
updated: 2026-07-03
rewrite-version: 1
authored-by: trellisx-spec
mode: optimize
---

# Proxy CONNECT 隧道 (HTTP Relay)

何时被读: 改 `src-tauri/src/gateway/proxy/` (尤其 CONNECT handler / 隧道 / proxy_log 写入 / 平台 host 匹配) 时
谁读: main / sub-agent
不遵守的代价: CONNECT 路由失效 / 隧道字节丢失 / stats_agg 污染 / AI 协议路由回归破。P1 (`07-02-07-01-proxy-http-relay-p1`) 实证 + research (`07-01-proxy-http-relay/research/http-relay-research.md`) 预判双错修正沉淀。

---

## CONNECT 路由契约 (MUST)

> 违反代价: `.route()` 注册 CONNECT → authority-form URI `host:port` 经 axum path matcher 不可靠 → 隧道建不起。research 结论 1 被实测否决。

- **禁用 axum `.route()` 注册 CONNECT** — authority-form URI (`CONNECT host:port HTTP/1.1`) 的 path 段是 `host:port`, 经 axum path matcher 路由不可靠
- **改在 `handle_proxy_core` 头部按 `req.method() == Method::CONNECT` 早期分流** — `handler.rs` 第一行 `if req.method() == CONNECT { return connect::handle_connect(...) }`, 非 CONNECT 请求原样 fallthrough 到现有 path 路由
- **early return 不破现有路由** — CONNECT 分流必须是 handle_proxy_core 的 early return; GET `/` `/proxy` `/models` `/v1/models` + POST `/api/*` + AI path (`/proxy/v1/messages` 等) 全部走原显式 `.route()` 或 fallback, 不受 CONNECT 分流影响
- CONNECT 响应: `200 OK + Body::empty()`, **禁 `Connection: upgrade` header** (hyper h1 role.rs 规则, 加了会断隧道)

## CONNECT target 多源解析 (MUST)

> 违反代价: `req.uri().path()` 对 authority-form URI 返空 → `target=""` → `TcpStream::connect("")` 必败 → 隧道自上线 100% 502 (diag-42617827 实证 6 连发全 502)。http 标准: authority-form URI path 段为空, authority 在 `uri().authority()`。

- **target 多源取**: `[uri().path().trim_start_matches('/'), uri().authority().map(|a| a.as_str()), HOST header].iter().find(|s| !s.is_empty())` — 顺序 path → authority → Host header 兜底
- **禁 `req.uri().path()` 单源** — axum 0.8 / hyper 1 对 authority-form (`CONNECT host:port`) 的 path() 返 `""` (非 `host:port`); 单源取必得空 target
- **空 target 早返 400** — 三源皆空 = 客户端坏请求, 返 `400 Bad Request` 早退, **禁走 `TcpStream::connect("")` 路径** (必败 502 + 误导诊断)
- **tracing 取证**: 入口 `info!(uri, method, target, "connect recv")` + 空 target `warn!(uri)` — CONNECT 复现可从 stderr 看真实 URI 结构
- **host_only 仍从 target `rsplit_once(':')`** — 多源取后 target 形如 `host:port`, rsplit_once 取 host 段喂 `match_platform_by_host`, 逻辑不变

## hyper-util upgrade downcast 类型 (MUST)

> 违反代价: downcast 类型错 → 取不到底层流 → 隧道空转 / panic。research 说 `downcast::<TcpStream>`, 实测失败。

- **downcast 类型参数 = `TokioIo<TcpStream>` 非 `TcpStream`** — `axum::serve` 喂入的是 `TokioIo<TcpStream>` (impl hyper Read/Write trait), 非 raw `TcpStream` (后者不 impl hyper traits)
- 取回后 `parts.io` 再包一层 `TokioIo::new()` 转 tokio IO, 供 `tokio::io::copy` 使用
- 预读 buf (`parts.read_buf`) **须在双向 copy 前 flush 到上游** — 防 TLS ClientHello 首字节已读入 buf 未转发, 隧道建后客户端握手失败

## TCP 双向隧道 (MUST)

- `tokio::io::copy` 双向 + `tokio::join!` 同时转发两向
- 字节 u64 返回值: P1 决策**不入库** (YAGNI, 用户锁), 仅记 `duration_ms` (`Instant::now` 起止 → `as_millis() as i32`); 未来若加字节统计须 migration 加列
- 上游 TCP 连接失败 → 响应 `502` + 落 proxy_log (status=502); 客户端升级前断开 → `499` + 落 log

## proxy_log 写入契约 (MUST — 不污染 stats_agg)

> 违反代价: CONNECT 流量走 `upsert_log` → 触发 `upsert_stats_agg` + `agg_mark_first` → 统计页虚高 (隧道流量非 AI 请求, 不该进 AI 统计)。

- **`upsert_connect_log` 独立路径** — 直接走 `insert_proxy_log_columns`, **禁调 `upsert_log`** (后者含 stats_agg 聚合); grep `upsert_connect_log` 函数体必须 0 处 `upsert_log` / `agg_mark_first` / `upsert_stats_agg`
- 字段: `source_protocol="http-connect"` / `target_protocol="http-connect"` / `tokens=0` / `cost=0` (P1 不解析 body) / `request_url=<CONNECT host:port>` / `platform_id` (host 命中→平台 id, 否则 0) / `group_key` (命中→关联分组, 否则 '') / `duration_ms` / `status_code`
- **schema 列名 `group_key`** (非 `group_name`) — Migration 010 `RENAME COLUMN group_name TO group_key` (`schema_late.rs:120`); `PROXY_LOG_COLUMNS` + struct 全用 `group_key`

## 平台 host 匹配 (MUST)

- `match_platform_by_host` (新增, `endpoint.rs`) — CONNECT target host 段比对平台 base_url host (复用 `endpoint_host()`)
- 命中 `status != Disabled` 的平台 (Enabled + AutoDisabled) → 返 platform_id + 关联 group_key; 未命中 → None → 调用方写 platform_id=0, group_key=''
- O(n) 全平台扫描, 平台数小可接受; 未来平台数大可加 host→platform_id 缓存

## 前端筛选 sentinel (MUST)

- Logs/Stats 平台筛选「无平台」: value `"0"` → `Number("0")=0` → `platform_id=0` (truthy 透传)
- 分组筛选「无分组」: sentinel `__none__` → 前端映射空串 → `group_key=''` (避与「全部」`""` 撞; FilterDropdown 用严格 `value === o.value` 字符串相等)
- Stats 后端 `Option<String>`: `filter_group === SENTINEL ? "" : filter_group` → `Some("")` → SQL `group_key = ''`

## 验证

```bash
# CONNECT 分流 early return, 非 CONNECT 原 fallthrough
grep -n "Method::CONNECT" src-tauri/src/gateway/proxy/handler.rs  # handle_proxy_core 头部

# CONNECT target 多源取 (禁 path() 单源返空致 502)
grep -n "authority()" src-tauri/src/gateway/proxy/connect.rs  # 多源兜底 target 解析
grep -n "BAD_REQUEST\|missing target" src-tauri/src/gateway/proxy/connect.rs  # 空 target 早返 400

# CONNECT 响应禁 Connection: upgrade
grep -n "upgrade" src-tauri/src/gateway/proxy/connect.rs  # 0 处 connection upgrade header

# downcast 类型 TokioIo<TcpStream>
grep -n "TokioIo" src-tauri/src/gateway/proxy/connect.rs  # downcast + 再包

# upsert_connect_log 不污染 stats_agg
grep -n "upsert_connect_log" src-tauri/src/gateway/proxy/log.rs  # 函数体 grep upsert_log/agg_mark_first/upsert_stats_agg = 0

# schema 列名 group_key
grep -n "group_key" src-tauri/src/gateway/db/proxy_log.rs  # PROXY_LOG_COLUMNS + struct
```

## MITM CA 信任库安装 (MUST — 三 OS 原生提权)

> 违反代价: 假 CA 装不进系统信任库 → 客户端不信任 AirDog 签的 host 证书 → MITM 解密全挂；零背景用户面对 `command not found` 兜底弹窗无法自理。research (`07-03-mitm-ca-elevated-install/research/elevation-feasibility.md`) 实证 + 插件源码双源沉淀。

何时被读: 改 `src-tauri/capabilities/mitm-ca.json` / `src-tauri/src/gateway/mitm/ca.rs` (trust_ca_command / untrust_ca_command) / `src-tauri/src/commands/mitm.rs` (CaCommandSpec) 时

### capability 键名必须 `cmd` (MUST — 静默丢弃根因)

> 违反代价: JSON 写 `"command"` → tauri-plugin-shell-2.3.5 `#[serde(rename = "cmd")]` (scope_entry.rs:27) + `EntryRaw` 无 `deny_unknown_fields` → `command` 键**静默丢弃** → scope 解析失败 → `ProgramNotAllowed` → 前端兜底弹窗。**`cargo build` 不报错** (build 期不反序列化 scope，运行时首次 invoke 才 lazy 解析)。

- **capability JSON 命令键必须 `"cmd"`，禁 `"command"`** — 所有 plugin-shell scope entry
- 校验: `grep -n '"command"' src-tauri/capabilities/*.json` 必须 0 命中
- 同模式风险: 任何 `capabilities/*.json` 用 plugin-shell scope 的都适用（不只 mitm-ca）

### 三 OS 原生提权 wrapper (MUST — 零背景用户无需手敲 sudo)

> 违反代价: plugin-shell `execute` 不提权 → `security add-trusted-cert -k /Library/Keychains/System.keychain` 等需 root 的命令 exit≠0 → 兜底弹窗。OS 原生提权让 OS 自己弹密码框。

- **macOS**: `cmd: /usr/bin/osascript`, args `["-e", "do shell script \"<inner command>\" with administrator privileges"]` — inner 双引号 AppleScript `\"` 转义
- **Windows**: `cmd: ...\powershell.exe`, args `["-Command", "$p = Start-Process -FilePath <tool> -ArgumentList ... -Verb RunAs -Wait -PassThru; exit $p.ExitCode"]` — **必须 `-PassThru` + `exit $p.ExitCode`** 传播被提权进程 exit code (不加 exit code 丢失, powershell 自身 exit 0)
- **Linux**: `cmd: /usr/bin/pkexec`, args `["/bin/sh", "-c", "<inner sh>"]` — 原命令串不变, 仅前置 pkexec

### validator 锚定语义 (MUST — raw 控制 ^...$ 包裹)

> 违反代价: validator regex 写错锚定 → scope 拒合法 args / 纵容非法注入。

- **`raw: false` (默认)**: plugin-shell 自动包 `^...$` 全串匹配 (scope.rs:93-99 fullmatch)
- **`raw: true`**: 不锚定，validator 自写 `^...$`；**union regex (`A|B`) 必须用 raw:true** (默认包成 `^A|B$` 语义错)
- validator 是 Rust regex crate (RE2, **不支持 lookahead/回溯**); JSON 反斜杠双重转义 `\\.` `\\$`
- **validator 必须与 Rust 命令产出 args 字面镜像** (双向锁测试, 见 ca_linux_capability_validator_matches_commands 模式)

### 失败分类 (前端 classifyTrustError, research Q5)

- exit code 三 OS 各异, **靠 stderr 文本区分取消/密码错/无 agent/命令失败**:
  - macOS osascript: exit 恒 1; stderr `(-128)` = 用户取消, `Authorization` = 密码错
  - Windows UAC: stderr 含 `1223` (ERROR_CANCELLED) = 取消
  - Linux pkexec: exit `126` = 取消, `127` = auth 失败 / 无 polkit agent (stderr 含 `agent`/`polkit`), 其他 = 命令失败

### 验证

```bash
# capability cmd 键 (禁 command)
grep -n '"command"' src-tauri/capabilities/mitm-ca.json  # 必须 0 命中
grep -c '"cmd"' src-tauri/capabilities/mitm-ca.json      # 三 OS trust/untrust 5 处

# 三 OS 提权 wrapper (ca.rs trust/untrust)
cd src-tauri && cargo test ca_trust_command_returns_os_specific -- --nocapture
cd src-tauri && cargo test ca_cleanup_untrust_current_os -- --nocapture
cd src-tauri && cargo test ca_cleanup_untrust_command_tokens_locked -- --nocapture  # 三 OS wrapper token 字面锁

# validator 镜像 (capability regex ↔ ca.rs args 产出)
cd src-tauri && cargo test ca_linux_capability_validator_matches_commands -- --nocapture
```
