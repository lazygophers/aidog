//! P3 ST3 按 SNI 动态签 host 证书（缓存）。
//!
//! 职责：
//!  - 包装 ST1 `ca::sign_host_cert`，按 host（SNI / CONNECT target host）签 leaf 证书
//!  - 把 rcgen PEM 产物转为 rustls `CertifiedKey`（DER + ECDSA P256 signer）
//!  - 缓存已签证书（`Mutex<HashMap<host, CachedCert>>`），同 host 二次命中复用
//!
//! 设计依据：design.md §3（TLS MITM 层）。
//!
//! ponytail: 缓存加容量上限 + TTL 惰性 sweep（抄 `proxy/devin.rs:50` `len>N` retain idiom，
//! 见 `CERT_CACHE_MAX` 注释推导）。原设计假设白名单 host 集合 n<20 故免加界；改为兜底防
//! 用户自定义平台增长 / host 抖动导致无界堆积。evict 只影响该 host **下一次**签名（走
//! `sign_host_cert` 重签），已建立的 TLS 连接持有自己的 `CertifiedKey` 副本不受影响。

use std::sync::{Arc, Mutex};
use std::time::Instant;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::sign::CertifiedKey;

use super::ca::{RootCa, sign_host_cert};

/// cert_signer 错（ST1 SignError 包装 + ST3 自有的 PEM / rustls 错）。
///
/// ponytail: ST1 `ca::SignError` 只有 Rcgen 变体；ST3 在 PEM→DER 解析 + ECDSA signer
/// 构造时引入新错因。不在 ca.rs 改 SignError（ST1 产物稳定），本模块独立 enum 包 Rcgen。
#[derive(Debug)]
pub enum CertSignError {
    /// rcgen 签证书错（透传 ST1 `ca::sign_host_cert`）。
    Rcgen(rcgen::Error),
    /// PEM 解析 / rustls signer 构造错（不可恢复，CA 数据损坏，调用方 fallback 盲转）。
    Other(String),
}

impl std::fmt::Display for CertSignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CertSignError::Rcgen(e) => write!(f, "rcgen: {e}"),
            CertSignError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for CertSignError {}

impl From<rcgen::Error> for CertSignError {
    fn from(e: rcgen::Error) -> Self {
        CertSignError::Rcgen(e)
    }
}

impl From<super::ca::SignError> for CertSignError {
    fn from(e: super::ca::SignError) -> Self {
        match e {
            super::ca::SignError::Rcgen(rg) => CertSignError::Rcgen(rg),
        }
    }
}

/// 缓存条目上限 —— 超过即触发惰性 sweep（抄 `proxy/devin.rs:50` `len>N` retain idiom）。
///
/// 依据（单条字节 × 合理并发量）：单条 `CachedCert` ≈ leaf 证书 DER（ECDSA P256，约
/// 1.2KB）+ ECDSA signer（约 0.5KB）+ HashMap host key（域名均长约 30B + String 开销）
/// ≈ 2KB/条。设计注释原假设白名单 host 集合 n<20；256 条 ≈ 512KB 留 12x 冗余覆盖用户
/// 自定义平台增长，同时封顶防止 host 抖动等异常场景无界堆积。
const CERT_CACHE_MAX: usize = 256;

/// 缓存条目 TTL（供 sweep 判定过期，非签名有效期）—— 超过 `CERT_CACHE_MAX` 时只清过期项。
const CERT_CACHE_ENTRY_TTL_SECS: u64 = 3600;

/// 缓存值：CertifiedKey + 签发时刻（sweep 判过期用）。
#[derive(Clone, Debug)]
struct CachedCert {
    key: Arc<CertifiedKey>,
    signed_at: Instant,
}

