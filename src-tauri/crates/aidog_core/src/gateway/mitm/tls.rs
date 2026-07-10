//! P3 ST3 TLS MITM 层。
//!
//! 职责：
//!  - `accept_client`: tokio-rustls TLS server，按 ClientHello SNI 动态签 host 证书
//!    （经 `CertSigner`），accept 客户端 TLS 握手，返回明文 `TlsStream`
//!  - `connect_upstream`: tokio-rustls TLS client，系统/Mozilla root store 验证上游
//!    真证书，握手失败（疑似 cert pinning）→ 返 `PinningSuspect`，调用方降级 P1 盲转
//!  - ALPN（D9）：server 段同时 advertise `h2` + `http/1.1`；client 段同（按上游响应协商）
//!
//! 设计依据：design.md §3、`.trellis/spec/backend/proxy-connect-relay.md`。
//!
//! ponytail: `accept_client` 用单个 `ServerConfig` + 自定义 `ResolvesServerCert`
//! （从 ClientHello.sni 取 host → CertSigner 查 / 签缓存）。比 `LazyConfigAcceptor`
//! （先读 SNI 再选 config）少一次 config 构建，标准 MITM 模式。

use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::ServerConfig;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::{client::TlsStream as ClientTlsStream, server::TlsStream as ServerTlsStream};

use super::cert_signer::CertSigner;

/// ALPN 协议（D9：两段独立协商）。
///
/// ponytail: server 段同时 advertise h2 + http/1.1，rustls 选客户端也支持的第一个。
/// 顺序：h2 优先（性能），降级 http/1.1（兼容）。Anthropic SDK 实测会选 h2，但若协商
/// 失败，ST6 会再细化（按上游实际协议强制协商）。
const ALPN_H2: &[u8] = b"h2";
const ALPN_HTTP1_1: &[u8] = b"http/1.1";

/// ServerConfig ALPN 顺序（h2 优先，http/1.1 兜底）。
const SERVER_ALPN: &[&[u8]] = &[ALPN_H2, ALPN_HTTP1_1];

// ─── accept_client（client ↔ AirDog 段，假证书）─────────────────────────────

/// 把 `CertSigner` 包装为 `ResolvesServerCert`，供 rustls ServerConfig 用。
///
/// `resolve()` 在 TLS 握手期间被调（client_hello.sni() 给出 host），返回对应 host
/// 的 CertifiedKey。SNI 缺失（老客户端 / 工具）→ fallback 用 `default_host`（CONNECT
/// target host，由 `accept_client` 调用方传入）。
#[derive(Debug)]
struct SnCertResolver {
    signer: Arc<CertSigner>,
    /// SNI 缺失时的 fallback host（CONNECT target host 段）。
    default_host: String,
}

impl ResolvesServerCert for SnCertResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let host = client_hello.server_name().unwrap_or(&self.default_host);
        match self.signer.certified_key_for(host) {
            Ok(ck) => {
                tracing::debug!(host, "mitm tls: resolved cert by SNI");
                Some(ck)
            }
            Err(e) => {
                // 签证书失败（CA 数据损坏等不可恢复错）→ abort handshake（返 None）。
                // 调用方（ST4 connect.rs）会从握手失败降级 P1 盲转。
                tracing::warn!(host, error = %e, "mitm tls: cert sign failed, aborting handshake");
                None
            }
        }
    }
}

/// accept 客户端 TLS 握手（client ↔ AirDog 段）。
///
/// - `stream`: 已建立的 TCP/upgrade 流（CONNECT 后的 `TokioIo<TcpStream>` 或 Upgraded）
/// - `sni_fallback`: CONNECT target host（SNI 缺失时的兜底签证书 host）
///
/// 返回明文 `TlsStream`（rustls server 端，用假 CA 签的 leaf 证书）。
/// 握手失败（client 不信任 CA / 网络断）→ `io::Error`，调用方（ST4）降级 P1 盲转 + 告警。
///
/// ponytail: 每次调用新建 ServerConfig —— config 构造廉价（无 IO），且每连接的
/// `default_host`（SNI fallback）不同。若 profiling 显示 config 构造成为瓶颈，
/// 按 `default_host` 缓存 `Arc<ServerConfig>`（host 集合受白名单限制，缓存命中率高）。
pub async fn accept_client<IO>(
    signer: Arc<CertSigner>,
    stream: IO,
    sni_fallback: String,
) -> Result<ServerTlsStream<IO>, std::io::Error>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    let resolver = Arc::new(SnCertResolver {
        signer,
        default_host: sni_fallback,
    });
    // ponytail: builder() 用 default CryptoProvider（rustls 0.23 ring process-default）。
    // ST1 Cargo.toml 已 features=["ring",...]，default provider 在 lib.rs setup 一次
    // `CryptoProvider::process_default()`（如未 process，builder() 内部 panic；本 subtask
    // 测试显式 install_default_provider 保证）。
    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(resolver);
    // ALPN（D9）：server 同时 advertise h2 + http/1.1。
    server_config.alpn_protocols = SERVER_ALPN.iter().map(|p| p.to_vec()).collect();

    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    acceptor.accept(stream).await
}

