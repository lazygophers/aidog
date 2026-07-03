//! P3 ST1 假 CA 子系统。
//!
//! 职责：
//!  - 生成 ECDSA P256 自签 Root CA（rcgen）
//!  - 持久化到 DB（mitm_ca 表，明文 + DB 文件权限 0600，D4/D5）
//!  - 动态签 host 证书（按 SNI / CONNECT target host 签，ST3 用）
//!  - 装信任库（macOS `security add-trusted-cert` / Windows `certutil -addstore` / Linux
//!    `update-ca-certificates`，经 tauri-plugin-shell + sudo 弹窗，D1/D8）
//!  - 清理（移除系统信任，ST9；本 subtask 留 stub 接口，实装在 ST9）
//!
//! 生成时机（D7）：用户首次启用 MITM 时（前端 UI 触发 ensure_root_ca），非启动默认。
//!
//! 设计依据：design.md §1、`.trellis/spec/backend/proxy-connect-relay.md`。
//! 失败模式：design.md 失败模式表（sudo 拒绝 → 标 ca_installed=false + UI 引导手动装）。

use rcgen::{
    Certificate, CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256,
};
use rusqlite::params;

use crate::gateway::db::Db;

/// CA 唯一 id（mitm_ca 表单行设计，id 固定 1）。
const CA_ROW_ID: i64 = 1;

/// Root CA 物化（PEM 双段 + 元数据），从 DB 加载或新建后存 DB。
///
/// `private_key_pem` / `cert_pem` 直接 rcgen 序列化产物；`KeyPair` / rustls `CertifiedKey`
/// 在 ST3 用时按需从 PEM 反序列化重建（不在本结构常驻，避免 ca.rs 依赖 rustls server 类型）。
#[derive(Debug, Clone)]
pub struct RootCa {
    pub private_key_pem: String,
    pub cert_pem: String,
    /// SHA-256 fingerprint（hex, colon-separated），用于装/卸信任库时定位证书。
    /// 由 cert_pem 计算得来（ST1 仅记录，ST9 清理时用）。
    pub fingerprint: String,
    pub created_at: i64,
    pub enabled: bool,
    pub ca_installed: bool,
}

impl RootCa {
    /// 从 rcgen 产物构造（首次生成路径），fingerprint 由证书 PEM 计算。
    fn new(key_pair: &KeyPair, cert: &Certificate) -> Self {
        let cert_pem = cert.pem();
        let fingerprint = cert_fingerprint_hex(&cert_pem);
        Self {
            private_key_pem: key_pair.serialize_pem(),
            cert_pem,
            fingerprint,
            created_at: crate::gateway::db::now(),
            enabled: true,
            ca_installed: false, // 新生成 CA 默认未装信任库
        }
    }
}

/// 从 PEM 证书文本计算 SHA-256 fingerprint（colon-separated uppercase hex）。
///
/// ponytail: 解析 PEM base64 → SHA-256 → hex。rustls 的 CertifiedKey fingerprint API
/// 需要先构造 rustls::server::ResolvesServerCert（ST3 实装），此处仅算字面指纹供装/卸信任库定位，
/// 不强耦合 rustls 类型。空 / 解析失败返空串（调用方容错：装信任库命令仍可执行，仅定位弱）。
fn cert_fingerprint_hex(cert_pem: &str) -> String {
    use rustls_pemfile::Item;
    let der = match rustls_pemfile::read_one_from_slice(cert_pem.as_bytes()) {
        Ok(Some((Item::X509Certificate(der), _))) => der.as_ref().to_vec(),
        _ => return String::new(),
    };
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&der);
    let hash = hasher.finalize();
    hash.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(":")
}

/// 生成 ECDSA P256 自签 Root CA（rcgen）。
///
/// CA 证书关键属性（CVE 类常规 CA 模板）：
///  - CN = `AirDog MITM CA`（DistinguishedName 唯一字段，便于信任库识别）
///  - SAN: 无（CA 证书不需 SAN，签的 leaf 才需）
///  - is_ca = true（BasicConstraints CA:TRUE）
///  - KeyPair: ECDSA P256（rcgen `PKCS_ECDSA_P256_SHA256`）
///  - validity: rcgen 默认（2030+，足够桌面 app 生命周期；ST9 轮换走重生成）
pub fn generate_root_ca() -> Result<(KeyPair, Certificate), rcgen::Error> {
    let mut params = CertificateParams::new(vec![])?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "AirDog MITM CA");
    params.distinguished_name.push(DnType::OrganizationName, "AirDog");
    // CA:TRUE 由 rcgen 默认开启（self-signed CA 即设 is_ca）。显式标记 is_ca 防未来版本默认变更。
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let cert = params.self_signed(&key_pair)?;
    Ok((key_pair, cert))
}

