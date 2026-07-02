# Research: HTTP CONNECT 代理实现 (axum 0.8 + Rust)

- **Query**: 调研 aidog 新增「通用 HTTP CONNECT 代理」关键技术点 (P1 隧道 + P2 MITM 解密计费)
- **Scope**: mixed (代码库 + 第三方 crate 源码 cargo registry)
- **Date**: 2026-07-02
- **任务路径**: `.trellis/tasks/07-01-proxy-http-relay`

## 关键事实清单 (Lock 版本对齐 Cargo.lock)

| Crate | 版本 | 来源 |
|---|---|---|
| axum | **0.8.9** | `src-tauri/Cargo.lock:362` |
| hyper | **1.10.1** | `src-tauri/Cargo.lock:2093` |
| hyper-util | **0.1.20** | `src-tauri/Cargo.lock:2146` |
| tokio | **1.52.3** | `src-tauri/Cargo.lock:5307` |
| tokio-rustls | **0.26.4** | `src-tauri/Cargo.lock:5354` |
| rustls | **0.23.40** | `src-tauri/Cargo.lock:4004` |
| rustls-pki-types | **1.10.1 / 1.14.1** (并存) | Cargo.lock |
| rcgen | **0.14.8 (未锁,需新增)** | `~/.cargo/registry/src/.../rcgen-0.14.8` 已下载 |
| ring | 0.17.14 | rustls 当前 provider |

**rustls 重要约束**: rustls 0.23 默认 feature 是 `aws_lc_rs`,但本仓库锁定的 rustls 实际启用了 `ring`(`Cargo.lock:4009` deps 含 `"ring"`)。rcgen 0.14 默认 feature 也是 `ring`(`Cargo.toml:67`)。两者**必须统一在 ring** 才能让 rcgen 签出的私钥被 rustls `crypto::ring::sign::any_supported_type` 接受(混用 aws_lc_rs 会导致 `SigningKey` 类型不匹配)。新依赖声明:

```toml
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "logging", "prefer-post-quantum"] }
rcgen = "0.14"   # 默认带 ring + pem,与 rustls 的 ring provider 对齐
```
(`default-features=false` 避免 rustls 默认 `aws_lc_rs` 强行引入 `aws-lc-sys` C 编译依赖;`prefer-post-quantum` / `logging` / `std` 是当前已启用特性的最小集,与 reqwest 已启用的 rustls 子集不冲突)

---

## 1. axum 0.8 CONNECT 方法处理路径

### 结论
axum 0.8.9 **原生支持** CONNECT method。`axum::serve` 底层用 `hyper_util::server::conn::auto::Builder::serve_connection_with_upgrades`(`axum-0.8.9/src/serve/mod.rs:396`),HTTP upgrade 与 CONNECT 隧道机制默认开启,无需额外 feature / flag。

注册方式与普通路由相同,handler 从 `Request` extensions 取 `hyper::upgrade::OnUpgrade`(`axum-0.8.9/src/extract/ws.rs:141` 是同一模式的活样本 — WebSocket upgrade)。

**CONNECT 与 WebSocket upgrade 的关键差异**: CONNECT 请求**没有 body**(hyper 1.x server decoder 对 `Method::CONNECT` 返回 `length(0)`,`hyper-1.10.1/src/proto/h1/role.rs:1275`),且响应**必须是 2xx + 空 body + 无 `Connection: upgrade` / `Upgrade` 头**(`hyper-1.10.1/src/proto/h1/role.rs:380-384`: CONNECT 2xx 响应禁止带 content-length / transfer-encoding)。WebSocket 用 `101 Switching Protocols` + `Connection: upgrade`,CONNECT 用 `200 OK` 即建立隧道,hyper 内部自动触发 upgrade。

