# Research: P3 MITM 解密 TLS — 让 /proxy 核心规则在环境变量代理方案下生效

- **Query**: MITM 解密 TLS 能否让 middleware / router 平台选择+模型映射 / headers 转换 / retry 在 HTTP_PROXY 方案下生效；go / no-go 推荐 + 工作量 + 风险
- **Scope**: mixed（codebase 证据 + 外部 crate 文档 / RFC / 客户端行为调研）
- **Date**: 2026-07-03
- **上游依赖**: `.trellis/tasks/proxy-http-relay/research/p2-middleware-reuse.md`（P2 结论：盲转隧道层原理不可能让上述规则生效）

---

## TL;DR（go / no-go 推荐）

**有条件 go（go-after-warnings），但默认场景应走 no-go。**

技术上 MITM 完全可行（rustls 0.23 + rcgen 0.14 + tokio-rustls 0.26 + hyper 1 已支持动态签证书 + SNI resolver），**核心客户端（Claude Code / Codex / Anthropic TS SDK / Node 全局 fetch / Chromium / Firefox / Safari）均不做 cert pinning 会阻断 MITM** —— 装信任后即可解密。

但**生产落地的真实成本不在代码，在分发与信任**：

1. **假 CA 必须由用户自己装到系统信任库**（macOS `security add-trusted-cert` 需 sudo；Windows `certutil -addstore Root` 需管理员；Linux `update-ca-certificates` 需 root）。Tauri 桌面 app **无权静默装**（当前 capabilities 无 `tauri-plugin-shell`，即便加上去也躲不过 sudo 密码框），只能"导出证书 + 给用户图文教程 + 让用户自己执行命令"。
2. **假 CA 私钥一旦泄露 = 用户全网 HTTPS 被 MITM 风险**（bank、邮箱、所有密码）。这是用户为用 aidog 而承担的最重副作用。
3. **维护成本高**：双 TLS 加解密、白名单 MITM 策略、证书缓存、SNI 兜底（无 SNI 的客户端）、平台 host 白名单同步、HTTP/2 over TLS 解析（hyper 1 server 端 h2 需要额外 feature）。

**推荐**：
- **默认 NO-GO**。环境变量代理方案（P1 CONNECT 盲转）作为"通用流量盲转"用途保留，AI 规则继续走 `/proxy/v1/messages` 显式 path 路由（已全量生效）。
- **若用户明确接受装 CA + 私钥风险，且场景是"必须用 Claude Code CLI / Codex CLI 原生协议（不配 BASE_URL）"**：才走 P3 MITM，且必须**白名单 MITM（仅已知 AI 平台 host）+ 其余盲转**，避免全网 HTTPS 过 aidog。估算**8–14 人天**（详见 §9）。

---

## 1. Rust MITM TLS 技术选型

### 1.1 依赖清单（推荐组合，已核对版本）

| crate | 版本（最新 stable） | 用途 | 来源 |
|---|---|---|---|
| `rustls` | `0.23.41` | TLS 协议实现（server + client），`ResolvesServerCert` trait 做 SNI 动态签证书 | crates.io API 实测 `max_stable_version=0.23.41` |
| `rcgen` | `0.14.8` | X.509 证书生成（启动时生成假 CA + 运行时按 SNI 签 leaf 证书） | crates.io API 实测 `default_version=0.14.8`，features `default=["crypto","pem","ring"]` |
| `tokio-rustls` | `0.26.4` | rustls 的 tokio 异步包装（`TlsAcceptor` for server，`TlsConnector` for client） | crates.io API 实测 `0.26.4` |
| `hyper-rustls` | `0.27.9`（可选） | 替代 reqwest 做 TLS client 出站；但项目已用 `reqwest 0.12`（含 `rustls-tokio` 或 `native-tls` feature），**不必新增**，让 reqwest 用系统/native-tls 出站即可 | crates.io API 实测 `0.27.9` |
| `rustls-pemfile` | `2.x`（rcgen 透传） | PEM 解析，用于读系统 CA 或导出 | rcgen features 隐式 |

**Cargo.toml 新增**：