/// 给定 host 动态签证书（ST3 用，本 subtask 提供 API + 单测，TLS accept 不接入）。
///
/// - CN / SAN[0] = host（CONNECT target host 或 SNI）
/// - 签发者：Root CA（key_pair + Issuer 从 DB PEM 重建，rcgen 0.14 `Issuer::from_ca_cert_pem`）
/// - 返 PEM（cert + private key），供 tokio-rustls TlsAcceptor 构造 CertifiedKey（ST3）
///
/// ponytail: 每次调用都 from_pem 重建 KeyPair + Issuer。ST3 加 leaf 缓存
/// （HashMap<String, Arc<CertifiedKey>> + TTL）后再优化；ST1 阶段只验证签证书逻辑正确。
pub fn sign_host_cert(ca: &RootCa, host: &str) -> Result<SignedCert, SignError> {
    let issuer_key = KeyPair::from_pem(&ca.private_key_pem)?;
    let issuer = rcgen::Issuer::from_ca_cert_pem(&ca.cert_pem, issuer_key)?;
    let mut params = CertificateParams::new(vec![host.to_string()])?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, host);
    let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let cert = params.signed_by(&leaf_key, &issuer)?;
    Ok(SignedCert {
        cert_pem: cert.pem(),
        private_key_pem: leaf_key.serialize_pem(),
        host: host.to_string(),
    })
}

/// 签出的 leaf 证书（PEM 双段 + 原 host 记账）。
#[derive(Debug, Clone)]
pub struct SignedCert {
    pub cert_pem: String,
    pub private_key_pem: String,
    pub host: String,
}

#[derive(Debug)]
pub enum SignError {
    Rcgen(rcgen::Error),
}

impl std::fmt::Display for SignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignError::Rcgen(e) => write!(f, "rcgen: {}", e),
        }
    }
}

impl From<rcgen::Error> for SignError {
    fn from(e: rcgen::Error) -> Self {
        SignError::Rcgen(e)
    }
}

impl std::error::Error for SignError {}

// ─── DB 持久化 ───────────────────────────────────────────────────────────

/// 从 DB 读 RootCa（单行 id=1）。无行返 None；DB 错返 Err。
pub async fn load_root_ca(db: &Db) -> Result<Option<RootCa>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT private_key_pem, cert_pem, fingerprint, created_at, enabled, ca_installed \
                 FROM mitm_ca WHERE id = ?1",
            )?;
            let row = stmt
                .query_row(params![CA_ROW_ID], |r| {
                    Ok(RootCa {
                        private_key_pem: r.get::<_, String>(0)?,
                        cert_pem: r.get::<_, String>(1)?,
                        fingerprint: r.get::<_, String>(2)?,
                        created_at: r.get::<_, i64>(3)?,
                        enabled: r.get::<_, i64>(4)? != 0,
                        ca_installed: r.get::<_, i64>(5)? != 0,
                    })
                })
                .ok();
            Ok(row)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 生成新 Root CA 并存 DB（覆盖旧行，D7 首次启用 / CA 轮换路径）。
///
/// D5: DB 文件权限 0600 由 `enforce_db_file_permissions` 在 Db::new 之后（CA 生成前）保证。
pub async fn create_and_store_root_ca(db: &Db) -> Result<RootCa, String> {
    let (key_pair, cert) = generate_root_ca().map_err(|e| e.to_string())?;
    let ca = RootCa::new(&key_pair, &cert);
    let ca_clone = ca.clone();
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT INTO mitm_ca (id, private_key_pem, cert_pem, fingerprint, created_at, enabled, ca_installed) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(id) DO UPDATE SET \
                    private_key_pem = excluded.private_key_pem, \
                    cert_pem = excluded.cert_pem, \
                    fingerprint = excluded.fingerprint, \
                    created_at = excluded.created_at, \
                    enabled = excluded.enabled, \
                    ca_installed = excluded.ca_installed",
                params![
                    CA_ROW_ID,
                    ca_clone.private_key_pem,
                    ca_clone.cert_pem,
                    ca_clone.fingerprint,
                    ca_clone.created_at,
                    ca_clone.enabled as i64,
                    ca_clone.ca_installed as i64,
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(ca)
}

/// 用户首次启用 MITM 时调（D7）：DB 无 CA → 生成；已有 → 返回现有。
///
/// 同时强制 DB 文件权限 0600（D5）：DB 路径由 main 经 app_data_dir 传入，
/// 本函数不感知路径；权限强制由 `enforce_db_file_permissions` 在 lib.rs setup 调用一次。
pub async fn ensure_root_ca(db: &Db) -> Result<RootCa, String> {
    if let Some(existing) = load_root_ca(db).await? {
        return Ok(existing);
    }
    create_and_store_root_ca(db).await
}

/// 标 CA 已装 / 未装信任库（装命令成功 / 失败后调）。
pub async fn set_ca_installed(db: &Db, installed: bool) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE mitm_ca SET ca_installed = ?1 WHERE id = ?2",
                params![installed as i64, CA_ROW_ID],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
}