### 关键 API 引用
- `hyper::upgrade::on(req) -> OnUpgrade`(`hyper-1.10.1/src/upgrade.rs:106`):从 `Request`/`Response` 取 pending upgrade future。文档明确写「HTTP `CONNECT`」是该模块支持的两种 upgrade 之一(`upgrade.rs:8-9`)。
- `axum::routing::connect(handler)`(`axum-0.8.9/src/routing/method_routing.rs:335`):CONNECT method router。
- `MethodFilter::CONNECT`(`axum-0.8.9/src/routing/method_filter.rs:29`):可用 `.route_layer(MethodFilter::CONNECT)`) 做方法过滤。
- `axum::serve(listener, app)`(`src-tauri/src/gateway/proxy/mod.rs:221`)已用 `serve_connection_with_upgrades`,**无需改启动逻辑**。

### 坑: hyper-util auto 包装的 IO 类型
axum 的 `serve` 用 `hyper-util` 的 `auto::Builder`,它会**用私有的 `Rewind<T>` 包装底层 `TcpStream`**(`hyper-util-0.1.20/src/server/conn/auto/upgrade.rs:9-13` 注释明说)。`on_upgrade().await` 拿到的 `Upgraded` 不能直接 `downcast::<TcpStream>`,必须用 `hyper_util::server::conn::auto::upgrade::downcast::<TcpStream>(upgraded)`(`hyper-util-0.1.20/src/server/conn/auto/upgrade.rs:24`)才能取回原始 `TcpStream` + 预读缓冲(`read_buf`)。**这个 read_buf 必须先发到对端**,否则 client 发的 TLS ClientHello 前若干字节会丢。

### 最小可行骨架 (Rust 伪码)

```rust
// src-tauri/src/gateway/proxy/connect.rs (新文件)
use axum::{extract::{Request, State}, response::Response, http::StatusCode};
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{ProxyState, ProxyLog, upsert_log_proxy_meta_only}; // 见第 5 节

/// CONNECT handler — 路由挂载:
///   .fallback(handle_proxy)                    // 现有 AI 协议代理
///   .route("/", get(handle_root))              // 健康
/// 注:CONNECT 必须在 fallback 之前被显式路由捕获,fallback(handle_proxy) 会
/// 把 CONNECT 当成普通请求 → 进 read body (CONNECT 无 body,h1 decoder 直接 0 长度,
/// 但 handle_proxy 走到 resolve_group 会返回 404,隧道建立失败)。
/// 所以独立挂 .fallback 的 on_unmatched 之前用 .route_service 或方法过滤拦截。
pub async fn handle_connect(
    State(state): State<Arc<ProxyState>>,
    req: Request,
) -> Response {
    // CONNECT target = URI 的 authority 形式 "host:port" (RFC 7231 §4.3.6)
    let target = req.uri().path(); // 形如 "api.anthropic.com:443"
    let host_only = target.rsplit_once(':').map(|(h, _)| h).unwrap_or(target);
    let on_upgrade: OnUpgrade = hyper::upgrade::on(&req); // 借用版,见 upgrade.rs:106

    // 1. P1 平台匹配:只用 host(无 apikey) — 见第 4 节
    let platform_id = super::endpoint::match_platform_by_host(&state.db, host_only).await
        .unwrap_or(0); // 0 = 无平台,proxy_log.platform_id=0 兼容现状

    // 2. 解析上游地址 → 建 TCP 连接 (隧道另一端)
    let upstream = match TcpStream::connect(target).await {
        Ok(s) => s,
        Err(e) => {
            // 写 proxy_log(status=502, platform_id) — 见第 5 节
            return (StatusCode::BAD_GATEWAY, format!("connect {target} failed: {e}")).into_response();
        }
    };

    // 3. 响应 200 + 空 body 建立隧道 (hyper 1.x 自动写 status line)
    //    不设 Connection/Upgrade 头(hyper h1 role.rs:380-384 禁止)。
    let resp = Response::builder().status(StatusCode::OK).body(axum::body::Body::empty()).unwrap();

    // 4. spawn 双向转发 (response 已先返回,upgrade 在 task 内 await)
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    tokio::spawn(async move {
        let upgraded = match on_upgrade.await {
            Ok(u) => u,
            Err(e) => { tracing::warn!(error=%e, target=%target, "connect upgrade failed"); return; }
        };
        // hyper-util auto 包装:downcast 回 TcpStream + 拿预读 buf
        let parts = match hyper_util::server::conn::auto::upgrade::downcast::<TcpStream>(upgraded) {
            Ok(p) => p,
            Err(_) => { tracing::warn!("downcast TcpStream failed"); return; }
        };
        // 关键: 预读字节 (client 可能已发了 TLS ClientHello) 先 flush 到上游
        let mut client = parts.io;
        if !parts.read_buf.is_empty() {
            let _ = client.write_all(&parts.read_buf).await;
        }
        let (mut cr, mut cw) = client.into_split();
        let (mut ur, mut uw) = upstream.into_split();

        // 双向 copy + 统计字节 — 见第 2 节
        let (c2u, u2c) = tokio::join!(
            copy_with_count(&mut cr, &mut uw),
            copy_with_count(&mut ur, &mut cw),
        );
        let bytes_up = c2u.unwrap_or(0);
        let bytes_down = u2c.unwrap_or(0);
        let duration_ms = start.elapsed().as_millis() as i32;

        // P1: 仅写元数据 (host/status/duration/字节流);tokens/cost=0
        // 用专用轻量函数,绕开 handler.rs 的 ChatRequest parse 路径(无 body)
        super::log::upsert_connect_log(&state, request_id, target.to_string(),
            platform_id, 200, duration_ms, bytes_up, bytes_down).await;
    });

    resp
}
```