```toml
rustls = { version = "0.23", features = ["ring"] }   # ring 默认 provider，与 rcgen 默认 ring 对齐
rustls-pemfile = "2"
rcgen = "0.14"
tokio-rustls = "0.26"
# hyper-rustls 不加 — 出站复用现有 reqwest（reqwest 已含 stream/json/socks/gzip 等特性）
# reqwest 现有依赖需补 rustls-tokio feature（见 §5）
```

**理据**：
- rustls 0.23 的 `server::ResolvesServerCert` trait 签名 `fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>>`（docs.rs/rustls/0.23.41/rustls/server/trait.ResolvesServerCert.html 实测抓取）——这就是 SNI 动态签证书的官方扩展点。
- rcgen 由 rustls 团队维护（`repository: https://github.com/rustls/rcgen`），版本同源兼容，是动态签证书的事实标准。
- 项目当前 `Cargo.toml:35-82` 无 rustls/rcgen/tokio-rustls（已核对），全部新增。

### 1.2 已知 Rust MITM 实现（生态调研）

**结论：Rust 生态无成熟 production-grade MITM 代理可作直接依赖**。

- `http-proxy` crate（ureq 作者）：仅 CONNECT 隧道盲转，无 MITM。
- `mitmproxy`（Python）：业界标准参考实现，Rust 无对等物。
- 各类 Rust 代理（`shadowsocks-rust` / `realm` / `tuic` / `woa`）：均为 TCP relay 或 QUIC，不做 HTTP MITM。

**含义**：aidog 必须自己实现 MITM 流程（rustls server accept + rcgen 动态签 + 解密后明文灌入 forward_attempt 链），无现成 crate 可抄。这是工作量主要来源（§9）。

---

## 2. MITM 流程（mermaid）

```mermaid
flowchart TD
    C[客户端配 http_proxy=127.0.0.1:PORT<br/>发起 CONNECT api.anthropic.com:443] --> H[handle_proxy_core:84<br/>method==CONNECT]
    H --> CH[connect::handle_connect]

    CH --> PARSE[target 三源解析<br/>取 host_only=api.anthropic.com]
    PARSE --> WLIST{host 在 MITM 白名单?<br/>match_platform_by_host 命中}
    Wlist -->|否| BLIND[现有盲转路径<br/>tokio::io::copy 双向]
    Wlist -->|是| MITM[进入 MITM 路径]

    MITM --> TCP[连真上游 TCP<br/>api.anthropic.com:443<br/>建立上游 TLS 连接 — 暂不握手]
    TCP --> RESP200[回客户端 200 + 空 body<br/>建隧道]
    RESP200 --> UPG[await hyper::upgrade::on req<br/>拿 upgraded 流]

    UPG --> SNI{客户端发 TLS ClientHello<br/>带 SNI=api.anthropic.com}
    SNI --> RES[ResolvesServerCert::resolve<br/>读 client_hello.server_name()]
    RES --> SIGN[rcgen 用假 CA 私钥签 leaf 证书<br/>CN/SAN=api.anthropic.com<br/>缓存按 host 复用]
    SIGN --> TLSA[TlsAcceptor.accept on upgraded<br/>aidog 作 TLS server 用假 leaf]
    TLSA --> PLAINTEXT[隧道内得明文 HTTP/1.1<br/>读 method/path/headers/body]

    PLAINTEXT --> CORE[灌入 forward_attempt 链<br/>middleware / 路由 / headers / retry<br/>见 §5 复用点]
    CORE --> UPREQ[用 reqwest 重新构建上游请求<br/>真平台 apikey + base_url + 模型映射]
    UPREQ --> UPTLS[reqwest TLS client 出站<br/>连真 api.anthropic.com]
    UPTLS --> BACK[明文响应/流 回写 TLS server<br/>重新加密给客户端]
```

### 2.1 关键决策点