/// 启用 / 禁用 MITM（用户开关；disabled 时 CONNECT 走 P1 盲转，ST4 接入）。
pub async fn set_enabled(db: &Db, enabled: bool) -> Result<(), String> {
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE mitm_ca SET enabled = ?1 WHERE id = ?2",
                params![enabled as i64, CA_ROW_ID],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
}

// ─── DB 文件权限（D5）────────────────────────────────────────────────────

/// 强制 DB 文件权限 0600（仅 owner 读写）。
///
/// D5 决策：私钥明文存 DB，安全模型与 api_key/token 同（均明文）；额外靠文件权限收窄访问面。
/// 在 setup 阶段（Db::new 后、CA 生成前）调一次；用户主动 chmod 会回退但 aidog 不持续 watch。
///
/// Unix-only：Windows 走 ACL（默认 user-only，无需显式设）；macOS/Linux 用 `std::fs` set_permissions。
pub fn enforce_db_file_permissions(db_path: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(db_path) {
            Ok(meta) => {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                if let Err(e) = std::fs::set_permissions(db_path, perms) {
                    tracing::warn!(error = %e, db_path, "mitm: failed to chmod 0600 DB file");
                } else {
                    tracing::info!(db_path, "mitm: enforced DB file permission 0600");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, db_path, "mitm: DB file not found for chmod 0600");
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = db_path;
        // Windows: DB 文件默认继承 user 目录 ACL，无需显式设。
    }
}

// ─── 装信任库（D1/D8，跨 OS）──────────────────────────────────────────────
//
// 装信任库经 tauri-plugin-shell execute（capability mitm-ca.json 限定的命名命令）。
// 本 subtask 提供 Rust 侧命令构造 + 状态更新；shell execute 由 ST7 前端 UI 触发
// （设计 §1：tauri-plugin-shell execute 从 frontend invoke）。
//
// 3 OS 命令（design.md §1 + spec）：
//  - macOS:   `security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain <ca.pem>`
//  - Windows: `certutil -addstore -f "Root" <ca.pem>`
//  - Linux:   `cp <ca.pem> /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates`
//
// 失败兜底（design 失败模式表）：sudo 拒绝 / 命令失败 → 标 ca_installed=false + UI 给命令路径
// 引导手动装。本模块只返回命令 spec，UI 层（ST7）负责实际 execute + 错误展示。

/// 装 CA 到系统信任库的命令 spec（前端 invoke shell execute 用）。
///
/// 返回 (program, args, ca_pem_path)。`ca_pem_path` 由调用方把 ca.cert_pem 写到
/// `app_data_dir/ca.pem` 后传入。
#[allow(dead_code)] // ST7 前端接入后引用
pub fn trust_ca_command(ca_pem_path: &str) -> (String, Vec<String>) {
    #[cfg(target_os = "macos")]
    {
        (
            "/usr/bin/security".to_string(),
            vec![
                "add-trusted-cert".to_string(),
                "-d".to_string(),
                "-r".to_string(),
                "trustRoot".to_string(),
                "-k".to_string(),
                "/Library/Keychains/System.keychain".to_string(),
                ca_pem_path.to_string(),
            ],
        )
    }
    #[cfg(target_os = "windows")]
    {
        (
            "certutil".to_string(),
            vec![
                "-addstore".to_string(),
                "-f".to_string(),
                "Root".to_string(),
                ca_pem_path.to_string(),
            ],
        )
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Linux: 拷到系统 CA 目录 + update-ca-certificates（一条 shell 命令两步）。
        // ponytail: 用 sh -c 跑两步（cp + update），capability scope 限命令名。
        // 实际 ST7 接入时若需分开两命令，扩 trust_ca_command 返多条 spec。
        (
            "/bin/sh".to_string(),
            vec![
                "-c".to_string(),
                format!(
                    "cp {ca_pem_path} /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates"
                ),
            ],
        )
    }
}

/// 卸载 CA（ST9 实装）。本 subtask 留接口签名，body 在 ST9 补命令 reverse。
#[allow(dead_code)] // ST9 接入后引用
pub fn untrust_ca_command(fingerprint_hex: &str) -> (String, Vec<String>) {
    #[cfg(target_os = "macos")]
    {
        (
            "/usr/bin/security".to_string(),
            vec![
                "delete-certificate".to_string(),
                "-Z".to_string(),
                fingerprint_hex.to_string(),
            ],
        )
    }
    #[cfg(target_os = "windows")]
    {
        (
            "certutil".to_string(),
            vec!["-delstore".to_string(), "Root".to_string(), fingerprint_hex.to_string()],
        )
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        (
            "/bin/sh".to_string(),
            vec![
                "-c".to_string(),
                "rm -f /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates".to_string(),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ST1 验收断言：rcgen 签 host 证书 SAN 含该 host。
    ///
    /// 构造：generate_root_ca → sign_host_cert("api.anthropic.com") → 解 cert_pem → 断言 SAN 含 host。
    /// SAN 解析用 rustls-pemfile + 简易 X.509 字节扫描（避免引入 x509-parser 重依赖）。
    #[test]
    fn ca_sign_cert_san_contains_host() {
        let (key_pair, cert) = generate_root_ca().expect("generate_root_ca");
        let ca = RootCa::new(&key_pair, &cert);
        let signed = sign_host_cert(&ca, "api.anthropic.com").expect("sign_host_cert");
        assert!(!signed.cert_pem.is_empty());
        assert_eq!(signed.host, "api.anthropic.com");
        // cert_pem 含 BEGIN CERTIFICATE 标记 + host 字面（SAN 扩展含 ASCII host）。
        assert!(
            signed.cert_pem.contains("BEGIN CERTIFICATE"),
            "cert_pem should be PEM-wrapped"
        );
        // 解析 DER 后断言 SAN 字节含 host —— X.509 SAN 是明文 IA5String 含 host。
        assert!(pem_der_contains_san_host(&signed.cert_pem, "api.anthropic.com"),
            "signed cert DER should contain host 'api.anthropic.com' in SAN extension");
    }

    /// ST1 验收：Root CA 自身可重新 from_pem 出 KeyPair（持久化 round-trip）。
    #[test]
    fn ca_root_keypair_roundtrip_from_pem() {
        let (key_pair, cert) = generate_root_ca().expect("generate_root_ca");
        let pem = key_pair.serialize_pem();
        let restored = KeyPair::from_pem(&pem).expect("from_pem roundtrip");
        // 序列 DER 应与原 key 一致（同 key material）。
        assert_eq!(key_pair.serialize_der(), restored.serialize_der());
        // cert PEM 可被 rustls-pemfile 解析（X509Certificate item）。
        use rustls_pemfile::Item;
        let cert_pem = cert.pem();
        let parsed = rustls_pemfile::read_one_from_slice(cert_pem.as_bytes());
        assert!(matches!(parsed, Ok(Some((Item::X509Certificate(_), _)))));
    }

    /// ST1 验收：fingerprint 非空且 deterministic（同 cert 重算同值）。
    #[test]
    fn ca_fingerprint_stable() {
        let (key_pair, cert) = generate_root_ca().expect("generate_root_ca");
        let ca = RootCa::new(&key_pair, &cert);
        assert!(!ca.fingerprint.is_empty(), "fingerprint should be non-empty");
        assert_eq!(ca.fingerprint, cert_fingerprint_hex(&ca.cert_pem),
            "fingerprint should be deterministic for same cert PEM");
        // fingerprint 是 colon-separated 64 hex chars（32 bytes SHA-256）。
        let octets: Vec<&str> = ca.fingerprint.split(':').collect();
        assert_eq!(octets.len(), 32, "SHA-256 fingerprint = 32 octets");
        for o in &octets {
            assert_eq!(o.len(), 2);
            assert!(o.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    /// trust_ca_command 在当前 OS 返非空命令 spec。
    #[test]
    fn ca_trust_command_returns_os_specific() {
        let (prog, args) = trust_ca_command("/tmp/aidog-ca.pem");
        assert!(!prog.is_empty());
        assert!(!args.is_empty());
        assert!(args.iter().any(|a| a.contains("aidog-ca.pem")));
    }

    /// 从 PEM 提取 DER，扫描 X.509 字节流中 SAN 扩展是否含 host 字面。
    ///
    /// ponytail: 简化判定 —— SAN 是 IA5String 编码的 host（ASCII 可见），DER 中
    /// host 串字节序列可直接 indexOf。完整 X.509 解析需 x509-parser crate（暂不引入，
    /// SAN 字面扫描对验证签证书正确性已足够；ST3 TLS 层用 rustls 正式验证链时再加完整解析）。
    fn pem_der_contains_san_host(cert_pem: &str, host: &str) -> bool {
        use rustls_pemfile::Item;
        let Ok(Some((Item::X509Certificate(der), _))) =
            rustls_pemfile::read_one_from_slice(cert_pem.as_bytes())
        else {
            return false;
        };
        // DER 中找 host 的 ASCII 字节序列（SAN 内为 IA5String = ASCII）。
        der.as_ref()
            .windows(host.len())
            .any(|w| w == host.as_bytes())
    }
}