/// host → rustls CertifiedKey 缓存（线程安全，签一次复用）。
///
/// ponytail: 全局 Mutex 而非 per-host 锁 —— 签证书是冷启动 + 偶发操作（首次见 host
/// 才签，之后命中缓存读不加锁路径外），全局锁竞争可忽略。若签名成为热路径瓶颈，拆
/// `DashMap<String, Arc<CertifiedKey>>`。
#[derive(Debug)]
pub struct CertSigner {
    ca: RootCa,
    cache: Mutex<std::collections::HashMap<String, CachedCert>>,
}

impl CertSigner {
    /// 由 RootCa 构造（CA 来自 ST1 `ensure_root_ca` / `load_root_ca`）。
    pub fn new(ca: RootCa) -> Self {
        Self {
            ca,
            cache: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// 按 host 签 / 取缓存的 CertifiedKey。
    ///
    /// 二次同 host 命中缓存（不重签，不重算 ECDSA signer）。首次签失败返 `CertSignError`，
    /// 不写入缓存（下次调用会重试）。缓存满 `CERT_CACHE_MAX` 时惰性清过期项（不影响已建立
    /// 的 TLS 连接 —— 它们持有自己的 `CertifiedKey` 副本，evict 只影响该 host 下次签名）。
    pub fn certified_key_for(&self, host: &str) -> Result<Arc<CertifiedKey>, CertSignError> {
        // ponytail: 双检 —— 先锁读，未命中再签 + 锁写。锁内签名（cold path，可接受）；
        // 若签名耗时 >100ms 影响首字节延迟，改 `entry().or_try_insert_with` 异步签。
        if let Some(cached) = self.cache.lock().unwrap().get(host).cloned() {
            return Ok(cached.key);
        }
        let signed = sign_host_cert(&self.ca, host)?;
        let ck = Arc::new(build_certified_key(
            signed.cert_pem,
            signed.private_key_pem,
        )?);
        let now = Instant::now();
        {
            let mut guard = self.cache.lock().unwrap();
            // 抄 devin.rs:50 idiom：超阈值才清一次过期项，避免长跑无界堆积。
            if guard.len() > CERT_CACHE_MAX {
                guard.retain(|_, v| {
                    now.duration_since(v.signed_at).as_secs() <= CERT_CACHE_ENTRY_TTL_SECS
                });
            }
            guard.insert(
                host.to_string(),
                CachedCert {
                    key: ck.clone(),
                    signed_at: now,
                },
            );
        }
        Ok(ck)
    }

    /// 测试用：返回缓存条数（验收 `cert_signer_cache` 用）。
    #[cfg(test)]
    pub fn cache_len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }
}

/// 从 PEM（cert + private key）构造 rustls CertifiedKey。
///
/// - cert_pem → DER chain（单 leaf，无中间证书；假 CA 自签，client 信任 CA 即可验链）
/// - private_key_pem → rustls ring ECDSA signer（any_ecdsa_type 自适应 P256/P384）
///
/// ponytail: 不附加 CA 证书到 chain —— 客户端若装了 ST1 假 CA，自己拿 CA + leaf 验链；
/// 若未装 CA，附加 CA 也无济于事（链根不受信）。省一次 PEM 解析 + chain 增长。
fn build_certified_key(
    cert_pem: String,
    private_key_pem: String,
) -> Result<CertifiedKey, CertSignError> {
    let cert_chain = parse_cert_chain_pem(&cert_pem)?;
    let key_der = parse_private_key_pem(&private_key_pem)?;
    // rustls 0.23 ring provider 的 ECDSA 加载器（P256/P384 自适应）。
    let key = rustls::crypto::ring::sign::any_ecdsa_type(&key_der)
        .map_err(|e| CertSignError::Other(e.to_string()))?;
    Ok(CertifiedKey::new(cert_chain, key))
}

/// PEM → DER 证书链（可能含多段 BEGIN CERTIFICATE；取全部 X509Certificate item）。
fn parse_cert_chain_pem(cert_pem: &str) -> Result<Vec<CertificateDer<'static>>, CertSignError> {
    use rustls_pemfile::Item;
    let mut chain = Vec::new();
    let mut cursor = std::io::Cursor::new(cert_pem.as_bytes());
    loop {
        let item = rustls_pemfile::read_one(&mut cursor)
            .map_err(|e| CertSignError::Other(format!("pem parse: {e}")))?;
        match item {
            Some(Item::X509Certificate(der)) => chain.push(der),
            Some(_) => continue, // 跳过非证书段（PEM 内偶发的其它 item）
            None => break,
        }
    }
    if chain.is_empty() {
        return Err(CertSignError::Other(
            "cert_pem has no X509Certificate".into(),
        ));
    }
    Ok(chain)
}