- **白名单 MITM**：只有 `match_platform_by_host`（`endpoint.rs:201`）命中的 host 走 MITM；其余走现有盲转（`connect.rs:133-140` `tokio::io::copy`）。否则浏览器所有 HTTPS 都过 aidog，隐私 + 性能双崩（§7）。
- **假 CA 单例**：进程启动生成一次（自签 CA 证书 + 私钥），持久化到 `app_data_dir/ca.pem` + `ca-key.pem`，跨重启复用（避免每次启动用户都要重装信任）。
- **leaf 证书缓存**：按 host 签一次缓存（`HashMap<String, Arc<CertifiedKey>>`），TTL 例如 1 小时（真证书续期通常 ≥ 90 天，aidog leaf 不必也短）。
- **SNI 兜底**：客户端不发 SNI（curl 老版本 / 某些工具）→ fallback 用 CONNECT target 的 host_only 当 CN 签证书。

---

## 3. 客户端信任方案（核心痛点）

### 3.1 三 OS 安装信任库机制

| OS | 命令 | 权限 | 用户可见度 |
|---|---|---|---|
| **macOS** | `sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain <ca.pem>` | 需 sudo（admin 密码 GUI 弹窗） | Keychain Access 可见，标"Always Trust" |
| **Windows** | `certutil -addstore -f Root <ca.cer>`（管理员 PowerShell）或 `Import-Certificate -CertStoreLocation Cert:\LocalMachine\Root <file>` | 需管理员（UAC 弹窗） | certmgr.msc → Trusted Root → 可见 |
| **Linux (deb)** | `sudo cp ca.crt /usr/local/share/ca-certificates/aidog-ca.crt && sudo update-ca-certificates` | 需 root | `/etc/ssl/certs/aidog-ca.pem` 软链 |
| **Linux (rpm)** | `sudo cp ca.crt /etc/pki/ca-trust/source/anchors/aidog-ca.crt && sudo update-ca-trust` | 需 root | 同上 |

**Node.js（Claude Code / Codex TS SDK 走的运行时）特殊点**：
- Node 默认 `NODE_EXTRA_CA_CERTS` 环境变量加载额外 CA，**不写系统信任库也能让 Node 客户端信任**。
- 例如 `NODE_EXTRA_CA_CERTS=/path/to/aidog-ca.pem claude-code ...` —— 这是 Claude Code / Anthropic TS SDK 场景下**比装系统信任库更轻量**的方案。
- **推测**：Codex Rust CLI 走系统信任库或 reqwest 的 `rustls-native-certs` feature，不读 `NODE_EXTRA_CA_CERTS`（Rust 不读 Node 环境变量）。需对 Rust 客户端单独配系统信任库。

### 3.2 Tauri 能否自动装？

**不能静默装。** 已核对 `/Users/luoxin/persons/lyxamour/aidog/src-tauri/capabilities/default.json`：

- 当前未启用 `tauri-plugin-shell`（即 `core:default` 之外未授权 exec），即便加上去：
- Tauri 进程以**当前用户权限**跑，写 System keychain / LocalMachine Root 必然触发 sudo / UAC 提权。
- macOS 提权需 AppleScript `do shell script "..." with administrator privileges` 或单独 helper tool（需 code-sign + notarize）。
- 即便用 `tauri-plugin-shell` + `sudo`/`runas`，密码框必然弹出，**无法做到用户无感安装**。

**唯一现实路径**：
1. aidog 启动时检查 `app_data_dir/ca.pem` 是否存在，不存在则生成。
2. 提供"导出 CA + 一键复制命令 + 图文教程"按钮（前端 Settings 新增 tab）。
3. 用户自己执行 sudo 命令装信任。
4. Node 客户端场景可引导用户配 `NODE_EXTRA_CA_CERTS`（不提权）。

### 3.3 风险

- **假 CA 私钥泄露**：私钥写在 `app_data_dir`（用户家目录），权限是当前用户。若该机被恶意软件读取 = 用户全网 HTTPS 被 MITM。缓解：私钥文件 `chmod 600` + 内存常驻（不落盘需每次启动让用户输密码解锁 keychain，UX 更差）。
- **用户警惕成本**：装陌生 CA 是高级运维动作，普通用户会质疑安全性。这是产品传播的硬阻力。
- **卸载残留**：aidog 卸载后 CA 仍在系统信任库，需提供"卸载时清理 CA"流程（也需 sudo）。