**挂载点**: `proxy/mod.rs:182` Router 构造处,在 `.fallback(handle_proxy)` 之前。axum fallback 只对未匹配路由生效,CONNECT 显式路由会优先匹配。

---

## 2. TCP 隧道双向转发 + 字节统计

### 结论
标准 tokio 双向 copy。`tokio::io::copy` 返回 `u64`(拷贝字节数),用 `tokio::join!` 并发双向。任一方向 EOF/err 即关闭对端(对端写一半的 drop 会触发 TCP FIN)。

### API
- `tokio::io::copy(&mut reader, &mut writer) -> io::Result<u64>`(返回拷贝字节数)
- `tokio::join!(fut_a, fut_b)`:并发,等两者都完成(若需 short-circuit 用 `tokio::select!`)
- `TcpStream::into_split()` → `(ReadHalf, WriteHalf)`(对 split 后两端各自 copy)
- `hyper_util::rt::TokioIo`:hyper 的 `Upgraded` 实现 hyper 自家 `Read/Write` trait,若直接用 hyper 的 trait 可包 `TokioIo(upgraded)` 转 `AsyncRead/AsyncWrite`;但本项目用 `downcast::<TcpStream>` 拿到原生 `TcpStream`,直接走 tokio IO,**无需 `TokioIo` 包装**。

### 骨架
```rust
use tokio::io::{AsyncRead, AsyncWrite};

async fn copy_with_count<R, W>(r: &mut R, w: &mut W) -> std::io::Result<u64>
where R: AsyncRead + Unpin, W: AsyncWrite + Unpin,
{
    tokio::io::copy(r, w).await
}

// 用法 (见第 1 节骨架):
// let (bytes_up, bytes_down) = tokio::join!(
//     copy_with_count(&mut client_read, &mut upstream_write),
//     copy_with_count(&mut upstream_read, &mut client_write),
// );
```

### 增强点 (按需,不在 P1 必须)
- **idle timeout**: `tokio::io::copy` 无超时,长连接隧道会永久挂。用 `tokio::time::timeout` 包每方向 copy,任一超时即关两边:
  ```rust
  let c2u = tokio::time::timeout(Duration::from_secs(idle_secs), copy_with_count(...));
  ```
- **short-circuit**: 一端 EOF 另一端可能还在写,用 `tokio::select!` 或手动 `drop` write half 触发 peer EOF。简单方案:`tokio::join!` + 任一返回后整体 drop 即可(TCP RST/FIN 由内核处理)。

---

## 3. P2 MITM 选型 (rustls + rcgen 动态签证书)