// ─── connect_upstream（AirDog ↔ upstream 段，真证书验证）──────────────────────

/// 上游 TLS 握手失败时的分类结果。
///
/// `PinningSuspect` 标记该 host 可能做 cert pinning（client 证书校验被上游拒绝），
/// 调用方（ST4 connect.rs）应把该 host 加入 pinning_suspect 集合，后续 CONNECT 降级
/// P1 盲转 + 告警日志（design §3 弱点 6）。
#[derive(Debug)]
pub enum UpstreamTlsOutcome {
    /// 握手成功，返回明文 `TlsStream`（rustls client 端，系统/Mozilla root 验证上游）。
    ///
    /// ponytail: Box 抑制 large_enum_variant（ClientTlsStream 含 rustls 会话状态，
    /// 远大于另两变体）。调用方拿到 Box 后正常 deref 使用；ST5 forward 接入时若需 owned
    /// stream，可 `*boxed` 移出。
    Connected(Box<ClientTlsStream<tokio::net::TcpStream>>),
    /// 疑似 cert pinning（或上游证书无效）：握手失败，但 TCP 通。
    /// 调用方降级盲转（不解密，原样转 TCP 字节）。
    PinningSuspect {
        host: String,
        error: String,
    },
    /// 其它 IO 错（TCP 断 / 超时），非 pinning 类。
    IoError(std::io::Error),
}

/// 连接上游并完成 TLS 握手（AirDog ↔ upstream 段）。
///
/// - `host`: CONNECT target host（用于 SNI + ServerName 验证）
/// - `stream`: 已建立的 TCP 流（连到上游 host:port）
///
/// 用 Mozilla 内置 root store（`webpki-roots`）验证上游证书 —— 无系统依赖，
/// 跨平台一致（design.md §3）。握手成功 → `Connected`；握手失败含证书错 → `PinningSuspect`；
/// 其它 IO 错 → `IoError`。
///
/// ponytail: ServerName 用 host 字面（非 IP）；CONNECT target 是域名场景占绝大多数。
/// IP literal 上游（极少见，AI 平台 base_url 全是域名）→ 当作 DNS name 尝试，
/// 失败走 IoError 分类，调用方降级盲转即可。
pub async fn connect_upstream(
    host: &str,
    stream: tokio::net::TcpStream,
) -> UpstreamTlsOutcome {
    // Mozilla 内置 root store（webpki-roots，无系统依赖）。
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let mut client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    // ALPN（D9）：client 段同 advertise，让 rustls 按上游响应协商。
    client_config.alpn_protocols = SERVER_ALPN.iter().map(|p| p.to_vec()).collect();

    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));

    let server_name = match ServerName::try_from(host.to_string()) {
        Ok(n) => n,
        Err(e) => {
            // host 不是合法 DNS name（IP / 空 / 非法字符）→ 当 IO 错，调用方降级。
            return UpstreamTlsOutcome::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid ServerName {host:?}: {e}"),
            ));
        }
    };

    match connector.connect(server_name, stream).await {
        Ok(tls_stream) => UpstreamTlsOutcome::Connected(Box::new(tls_stream)),
        Err(e) => {
            // 分类：证书验证类错（疑似 pinning / 上游 cert 无效）→ PinningSuspect；
            // 纯 IO 错（TCP 断）→ IoError。
            if is_cert_validation_error(&e) {
                tracing::warn!(
                    host,
                    error = %e,
                    "mitm tls: upstream handshake failed (cert validation), flagging pinning suspect"
                );
                UpstreamTlsOutcome::PinningSuspect {
                    host: host.to_string(),
                    error: e.to_string(),
                }
            } else {
                tracing::warn!(host, error = %e, "mitm tls: upstream handshake IO error");
                UpstreamTlsOutcome::IoError(e)
            }
        }
    }
}

/// 判定 rustls 错是否证书验证类（疑似 cert pinning / 上游 cert 无效）。
///
/// ponytail: 用 error.to_string() contains 粗判 —— rustls 0.23 的 Error enum 有几十
/// 变体，逐个 match 维护成本高且版本敏感。证书错的关键词（InvalidCertificate /
/// UnknownIssuer / BadCertificate / NotValidForName / CertValidation）覆盖主要变体；
/// 未覆盖的少数变体 fallback 到 IoError（调用方仍降级盲转，行为正确，仅告警分类弱）。
fn is_cert_validation_error(e: &std::io::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("invalidcertificate")
        || msg.contains("unknownissuer")
        || msg.contains("badcertificate")
        || msg.contains("notvalidforname")
        || msg.contains("certvalidation")
        || msg.contains("certificate")
        && (msg.contains("invalid") || msg.contains("untrusted") || msg.contains("expired"))
}