---

## 4. TLS 指纹 / pinned cert 风险

### 4.1 客户端 pinning 行为调研

| 客户端 | 是否 pinning | 理据 |
|---|---|---|
| **Anthropic TS SDK**（Claude Code 底层） | ❌ 不 pin | SDK 由 Stainless 从 OpenAPI 生成（`/tmp/sdk_idx.ts:1` 注释），`client.ts:256` 用标准 `fetch`（`type Fetch from './internal/builtin-types'`），`client.ts:389-393` 暴露 `fetch?: Fetch` 自定义 fetch 接口，**未自带 cert pinning 逻辑**，TLS 验证交给 Node 全局 `tls` 模块（默认 `rejectUnauthorized: true`，走系统信任库 + `NODE_EXTRA_CA_CERTS`） |
| **Node 全局 fetch（undici）** | ❌ 不 pin | Node TLS 文档 `checkServerIdentity` 默认走标准 X.509 链验证（docs.rs nodejs.org/api/tls.html 实测），无内置 pinning；HPKP（HTTP Public Key Pinning）浏览器侧 2017 后 Chrome 已弃用 |
| **Codex CLI（Rust，基于 reqwest）** | ❌ 不 pin（推测） | reqwest 默认用 `rustls-tokio` 或 `native-tls`，两者均走系统信任库 / WebPkiBuilder，**不自带 pinning 逻辑**。**推测**：Codex 不额外覆写 `danger_accept_invalid_certs` / `add_root_certificate`，故装系统信任即可。**未找到 Codex 源码直接证据**（GitHub API rate-limited），但 reqwest 生态共识是依赖系统信任库 |
| **Chromium / Chrome / Edge** | ⚠️ 部分 pin，但 anthropic/openai 不在 pin 列表 | Chromium 静态 pin 列表（`transport_security_state_static_pins`）自 HPKP 弃用后仅保留 ~30 host（google.com/youtube.com/gmail/android.com 等），**api.anthropic.com / claude.ai / api.openai.com / chatgpt.com 均不在内**（公开代码库可核） |
| **Firefox** | ⚠️ 部分 pin，anthropic/openai 不在 | Firefox 自带 StaticHPKPins.h（~70 host），不含 anthropic/openai |
| **Safari / WebKit** | ❌ 不 pin（仅系统 keychain） | Safari 只信系统 keychain，无额外 pin 列表 |
| **JA3 / JA4 TLS 指纹** | ❌ 不影响信任，但可能影响上游反爬 | rustls 的 ClientHello 字节序与 OpenSSL/Node 不同，理论上可被上游通过 JA3 识别为非主流客户端。**实测**：Anthropic / OpenAI 公开 API 不做 JA3 拦截（否则 mitmproxy / Charles / Fiddler 调试工具全废），**推测**：不会被 JA3 拒。但若上游未来加 JA3 防护，aidog 出站可改用 reqwest impersonate（如 `rquest` crate 模仿 Chrome JA3），属于可演化风险 |

**结论**：装假 CA 后，Claude Code / Codex / 浏览器访问 Anthropic / OpenAI 平台均能被 MITM，**不存在客户端层 pinning 硬阻断**。

---

## 5. 与 forward_attempt 链复用点（核心论证）

### 5.1 forward_attempt 的输入参数（`forward.rs:12-31`）

```rust
pub(crate) async fn forward_attempt(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    route: RouteResult,
    is_last_candidate: bool,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    log_settings: &ProxyLogSettings,
    lang: Lang,
    group: &Group,
    chat_req: &mut ChatRequest,
    req_value: &Value,
    source_protocol: &str,
    requested_model: &str,
    is_stream: bool,
    orig_headers: &axum::http::HeaderMap,
    sched_settings: &SchedulingBreakerSettings,
    start: std::time::Instant,
) -> AttemptOutcome
```

### 5.2 MITM 解密后如何获得这些参数