### 结论
P2 在第 1 节骨架的「双向 copy」前**插入一层 TLS accept**:client → [TLS accept with leaf cert] → 解密后的明文 HTTP → 读 body 解析 AI → 转发上游(上游仍走 reqwest 自有 TLS)。

链路:
1. 启动时一次性生成自签 CA(`rcgen::generate_simple_self_signed` 或 `CertificateParams::signed_by`)→ 落盘 PEM + 引导用户加 macOS keychain 信任。
2. 每条 CONNECT 443 流:用 SNI(client_hello.server_name)动态签 leaf cert(`CertificateParams { san: [host], ... }.signed_by(&ca_cert, &ca_key)`)→ 缓存(LRU,按 host)→ 喂给 `rustls::server::ResolvesServerCert`。
3. `TlsAcceptor::accept(client_tcp)` 完成 TLS 握手,拿到明文 `TlsStream<TcpStream>` → 当 HTTP/1.1 server 读 body。

### 依赖清单 (Cargo.toml additions)
```toml
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "logging"] }
rcgen = "0.14"          # 默认 ring + pem
# tokio-rustls 0.26.4 已在 lock,直接用即可(目前是 reqwest 间接依赖,加到 [dependencies] 显式)
tokio-rustls = "0.26"
hyper-util = { version = "0.1", features = ["tokio"] }  # 已在 lock,auto::upgrade::downcast 用
```
**注意**: rustls 加到 `[dependencies]` 时**必须显式 `default-features = false`**,否则 `default = ["aws_lc_rs"]` 会强引入 `aws-lc-sys`(C 编译,macOS 需 cmake/nasm,CI 计费分钟暴增)。reqwest/tauri 间接拉的 rustls feature 在 cargo feature unification 下会与你的合并 — 用 `cargo tree -e features -p rustls` 确认最终是 `ring` 还是 `aws_lc_rs` 胜出;`Cargo.lock:4009` 显示当前是 `ring`,保持即可。

### API 引用
- `rustls::server::ResolvesServerCert::resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>>`(`rustls-0.23.40/src/server/server_conn.rs:124,129`)— 自定义此 trait,按 SNI 返回动态签的 cert。
- `ClientHello::server_name() -> Option<&str>`(`server_conn.rs:157`)— 取 SNI host。
- `rustls::sign::CertifiedKey::new(cert: Vec<CertificateDer>, key: Arc<dyn SigningKey>)`(`rustls-0.23.40/src/crypto/signer.rs:180`)。
- `rustls::crypto::ring::sign::any_supported_type(&key_der) -> Result<Arc<dyn SigningKey>>`(`signer.rs:31,52` 注释)。
- `rustls::crypto::ring::default_provider() -> CryptoProvider`(`rustls-0.23.40/src/crypto/ring/mod.rs:31`)。
- `ServerConfig::builder_with_provider(provider)`(`server_conn.rs:498`)`.with_no_client_auth().with_cert_resolver(Arc::new(resolver))`。
- `tokio_rustls::TlsAcceptor::from(Arc<ServerConfig>)` → `.accept(tcp_stream)`(`tokio-rustls-0.26.4/src/server.rs:19,31`)。
- rcgen: `CertificateParams::signed_by(self, ca_cert, ca_key) -> Result<Certificate, Error>`(`rcgen-0.14.8/src/lib.rs:9`)签 leaf;`generate_simple_self_signed(sans) -> Result<CertifiedKey<KeyPair>, Error>`(`lib.rs:128`)签 CA。

### macOS CA 信任引导命令
自签 CA 落盘 `~/Library/Application Support/aidog/ca.pem` 后,引导用户信任(Tauri 弹原生 prompt 或在 Settings 页提供「安装 CA」按钮):