// ─── ResolvesServerCert Debug via CertSigner not needed; derive fails on dyn ─
//
// 注意：SnCertResolver derive(Debug) 要求 CertSigner: Debug。CertSigner 含
// `Mutex<HashMap<String, Arc<CertifiedKey>>>` —— CertifiedKey 自身实现 Debug。
// rcgen::Error / RootCa 均 Debug，故 CertSigner 可加 #[derive(Debug)]，本文件已生效。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::mitm::ca::{generate_root_ca, RootCa};
    use crate::gateway::mitm::cert_signer::CertSigner;

    /// 测试用 RootCa（字段直接构造，fingerprint 留空）。
    fn test_ca() -> RootCa {
        let (key_pair, cert) = generate_root_ca().expect("generate_root_ca");
        RootCa {
            private_key_pem: key_pair.serialize_pem(),
            cert_pem: cert.pem(),
            fingerprint: String::new(),
            created_at: 0,
            enabled: true,
            ca_installed: false,
        }
    }

    /// ST3 验收：mock client（信任 ST1 假 CA）→ accept_client 握手成功，能读写明文。
    ///
    /// 流程：
    ///  1. 生成假 CA（rcgen）
    ///  2. 构造信任该 CA 的 rustls ClientConfig
    ///  3. in-memory 双向管道（tokio duplex）连 client / server 两端
    ///  4. server 端 accept_client（CertSigner 用假 CA）
    ///  5. client 端 connect（SNI=api.anthropic.com）→ 握手成功 → 双向读写明文 echo
    #[tokio::test]
    async fn tls_handshake() {
        // rustls ring provider 显式 install（测试进程可能未 process_default）。
        let _ = rustls::crypto::ring::default_provider().install_default();

        // 1. 假 CA
        let ca = test_ca();
        let signer = Arc::new(CertSigner::new(ca.clone()));

        // 2. client config 信任假 CA
        let mut root_store = rustls::RootCertStore::empty();
        let ca_certs = parse_cert_chain_pem_for_test(&ca.cert_pem);
        for c in ca_certs {
            root_store.add(c).expect("add CA to root store");
        }
        let client_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        // 3. duplex 管道
        let (client_io, server_io) = tokio::io::duplex(8 * 1024);

        // 4. server accept
        let signer_clone = signer.clone();
        let server_task = tokio::spawn(async move {
            accept_client(signer_clone, server_io, "api.anthropic.com".to_string()).await
        });

        // 5. client connect（连到 server 端的 duplex 另一半，模拟隧道）
        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));
        let server_name = ServerName::try_from("api.anthropic.com".to_string()).unwrap();
        let client_task = tokio::spawn(async move {
            connector.connect(server_name, client_io).await
        });

        let mut server_stream = server_task
            .await
            .expect("server task join")
            .expect("server accept handshake");
        let mut client_stream = client_task
            .await
            .expect("client task join")
            .expect("client connect handshake");

        // 双向 echo：client 写 → server 读 → server 写回 → client 读
        tokio::io::AsyncWriteExt::write_all(&mut client_stream, b"hello mitm")
            .await
            .expect("client write");
        let mut buf = [0u8; 10];
        tokio::io::AsyncReadExt::read_exact(&mut server_stream, &mut buf)
            .await
            .expect("server read");
        assert_eq!(&buf, b"hello mitm");
    }

    /// 解析 PEM 证书链为 DER vec（测试辅助；复用 cert_signer 逻辑会循环依赖，本地拷贝）。
    fn parse_cert_chain_pem_for_test(
        cert_pem: &str,
    ) -> Vec<rustls::pki_types::CertificateDer<'static>> {
        use rustls_pemfile::Item;
        let mut chain = Vec::new();
        let mut cursor = std::io::Cursor::new(cert_pem.as_bytes());
        loop {
            let item = rustls_pemfile::read_one(&mut cursor).expect("pem parse");
            match item {
                Some(Item::X509Certificate(der)) => chain.push(der),
                Some(_) => continue,
                None => break,
            }
        }
        chain
    }

    /// cert 错分类：含 "certificate" + "invalid" → PinningSuspect。
    #[test]
    fn pinning_error_classification_cert_invalid() {
        let e = std::io::Error::other("invalidcertificate: untrusted root");
        assert!(is_cert_validation_error(&e));
    }

    /// cert 错分类：纯 IO 错（"connection reset"）→ 非 pinning。
    #[test]
    fn pinning_error_classification_plain_io() {
        let e = std::io::Error::from(std::io::ErrorKind::ConnectionReset);
        assert!(!is_cert_validation_error(&e));
    }
}