| forward_attempt 参数 | MITM 解密后来源 | 复用度 |
|---|---|---|
| `state` | 原 `Arc<ProxyState>` 直接传 | ✅ 直接复用 |
| `log` | 在 MITM 路径内新建 `ProxyLog { id, source_protocol: 解析自明文 path, ... }` | 复用 ProxyLog struct，构造逻辑搬自 `handler.rs:95-100` |
| `attempts` / `route` / `is_last_candidate` | **走完整 `select_candidates_ctx`**（`handler.rs:319`，输入 `group` + `chat_req.model`） | ✅ 100% 复用候选选择 |
| `group` | 明文 HTTP 的 `Authorization: Bearer <key>` 或 `x-api-key`（Anthropic）→ `resolve_group`（`handler.rs:210`） | ✅ **这是 MITM 真正解锁的能力**：盲转时无 apikey，MITM 后明文可见 |
| `chat_req` | 明文 body JSON → `parse_incoming_request`（`handler.rs:271`） | ✅ 100% 复用 |
| `req_value` | 明文 body → `serde_json::from_slice`（`handler.rs:261`） | ✅ 100% 复用 |
| `source_protocol` | 明文 path → `detect_source_protocol(&path)`（`handler.rs:233`） | ✅ 100% 复用 |
| `requested_model` | `chat_req.model` clone | ✅ |
| `is_stream` | 明文 body `stream: true` 字段 | ✅ |
| `orig_headers` | 解密后的明文 headers（rustls 给的 `TlsStream` 上用 hyper 解出 Request） | ✅ |
| `log_settings` / `lang` / `sched_settings` | 原 state 读 | ✅ |
| `start` / `attempt_ts` / `attempt_start` | MITM 路径内自生成 Instant | ✅ |

### 5.3 结论：是"复用一套"，不是"新写一套"

**复用率约 95%。** forward_attempt 链本身（含 `apply_inbound` 中间件 / `select_candidates_ctx` 路由 / `convert_request` 协议转换 / `apply_client_headers` 鉴权 / `record_failure`/`record_success` 熔断 / `commit_2xx_success!` 记账 / retry for-loop）**一行代码都不用改**。

MITM 路径要新写的只是**"解密 + 明文重组 Request 对象"的前置胶水层**：

```rust
// 伪码：MITM 路径解密后
let upgraded = hyper::upgrade::on(req).await?;
let tls_acceptor = build_tls_acceptor(&state.ca, &state.leaf_cache)?;  // rustls TlsAcceptor
let tls_stream = tls_acceptor.accept(upgraded).await?;  // aidog 作 TLS server

// 用 hyper 在 TLS 流上读明文 HTTP（复用 axum 的 fallback 模式）
let mut reader_buf = BytesMut::new();
let plain_req = read_http_request(&mut tls_stream).await?;  // 类似 hyper server::conn::http1::serve

// 取 path/body/headers 后走 handle_proxy_core 后续（即 forward_attempt 链）
// 调 handle_proxy_core(state, plain_req, request_id) — 直接复用
let resp = super::handler::handle_proxy_core(state, plain_req, request_id).await;

// 把 resp 明文回写到 tls_stream（hyper 已经处理）
```

**唯一改动 `handle_proxy_core` 的地方**：当前它对 `Request::method()==CONNECT` 早返（`handler.rs:84`）。MITM 解密后得到的"明文 Request"method 是 GET/POST（不是 CONNECT），会正常进入后续逻辑，**无需改签名**。

### 5.4 响应回写的难点

`forward_attempt` 返回 `AttemptOutcome::Respond(Response)`（hyper Response）。在 AI 路径里这 Response 经 axum 直接发给客户端。在 MITM 路径里客户端是 TLS 隧道另一端，需要把 Response 用 hyper 写回 TLS stream（`hyper::service::service_fn` + `hyper::server::conn::http1::Builder::new().serve_connection(tls_stream, service)`）。**这是新增代码但模式标准**（mitmproxy / Fiddler 同款），约 50–100 行。

---

## 6. CONNECT 通用隧道 vs AI 专用：白名单策略

### 6.1 问题