```bash
# 选项 A: 写 login keychain (用户态,免 sudo,推荐 GUI app)
security add-trusted-cert -d -r trustRoot -k ~/Library/Keychains/login.keychain-db <ca.pem>

# 选项 B: 写 System keychain (需 sudo + GUI 弹密码框,影响全机)
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain <ca.pem>
```
- `-d` 加入信任域,`-r trustRoot` 设为受信任根。
- Tauri 调用方式: `tauri-plugin-shell` 或 `std::process::Command::new("security").args([...])`;若需提权(sudo)用 `osascript -e 'do shell script "..." with administrator privileges'`(弹原生 macOS 提权框)。本项目 `gateway/scripts.rs` 已有 std::process::Command 调外部脚本的先例,可直接复用模式。

### Tauri 调骨架
```rust
// src-tauri/src/gateway/proxy/mitm.rs (P2 新文件)
use std::process::Command;
pub fn install_ca_to_login_keychain(ca_pem_path: &str) -> Result<(), String> {
    let out = Command::new("security")
        .args(["add-trusted-cert", "-d", "-r", "trustRoot",
               "-k", "~/Library/Keychains/login.keychain-db", ca_pem_path])
        .output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into());
    }
    Ok(())
}
```

### 动态签 cert resolver 骨架
```rust
use rustls::server::{ClientHello, ResolvesServerCert, ServerConfig};
use rustls::sign::CertifiedKey;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;

struct DynamicCertResolver {
    ca_cert: rcgen::Certificate,        // 已签的 CA 证书(内存持有用于签 leaf)
    ca_key: rcgen::KeyPair,             // CA 私钥
    cache: Mutex<HashMap<String, Arc<CertifiedKey>>>,  // host → 已签 leaf
}

impl ResolvesServerCert for DynamicCertResolver {
    fn resolve(&self, hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        let host = hello.server_name()?.to_string();
        // 查缓存
        if let Some(c) = self.cache.lock().unwrap().get(&host).cloned() { return Some(c); }
        // 动态签 leaf
        let mut params = rcgen::CertificateParams::new(vec![host.clone()]);
        params.distinguished_name = rcgen::DistinguishedName::new();
        // signed_by 需要 &Certificate (CA 已签的) + &KeyPair (CA 私钥)
        let leaf = params.serialize_request().ok()?
            .self_signed(&self.ca_key).ok()?; // 或 signed_by 如果有完整 CA 证书链
        let cert_der = CertificateDer::from(leaf.cert.der().to_vec()).into_owned();
        let key_der = leaf.signing_key.serialize_der().ok()?;
        let signing_key = rustls::crypto::ring::sign::any_supported_type(
            &PrivateKeyDer::try_from(key_der).ok()?).ok()?;
        let ck = Arc::new(CertifiedKey::new(vec![cert_der], Arc::new(signing_key)));
        self.cache.lock().unwrap().insert(host, ck.clone());
        Some(ck)
    }
}

pub async fn mitm_accept(client_tcp: tokio::net::TcpStream, resolver: Arc<DynamicCertResolver>) {
    let provider = rustls::crypto::ring::default_provider();
    let config = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_no_client_auth()
        .with_cert_resolver(resolver);
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    match acceptor.accept(client_tcp).await {
        Ok(tls_stream) => { /* 把 tls_stream 当 HTTP server 读 body → AI 解析 → 转发上游 */ }
        Err(e) => tracing::warn!(error=%e, "mitm tls accept failed"),
    }
}
```
> **注**: rcgen 0.14 的 `Certificate` / `serialize_request` API 名称需对照 `rcgen-0.14.8/src/lib.rs` 实测调整(本次未逐行验证签 leaf 的 method 名,rcgen 0.13→0.14 API 有改名)。`需要:` 实现时跑一次 `cargo doc --open -p rcgen` 核对签 leaf 的确切方法链。

---

## 4. 现有平台匹配复用点 (resolve_group / host 提取 / apikey)

### 结论
P1 隧道**只有 host**(CONNECT target),无 apikey(HTTPS 隧道不解密,body 不可见,Authorization 在 TLS 内)。P2 MITM 解密后才能从 HTTP body / header 拿 apikey。复用点分两段:

### P1: 仅 host 匹配 (新函数,需新增)
现有 `endpoint.rs` 有现成的 host 提取工具 `endpoint_host(base_url) -> Option<String>`(`src-tauri/src/gateway/proxy/endpoint.rs:72-92`):剥 scheme + userinfo + port,小写化。**可直接复用**遍历所有平台 + 所有 endpoints,比对 CONNECT target 的 host 段:

```rust
// 新增 src-tauri/src/gateway/proxy/endpoint.rs
pub(crate) async fn match_platform_by_host(db: &Db, connect_host: &str) -> Option<u64> {
    let platforms = super::db::list_platforms(db).await.ok()?;
    platforms.iter()
        .filter(|p| p.status != PlatformStatus::Disabled)  // 仅 enabled/auto_disabled
        .find(|p| {
            // 比对主 base_url + 所有 endpoints 的 base_url host
            endpoint_host(&p.base_url).as_deref() == Some(connect_host)
                || p.endpoints.iter().any(|ep| endpoint_host(&ep.base_url).as_deref() == Some(connect_host))
        })
        .map(|p| p.id)
}
```
> **api_key 在 P1 不可用**: `Platform.api_key`(`src-tauri/src/gateway/models/platform.rs:162`)是出站发上游用的 key,不是入站鉴权 token。入站鉴权用 `Group.group_key`(`models/group.rs:13`)。CONNECT 隧道 P1 不解密 → 拿不到入站 Authorization → **无法做 group 路由**,只能按 host 标 platform_id(写 proxy_log 用),不计费、不入候选选择。这是 P1 的本质限制,设计上接受。

### P2: MITM 解密后 apikey 可用
解密拿到明文 HTTP body 后,复用现有完整链路:
1. 从明文 header 取 `Authorization: Bearer <token>` 或 `x-api-key`(`src-tauri/src/gateway/proxy/handler.rs:144-155` 已有完整提取逻辑可抽函数)。
2. `resolve_group(db, Some(token))`(`endpoint.rs:148`)拿 Group。
3. `detect_source_protocol(&path)`(`endpoint.rs:9`)从明文 path 推断协议。
4. `adapter::parse_incoming_request(&source_protocol, &body_json)`(`adapter/converter/request.rs:69`)→ `ChatRequest`。
5. `select_candidates_ctx(db, &group, &model, Some(&ctx))`(`router/candidates.rs:45`)→ 候选平台。
6. `convert_request(&chat_req, &wire_proto, &platform_proto)`(`adapter/converter/request.rs:12`)→ 出站 body + path。
7. 上游发请求用 `reqwest`(复用 `http_client.rs` 共享 client)。
8. 计费走 `calc_est_cost` / `resolve_price`(见第 5 节)。

**关键: P2 解密后等同于把现有 handle_proxy 链路重跑一遍**,只是入站 IO 从 axum Request 换成 MITM 解出的 (method, path, headers, body)。可考虑把 handler.rs 的核心逻辑(`handle_proxy_core` 从 line 76 的 body 读取之后)抽成 `process_inbound_http(state, method, uri, headers, bytes, request_id)` 通用函数,MITM 和现有 axum fallback 都调它。**P2 实现时这是主要重构点**。

### 平台 base_url host 提取复用点 (file:line)
- `endpoint_host()` — `src-tauri/src/gateway/proxy/endpoint.rs:72-92`(已存在,直接复用)
- `Platform.base_url` — `src-tauri/src/gateway/models/platform.rs:161`
- `Platform.api_key` — `models/platform.rs:162`(出站 key,P1 不可见,P2 解密后若需验证可读但通常用 Group.group_key 路由)
- `Platform.endpoints[].base_url` — `models/platform.rs:142`
- `Group.group_key` — `models/group.rs:13`(入站鉴权 token)
- `resolve_group(db, token)` — `endpoint.rs:148`

---

## 5. proxy_log 字段映射 + 写入站点