/// PEM 私钥 → DER（rustls-pemfile 把 PKCS#8 / PKCS#1 / SEC1 统一成 PrivateKeyDer）。
///
/// ponytail: 三种 Private*KeyDer 都是 PrivateKeyDer 的子类型构造器，各自 Into<PrivateKeyDer>。
/// rustls-pemfile 返回具名 newtype（PrivatePkcs8KeyDer 等），不能在同一 match arm 绑定
/// （类型不同）。逐 arm 提升为统一 PrivateKeyDer。
fn parse_private_key_pem(private_key_pem: &str) -> Result<PrivateKeyDer<'static>, CertSignError> {
    use rustls_pemfile::Item;
    let mut cursor = std::io::Cursor::new(private_key_pem.as_bytes());
    loop {
        let item = rustls_pemfile::read_one(&mut cursor)
            .map_err(|e| CertSignError::Other(format!("pem parse: {e}")))?;
        let key: PrivateKeyDer<'static> = match item {
            Some(Item::Pkcs8Key(der)) => der.into(),
            Some(Item::Pkcs1Key(der)) => der.into(),
            Some(Item::Sec1Key(der)) => der.into(),
            Some(_) => continue,
            None => break,
        };
        return Ok(key);
    }
    Err(CertSignError::Other(
        "private_key_pem has no recognizable key".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca::{RootCa, generate_root_ca};

    /// 测试用 RootCa：从 rcgen 产物直接构造（ca.rs 的 `RootCa::new` 是私有关联函数，
    /// 跨模块不可达；这里用公开字段构造等价结构）。
    ///
    /// ponytail: fingerprint 留空串（仅装/卸信任库时定位用，TLS 握手测试不需要）。
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

    /// ST3 验收：同 host 二次签名命中缓存（cache_len 不增）。
    #[test]
    fn cert_signer_cache() {
        let signer = CertSigner::new(test_ca());
        let ck1 = signer
            .certified_key_for("api.anthropic.com")
            .expect("first sign");
        let ck2 = signer
            .certified_key_for("api.anthropic.com")
            .expect("second sign (cached)");
        assert_eq!(signer.cache_len(), 1, "same host should hit cache");
        assert!(
            Arc::ptr_eq(&ck1, &ck2),
            "cached CertifiedKey should be the same Arc"
        );
    }

    /// ST3 验收：不同 host 各签一次（cache_len = 2）。
    #[test]
    fn cert_signer_cache_distinct_hosts() {
        let signer = CertSigner::new(test_ca());
        signer
            .certified_key_for("api.anthropic.com")
            .expect("sign anthropic");
        signer
            .certified_key_for("api.openai.com")
            .expect("sign openai");
        assert_eq!(signer.cache_len(), 2);
    }

    /// ST3 验收：签出的 CertifiedKey 非空 + cert chain 含至少 1 张证书。
    #[test]
    fn cert_signer_certified_key_nonempty() {
        let signer = CertSigner::new(test_ca());
        let ck = signer.certified_key_for("api.anthropic.com").expect("sign");
        assert!(!ck.cert.is_empty(), "cert chain must be non-empty");
        // end-entity cert DER 非空。
        assert!(!ck.cert[0].as_ref().is_empty());
    }
}