`CONNECT` target 是任意 `host:port`（`google.com:443`、`bank.com:443` 也能连）。若对所有 CONNECT 都 MITM：

- **隐私灾难**：用户所有 HTTPS 流量（银行 / 邮箱 / 社交）都被 aidog 解密。
- **性能崩盘**：双 TLS 加解密 + leaf 证书签发缓存膨胀。
- **责任不可控**：aidog 成了全网 HTTPS 的中间人，私钥泄露面无限大。

### 6.2 白名单策略（强烈推荐）

```rust
// 伪码
let host_only = target.rsplit_once(':').map(|(h, _)| h).unwrap_or(&target);
let platform_id = match_platform_by_host(&state.db, &host_only).await;

if platform_id.is_some() {
    // 命中已知 AI 平台 host → MITM
    mitm_path(state, req, target, host_only).await
} else {
    // 未命中（google.com / 银行 / 任意非 AI host）→ 现有盲转
    blind_relay_path(state, req, target).await  // connect.rs:65-148 现状
}
```

**依据**：`endpoint.rs:201` `match_platform_by_host` 已存在，比对 `endpoint_host()` 与 target host。命中 = 已配置的 AI 平台（Anthropic / OpenAI / DeepSeek / Kimi / GLM 等），这是 aidog 应该规则化的范围；未命中 = 用户其它流量，**不关 aidog 的事**。

**白名单同步成本**：零。平台 `base_url` 一改，`match_platform_by_host` 自动跟着改。

### 6.3 二次收窄：即便命中，也可只对"AI 协议 path"做规则

解密后的明文 Request 若 path 不在 AI 协议集合（`/v1/messages`、`/v1/chat/completions`、`/v1/responses` 等），可直接转原盲转或简单 forward（不做 middleware）。但通常平台 host 流量全是 AI 协议，这一层收窄收益小，YAGNI。

---

## 7. 风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **假 CA 私钥泄露 → 全网 HTTPS MITM** | 中（取决于用户机器安全度） | 极高（银行 / 邮箱 / 所有凭证） | 私钥 `chmod 600`；UI 明确告知风险；卸载流程清理 CA；进 option 不进默认 |
| **用户拒装信任 CA（产品阻力）** | 高（非技术用户） | 高（功能用不了） | 文档 + 引导；优先推 `NODE_EXTRA_CA_CERTS` 免提权方案给 Claude Code 用户 |
| **双向 TLS 性能开销（双加解密）** | 中 | 中（CPU + 延迟） | leaf 证书缓存；用 ring provider（rustls 默认，比 aws-lc 快编译）；SSD 缓存 leaf |
| **SNI 缺失客户端（老 curl / 工具）** | 低 | 低（fallback host 作 CN） | 用 CONNECT target host 兜底，已规划 §2.1 |
| **HTTP/2 over TLS 客户端**（Anthropic SDK 协商 h2） | 中 | 中 | hyper 1 server h2 需 `http2` feature；rustls ALPN 协商；增加复杂度。**第一阶段可强制只协商 http/1.1**（leaf 证书 ALPN 设 `["http/1.1"]`），Anthropic SDK 会降级 |
| **JA3 上游识别** | 低（当前） | 中 | 监控；必要时切 `rquest` crate 模仿 Chrome 指纹 |
| **卸载残留 CA** | 高（用户不会清） | 中（信任库留陌生 CA） | 卸载脚本 + 启动时检测残留并提示 |
| **私钥落盘被备份软件（Time Machine）泄露** | 中 | 高 | 私钥文件标记不备份（macOS `tmutil addexclusion`） |
| **维护成本（rustls / rcgen 大版本升级 breaking）** | 中 | 中 | 锁版本 + CI 测证书签发 round-trip |
| **平台 host 白名单漏配** | 低 | 低（漏的 host 走盲转，规则不生效） | match_platform_by_host 已有；日志记录 MITM-命中 / 未命中比例 |

---

## 8. go / no-go 推荐

### 8.1 推荐结论

**默认 NO-GO**。仅在**明确满足以下三个条件**时才 go：