### 结论
P1 元数据写入: **新建专用轻量函数 `upsert_connect_log`**,不走 handler.rs 的 `upsert_log`(那条路径假设有 ChatRequest / body,CONNECT 无 body 会 panic / 走错分支)。但底层 DB insert 复用现有 `insert_proxy_log_columns`(`db/proxy_log.rs:216`)。

### proxy_log schema 现状 (校正任务描述)
任务描述说 `group_name=''`,但**实际列名是 `group_key`**(2024 年已 rename,见 `db/schema_early.rs:78` 建表 `group_name`,但 `proxy_log.rs:6` 的 `PROXY_LOG_COLUMNS` 常量列名是 `group_key`,且 `models/proxy_log.rs:9` struct 字段也是 `group_key`)。**这是 schema 列名与代码常量不一致的历史包袱** — `schema_early.rs:78` 建表用 `group_name`,但所有读写 SQL 用 `group_key`。推测有后续 migration rename(未在 schema_early 内,可能在 schema_late.rs)。`需要:` 实现前 grep `ALTER TABLE proxy_log RENAME COLUMN group_name` 确认列实际名,或直接信任 `PROXY_LOG_COLUMNS` 常量(它是写入 source of truth)。

### P1 字段映射 (新增 upsert_connect_log)
| proxy_log 列 | P1 取值 | 说明 |
|---|---|---|
| `id` | `uuid v4 simple` | 同 handler.rs:10 模式 |
| `group_key` | `''`(空) | P1 无 apikey 无法路由分组 |
| `model` / `actual_model` | `''` | 隧道不解密,无 model |
| `source_protocol` | `"http-connect"` | 新协议标识,便于 Logs 页区分 |
| `target_protocol` | `""` | 无转换 |
| `platform_id` | `match_platform_by_host()` 或 `0` | host 命中则填,否则 0 |
| `request_url` | CONNECT target (`api.anthropic.com:443`) | 原样存 |
| `request_headers` / `request_body` / `upstream_*` | `''` | 隧道无可见 body(P1 不解密) |
| `status_code` | `200` / `502` / `499` | 隧道建立成功 / 上游连不上 / 客户端断 |
| `upstream_status_code` | `0` | 不发上游 HTTP,无 |
| `duration_ms` | `start.elapsed().as_millis()` | 隧道全程时长 |
| `input_tokens` / `output_tokens` / `cache_tokens` | `0` | 不计费 |
| `est_cost` | `0.0` | 不计费 |
| `is_stream` | `false` | 非 SSE |
| `attempts` | `[]` / `[single]` | 可记一条隧道尝试 |
| `created_at` / `updated_at` | `db::now()` | 同现有 |

> **字节统计无对应列**: proxy_log schema 无 `bytes_up`/`bytes_down` 字段。P1 若要展示字节流,有两条路:
> 1. **零 schema 改动**: 把字节塞进 `attempts` JSON 或 `blocked_reason`(hack,不推荐)。
> 2. **加 2 列**(migration 021+): `bytes_up INTEGER DEFAULT 0, bytes_down INTEGER DEFAULT 0`。任务描述说「DB 零改动」,但字节统计确实是 P1 的核心展示数据。`需要:` 与 main 确认 P1 是否真要展示字节流(若仅记 host/status/duration 则真零改动)。

### 写入站点
- **现有 insert 入口**: `db/proxy_log.rs:216 insert_proxy_log_columns(db, cols: ProxyLogColumns)` — 首节点 INSERT。`update_proxy_log_columns(db, new, prev)`(`:242`)— 后续 UPDATE 变化列。
- **handler 调用层**: `proxy/log.rs:16 upsert_log(state, log, settings)` — 现有渐进式日志的总入口(含 settings 脱敏 + stats_agg 聚合)。CONNECT P1 **不应走这个**(会触发 stats_agg 把 0 token 请求计入今日统计,污染)。
- **建议**: 在 `proxy/log.rs` 新增 `upsert_connect_log(state, id, target, platform_id, status, duration_ms, bytes_up, bytes_down)`:
  - 不调 `agg_mark_first`(避免污染 stats)。
  - 不调 `calc_est_cost`(0 token,无意义)。
  - 直接构造 `ProxyLogColumns`(全空 body / 0 token)+ `insert_proxy_log_columns` 落一行。