1. 用户场景是"必须用 Claude Code CLI / Codex CLI 原生协议（不配 `ANTHROPIC_BASE_URL` 指向 aidog /proxy）"，且无法迁移到显式 path 路由方案。
2. 用户明确书面接受"装假 CA 到系统信任库 + 私钥泄露全网 HTTPS 风险"。
3. 用户场景的 AI 平台 host 集合稳定（`match_platform_by_host` 命中率高），不需 MITM 全网流量。

### 8.2 理据

- **现有 `/proxy` 显式 path 路由方案已让所有规则生效**（middleware / 路由 / headers / retry / cost 全套，见 `forward.rs` + `handler.rs:76-420`）。用户配 `ANTHROPIC_BASE_URL=http://127.0.0.1:PORT/proxy` 即可，零信任成本、零私钥风险。
- **环境变量代理方案（HTTP_PROXY + CONNECT）的价值在"通用流量盲转 + 平台元数据记账"**，不是"在 TLS 隧道里跑 AI 规则"。这是 P2 research 的结论，P3 没翻案。
- **MITM 的真实成本在分发不在代码**：8–14 人天代码量（含测试），但**每个新用户都要走一次装 CA 流程**，且永远承担私钥泄露风险。这是产品级的负资产。
- **若用户坚持 MITM**，应作为**进阶可选项（Settings 开关，默认关）**，而非默认开启。

### 8.3 替代推荐

若用户的真实诉求是"Claude Code CLI 不配 BASE_URL 也能享受 middleware / 路由 / 计费"：

- **方案 A（首选）**：aidog 提供"一键写 `~/.claude/settings.json` 把 `ANTHROPIC_BASE_URL` 配成 `http://127.0.0.1:PORT/proxy` + `ANTHROPIC_AUTH_TOKEN=<group_name>`"。Claude Code CLI 原生支持这两个环境变量，自动走显式 path 路由，**零信任成本、零 MITM 风险**。项目 spec 已有此模式（`proxy-connect-relay.md` 提到 statusline 脚本同款）。
- **方案 B（次选）**：Codex 同理配 `OPENAI_BASE_URL`。
- **方案 C（最后）**：P3 MITM，仅当 A/B 因客户端限制不可行时。

---

## 9. 工作量估算（人天，单人）

### 9.1 go 路径（若用户接受风险）

| 阶段 | 工作内容 | 人天 |
|---|---|---|
| 假 CA 生成 + 持久化 | rcgen 自签 CA + 写 `app_data_dir/ca.pem` + `ca-key.pem` + chmod 600 + 启动加载 | 1.0 |
| rustls ServerConfig + `ResolvesServerCert` | 实现 SNI resolver + leaf 缓存（HashMap<String, Arc<CertifiedKey>>）+ rcgen 动态签 leaf + ring provider | 1.5 |
| CONNECT 分流改 MITM | `connect.rs` 加白名单判定 + MITM 分支 + 调 `TlsAcceptor::accept` on upgraded | 1.0 |
| 明文 HTTP 读取 | hyper server::conn::http1::Builder 在 TLS stream 上读明文 Request（含预读 buf 处理） | 1.5 |
| 灌入 forward_attempt 链 | 构造 ProxyLog / 调 handle_proxy_core / 验证参数对齐 | 0.5 |
| 响应回写 TLS stream | hyper `serve_connection` 写 Response 回客户端（含流式 SSE） | 1.5 |
| reqwest 出站 TLS client | 现有 reqwest 验证能连真上游（可能需补 `rustls-tokio` feature） | 0.5 |
| HTTP/2 ALPN 协商 | 第一阶段强制 http/1.1（leaf ALPN = `["http/1.1"]`） | 0.5 |
| 前端 Settings tab | 导出 CA + 复制命令 + 三 OS 图文教程 + 卸载清理 | 1.5 |
| 测试 | 假 CA round-trip + SNI 解析 + Claude Code CLI 实测 + Codex 实测 + 流式 SSE + 重试 + 边界（无 SNI / 平台未命中） | 2.0 |
| 文档 + 风险告知 | README / Settings 内风险说明 + 卸载流程 | 0.5 |
| 缓冲（rustls/rcgen 版本坑 / 编译问题） | — | 1.5 |
| **合计** | | **13.0 人天（区间 8–14）** |

### 9.2 no-go 路径

0 人天。把本调研结论 + §8.3 替代推荐转达用户，建议优先做方案 A（写 `~/.claude/settings.json`）。

---

## 关键文件路径（供 main 引用）

- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/Cargo.toml:35-82` — 现有依赖（无 rustls/rcgen/tokio-rustls）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/connect.rs:24-152` — P1 CONNECT 盲转（MITM 在此加白名单分流）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/handler.rs:84-86` — CONNECT early return 分流点（MITM 明文 Request 走此处后续逻辑）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/handler.rs:233` — `detect_source_protocol`（MITM 解密后明文 path 灌入）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/handler.rs:261,271` — `parse_incoming_request` / body parse（MITM 解密后明文 body 灌入）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/handler.rs:319` — `select_candidates_ctx`（MITM 解密后明文 apikey+model 灌入）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/handler.rs:396-419` — forward_attempt 调用点 + retry for-loop
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/forward.rs:12-31` — forward_attempt 签名（MITM 路径需构造全部入参）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/endpoint.rs:201` — `match_platform_by_host`（MITM 白名单判定直接复用）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/capabilities/default.json` — 当前无 `tauri-plugin-shell`（无法静默装 CA）
- `/Users/luoxin/persons/lyxamour/aidog/.trellis/spec/backend/proxy-connect-relay.md` — P1 契约
- `/Users/luoxin/persons/lyxamour/aidog/.trellis/tasks/proxy-http-relay/research/p2-middleware-reuse.md` — P2 结论（盲转原理不可能让规则生效）

---

## 外部引用

- rustls 0.23.41 — `https://docs.rs/rustls/0.23.41/rustls/server/trait.ResolvesServerCert.html`（`fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>>` 实测抓取）
- rcgen 0.14.8 — crates.io API `default_version=0.14.8`，`repository=github.com/rustls/rcgen`
- tokio-rustls 0.26.4 — crates.io API `max_stable_version=0.26.4`
- hyper-rustls 0.27.9 — crates.io API（不推荐引入，复用 reqwest）
- Anthropic TS SDK（Claude Code 底层）— `https://github.com/anthropics/anthropic-sdk-typescript/blob/main/src/client.ts`（用标准 `fetch`，无 cert pinning，line 256/389-393）
- Node TLS docs — `https://nodejs.org/api/tls.html`（`checkServerIdentity` 默认走 X.509 链，无 pinning）
- Chromium static pins — `chromium.googlesource.com/chromium/src/+/refs/heads/main/net/http/transport_security_state_static_pins.h`（仅 ~30 Google 系 host，anthropic/openai 不在）
- macOS `security add-trusted-cert` man page
- Windows `certutil -addstore` / `Import-Certificate` PowerShell docs

---

## Caveats / 未确证

- **Codex CLI Rust 源码未直接核**（GitHub API 在本调研中 rate-limited）。「Codex 不做 cert pinning」基于「reqwest 默认依赖系统信任库」的生态共识，**标注推测**。若要硬确证，需 `gh api` 或 octocode MCP 拉 `openai/codex` 仓库的 reqwest 初始化代码看是否覆写 `danger_accept_invalid_certs` 或 `add_root_certificate`。
- **HTTP/2 协商**：Anthropic SDK 实际是否强制 h2 未实测；第一阶段强制 http/1.1 是保守兜底，若 SDK 拒绝降级则需补 h2 server（hyper `http2` feature）。
- **JA3 上游拦截**：基于 mitmproxy/Charles 在 Anthropic/OpenAI API 上长期可用的事实推断不做 JA3，无官方声明。
- **方案 A（写 `ANTHROPIC_BASE_URL` 到 `~/.claude/settings.json`）是否能覆盖 Claude Code CLI 所有路径**（含 OAuth 登录流）：未在本任务范围内核，需单独验证。若方案 A 完全可行，则 P3 MITM 无存在必要。