### P2 计费复用点 (解密后填 actual_model/tokens/est_cost)
P2 解密走通 AI 解析后,计费链:
- `crate::gateway::db::resolve_price(db, model, platform_type, in_tok, out_tok, cache_tok)` — `src-tauri/src/gateway/db/model_price.rs:179`(单一价格源,含 LiteLLM GitHub JSON + manual 回退)。
- `crate::gateway::db::calc_est_cost(db, model, platform_type, in_tok, out_tok, cache_tok)` — `src-tauri/src/gateway/db/stats_today.rs:184`(包了 resolve_price,直接返回 $)。
- 调用样本见 `proxy/log.rs:45-53`(`upsert_log` 内 first_agg 分支)和 `proxy/log.rs:96-115`(cols.est_cost 回退)— P2 解密拿到 usage 后**直接复用这两段代码**。

---

## 风险 / 未知

### 已确认可解
1. axum 0.8 + hyper 1.x 原生支持 CONNECT,`axum::serve` 默认开 upgrade,无需 feature。
2. rustls/tokio-rustls/rcgen 全部已可用(rcgen 需新增依赖,版本 0.14)。
3. `endpoint_host` host 提取可直接复用。
4. hyper-util auto 包装需 `downcast::<TcpStream>`(已知坑点)。

### 需要确认 (标 `需要:`)
1. **schema 列名 group_name vs group_key**: `schema_early.rs:78` 写 `group_name`,但 `PROXY_LOG_COLUMNS` 常量是 `group_key`。需 grep migration 确认实际列名(或直接信任常量,因为 INSERT SQL 用的是常量列名,若 DB 是 `group_name` 则 INSERT 会报 no such column — 但现网没报,说明实际是 `group_key`,推测 schema_late 有 rename migration)。**实现前必须确认**,否则 P1 写入会失败。
2. **P1 字节统计列**: 任务说「DB 零改动」但又要「记字节/时长」,字节无列。要么放宽到加 2 列 migration,要么放弃字节展示(仅 host/status/duration)。
3. **rcgen 0.14 签 leaf 的确切 API 链**: `CertificateParams::signed_by` 还是 `serialize_request().self_signed()` 在 0.14.8 的具体签名需实测(`lib.rs:9` 注释提到两个方法,未逐行验证)。`cargo doc -p rcgen` 核对。
4. **rustls feature 统一**: 加 `rustls = { ..., features = ["ring"] }` 后,跑 `cargo tree -e features -p rustls` 确认最终 feature 集是 `ring`(而非被 reqwest 间接统一成 `aws_lc_rs`)。若被统一成 aws_lc_rs,rcgen 签出的 ring key 会不被接受,需把 rcgen 也切 `aws_lc_rs` feature(但会引入 C 编译依赖,CI 计费分钟增加)。
5. **CONNECT fallback 挂载顺序**: axum Router 的 `.fallback(handle_proxy)` 与显式 CONNECT handler 的优先级需实测。理论上显式路由优先,但若 CONNECT target 形如 `host:port` 不匹配任何 `.route()` pattern,可能落到 fallback。建议用 `MethodRouter::new().fallback(handle_proxy)` + 在 fallback 内对 `Method::CONNECT` 分流,而非依赖 axum 路由匹配 CONNECT URI(axum 的 path matcher 对 `host:port` 这种 authority-form URI 可能不解析为常规 path)。
6. **HTTP/2 CONNECT**: axum::serve 默认 auto 检测 h1/h2,h2 的 CONNECT 语义不同(RFC 8441,extended CONNECT,需 `:protocol` pseudo-header)。客户端若走 h2 proxy,当前 axum::serve 的 `http2().enable_connect_protocol()`(`serve/mod.rs:394`)已开,但 MITM 隧道场景客户端几乎都是 h1(curl/系统 proxy 走 h1),h2 CONNECT 边界情况 P1 可先不支持,遇到返回 501。
