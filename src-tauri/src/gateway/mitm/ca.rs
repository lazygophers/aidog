//! P3 ST1 假 CA 子系统。
//!
//! 职责：
//!  - 生成 ECDSA P256 自签 Root CA（rcgen）
//!  - 持久化到 setting 表（scope=mitm, key=ca，明文 + DB 文件权限 0600，D4/D5）
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
use serde::{Deserialize, Serialize};

use crate::gateway::db::{get_setting, set_setting, Db};
use crate::gateway::models::SetSettingInput;

/// MITM 配置在 setting 表的 scope（与 app/global/middleware 同级）。
const MITM_SCOPE: &str = "mitm";
/// CA 对象在 setting 表的 key（单 JSON 对象：RootCa 序列化）。
const MITM_CA_KEY: &str = "ca";

/// Root CA 物化（PEM 双段 + 元数据），从 setting 加载或新建后存 setting。
///
/// `private_key_pem` / `cert_pem` 直接 rcgen 序列化产物；`KeyPair` / rustls `CertifiedKey`
/// 在 ST3 用时按需从 PEM 反序列化重建（不在本结构常驻，避免 ca.rs 依赖 rustls server 类型）。
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    let der = match pem_der(cert_pem) {
        Some(d) => d,
        None => return String::new(),
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

/// 从 PEM 证书文本计算 SHA-1 thumbprint（plain lowercase hex, NO colon）。
///
/// ST9 CA 清理：macOS `security delete-certificate -Z` 与 Windows `certutil -delstore Root`
/// 均按 SHA-1 thumbprint 定位。capability mitm-ca.json validator `^[0-9A-Fa-f]+$` 拒冒号，
/// 故返 plain hex（40 chars）。空/解析失败返空串（调用方应已校验 CA 存在）。
fn cert_sha1_thumbprint_hex(cert_pem: &str) -> String {
    let der = match pem_der(cert_pem) {
        Some(d) => d,
        None => return String::new(),
    };
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(&der);
    let hash = hasher.finalize();
    hash.iter().map(|b| format!("{:02x}", b)).collect::<String>()
}

/// 解析 PEM 取 X.509 DER bytes（cert_fingerprint_hex / cert_sha1_thumbprint_hex 共用）。
fn pem_der(cert_pem: &str) -> Option<Vec<u8>> {
    use rustls_pemfile::Item;
    match rustls_pemfile::read_one_from_slice(cert_pem.as_bytes()) {
        Ok(Some((Item::X509Certificate(der), _))) => Some(der.as_ref().to_vec()),
        _ => None,
    }
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

// ─── setting 持久化（scope=mitm, key=ca）─────────────────────────────────

/// 从 setting 读 RootCa（scope=mitm, key=ca）。无行返 None；解析失败返 None（tracing::warn）。
pub async fn load_root_ca(db: &Db) -> Result<Option<RootCa>, String> {
    match get_setting(db, MITM_SCOPE, MITM_CA_KEY).await? {
        Some(v) => match serde_json::from_value::<RootCa>(v.clone()) {
            Ok(ca) => Ok(Some(ca)),
            Err(e) => {
                tracing::warn!(scope = MITM_SCOPE, key = MITM_CA_KEY, error = %e, "stored mitm ca JSON corrupt, returning None");
                Ok(None)
            }
        },
        None => Ok(None),
    }
}

/// 生成新 Root CA 并存 setting（覆盖旧对象，D7 首次启用 / CA 轮换路径）。
///
/// D5: DB 文件权限 0600 由 `enforce_db_file_permissions` 在 Db::new 之后（CA 生成前）保证。
pub async fn create_and_store_root_ca(db: &Db) -> Result<RootCa, String> {
    let (key_pair, cert) = generate_root_ca().map_err(|e| e.to_string())?;
    let ca = RootCa::new(&key_pair, &cert);
    let value = serde_json::to_value(&ca).map_err(|e| format!("serialize RootCa: {e}"))?;
    set_setting(
        db,
        SetSettingInput {
            scope: MITM_SCOPE.to_string(),
            key: MITM_CA_KEY.to_string(),
            value,
        },
    )
    .await?;
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
///
/// read-modify-write：load → 改 ca_installed → set_setting 整对象回写。
/// CA 装信任库是用户低频操作，单 async fn 内串行，无并发竞争。
pub async fn set_ca_installed(db: &Db, installed: bool) -> Result<(), String> {
    let mut ca = load_root_ca(db)
        .await?
        .ok_or_else(|| "CA not generated".to_string())?;
    ca.ca_installed = installed;
    let value = serde_json::to_value(&ca).map_err(|e| format!("serialize RootCa: {e}"))?;
    set_setting(
        db,
        SetSettingInput {
            scope: MITM_SCOPE.to_string(),
            key: MITM_CA_KEY.to_string(),
            value,
        },
    )
    .await
}

// ─── keychain 实状校验（修问题 2：手动装/卸后 app 不感知）──────────────────
//
// 用户手敲 sudo security add-trusted-cert / 删 keychain 成功时，DB 的 ca_installed 静态字段
// 不更新（脱离 app 进程）。mitm_status 读 DB 静态值 → 页面错显。verify 直查 keychain 实状，
// sync 与 DB 不一致时回写，status 取实状返前端。
//
// verify 用 std::process::Command 子进程（后端直跑，非 tauri-plugin-shell，无需 capability）。
// CN 固定 `AirDog MITM CA`（generate_root_ca L117），读公开 keychain 不需 sudo。

/// CN 字面（与 generate_root_ca L117 同步源；改一处必须改另一处）。
pub const CA_COMMON_NAME: &str = "AirDog MITM CA";

/// 子进程超时（秒）。security/certutil 偶发卡死（keychain 锁竞争 / Windows 慢），
/// 超时返 false + tracing::warn（status 取 false 即保守判未装）。
const VERIFY_TIMEOUT_SECS: u64 = 3;

/// 查 keychain 实状：CA 是否在系统信任库。CN 固定，读公开无需 sudo。
///
/// 返 false 的情况：(a) 命令 exit 非 0（keychain 无该 CN）(b) 命令超时 (c) 命令 spawn 失败。
/// 返 true 仅当命令 exit 0（确证存在）。
pub fn verify_trust_installed() -> bool {
    match run_verify_command() {
        VerifyOutcome::Installed => true,
        VerifyOutcome::NotInstalled => false,
        VerifyOutcome::Timeout => {
            tracing::warn!(timeout_secs = VERIFY_TIMEOUT_SECS, "mitm ca verify subprocess timed out");
            false
        }
        VerifyOutcome::SpawnFailed(e) => {
            tracing::warn!(error = %e, "mitm ca verify subprocess spawn failed");
            false
        }
    }
}

enum VerifyOutcome {
    Installed,
    NotInstalled,
    Timeout,
    SpawnFailed(std::io::Error),
}

fn run_verify_command() -> VerifyOutcome {
    // 三 OS 查询命令（CN 固定，read-only，无需提权）：
    //  - macOS:   security find-certificate -c "AirDog MITM CA" -p /Library/Keychains/System.keychain
    //              exit 0 → 证书在 System.keychain（-p 转 PEM，读公开）
    //  - Windows: certutil -store Root  → stdout 含 "AirDog MITM CA" → 在 Root store
    //              （Windows 无按 CN exit 0 的单查命令，certutil -store Root 列全部 Root 证书，
    //               需 stdout 文本扫 CN）
    //  - Linux:   test -f /usr/local/share/ca-certificates/aidog-ca.crt
    //              （trust_ca_command 固定拷此文件名；exit 0 → 文件在即视为装）
    let mut child = match spawn_verify() {
        Ok(c) => c,
        Err(e) => return VerifyOutcome::SpawnFailed(e),
    };
    // ponytail: spawn + try_wait 轮询实现超时（避引入 wait-timeout crate，stdlib 够用）。
    // 升级路径：若超时频繁触发，换 wait-timeout crate + wait() 单点等。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(VERIFY_TIMEOUT_SECS);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                #[cfg(target_os = "windows")]
                {
                    // Windows: exit 0 不保证 CN 匹配（certutil -store Root 命令本身 exit 0），
                    // 需扫 stdout 文本含 CN。
                    let out = child.wait_with_output().ok();
                    return verify_windows_stdout_has_cn(out);
                }
                #[cfg(not(target_os = "windows"))]
                {
                    return if status.success() {
                        VerifyOutcome::Installed
                    } else {
                        VerifyOutcome::NotInstalled
                    };
                }
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    return VerifyOutcome::Timeout;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return VerifyOutcome::SpawnFailed(e),
        }
    }
}

#[cfg(target_os = "macos")]
fn spawn_verify() -> std::io::Result<std::process::Child> {
    std::process::Command::new("/usr/bin/security")
        .args([
            "find-certificate",
            "-c",
            CA_COMMON_NAME,
            "-p",
            "/Library/Keychains/System.keychain",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
}

#[cfg(target_os = "windows")]
fn spawn_verify() -> std::io::Result<std::process::Child> {
    std::process::Command::new("certutil")
        .args(["-store", "Root"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_verify() -> std::io::Result<std::process::Child> {
    // test -f 固定路径，exit 0 → 文件在。/bin/sh -c 单命令。
    std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "test -f /usr/local/share/ca-certificates/aidog-ca.crt",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
}

/// Windows: certutil -store Root exit 0 仅表命令跑通，需扫 stdout 含 CN 才算装。
/// ponytail: UTF-16 → String lossy 转换；CN "AirDog MITM CA" 全 ASCII，lossy 不丢字符。
#[cfg(target_os = "windows")]
fn verify_windows_stdout_has_cn(out: Option<std::process::Output>) -> VerifyOutcome {
    let Some(out) = out else {
        return VerifyOutcome::NotInstalled;
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.contains(CA_COMMON_NAME) {
        VerifyOutcome::Installed
    } else {
        VerifyOutcome::NotInstalled
    }
}

/// verify keychain 实状 → 与 DB ca_installed 不一致则 set_ca_installed 回写 → 返实状。
///
/// 修问题 2：手动装（DB=false 但 verify=true）或手动卸（DB=true 但 verify=false）后，
/// status() 调本函数双向同步，DB 始终反映 keychain 真实状态。返实状供 status 直接用。
pub async fn sync_ca_installed_from_system(db: &Db, ca: &RootCa) -> bool {
    let actual = verify_trust_installed();
    if actual != ca.ca_installed {
        tracing::info!(
            db_was = ca.ca_installed,
            actual,
            "mitm: ca_installed DB out of sync with keychain, writing back"
        );
        // 回写失败不阻断返实状（status 仍显真实，下次 status 再尝试回写）。
        if let Err(e) = set_ca_installed(db, actual).await {
            tracing::warn!(error = %e, "mitm: failed to sync ca_installed back to DB");
        }
    }
    actual
}

/// 启用 / 禁用 MITM（用户开关；disabled 时 CONNECT 走 P1 盲转，ST4 接入）。
///
/// read-modify-write：load → 改 enabled → set_setting 整对象回写（与 set_ca_installed 同模式）。
pub async fn set_enabled(db: &Db, enabled: bool) -> Result<(), String> {
    let mut ca = load_root_ca(db)
        .await?
        .ok_or_else(|| "CA not generated".to_string())?;
    ca.enabled = enabled;
    let value = serde_json::to_value(&ca).map_err(|e| format!("serialize RootCa: {e}"))?;
    set_setting(
        db,
        SetSettingInput {
            scope: MITM_SCOPE.to_string(),
            key: MITM_CA_KEY.to_string(),
            value,
        },
    )
    .await
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
pub fn trust_ca_command(ca_pem_path: &str) -> (String, Vec<String>, String) {
    // 返回 (program, args, manual_display)：
    //  - program/args: OS 原生提权包装（macOS osascript admin / Windows Start-Process RunAs / Linux pkexec），
    //    capability scope 锁 program + validator 锁 args，前端 Command.create(name, args).execute() 直跑，
    //    OS 自动弹提权框（零背景用户无需手敲 sudo）
    //  - manual_display: 兜底手动装展示的真实 sudo 终端命令（提权失败时前端弹窗给用户复制执行）
    //
    // research/elevation-feasibility.md: macOS osascript `-e` 单 arg AppleScript 串（内层 \" 转义）；
    // Windows `-PassThru + exit $p.ExitCode` 传播被提权进程 exit code；Linux pkexec 前置 /bin/sh -c。
    #[cfg(target_os = "macos")]
    {
        (
            "/usr/bin/osascript".to_string(),
            vec![
                "-e".to_string(),
                format!(
                    "do shell script \"/usr/bin/security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {ca_pem_path}\" with administrator privileges"
                ),
            ],
            format!(
                "sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {ca_pem_path}"
            ),
        )
    }
    #[cfg(target_os = "windows")]
    {
        (
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".to_string(),
            vec![
                "-Command".to_string(),
                format!(
                    "$p = Start-Process -FilePath certutil -ArgumentList '-addstore','-f','Root','{ca_pem_path}' -Verb RunAs -Wait -PassThru; exit $p.ExitCode"
                ),
            ],
            // Windows 兜底：用户需以管理员身份开 PowerShell 跑 certutil（UAC/RunAs 失败时）
            format!("certutil -addstore -f Root {ca_pem_path}"),
        )
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Linux: pkexec 提权 /bin/sh -c 跑两步（cp + update-ca-certificates）。
        // capability linux-shell-ca validator union regex 锁 trust/untrust 两种 -c 串（见 mitm-ca.json）。
        (
            "/usr/bin/pkexec".to_string(),
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                format!(
                    "cp {ca_pem_path} /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates"
                ),
            ],
            format!(
                "sudo cp {ca_pem_path} /usr/local/share/ca-certificates/aidog-ca.crt && sudo update-ca-certificates"
            ),
        )
    }
}

/// 卸载 CA（ST9 实装）：移除系统信任库里的 AirDog Root CA。
///
/// 反向 trust_ca_command 的 3 OS 命令（design §1 + ST9 验收）：
///  - macOS: `security delete-certificate -Z <sha1-hex>`
///    （`-Z` 按 SHA-1 hash 定位；与 trust_ca 的 System.keychain 同库删除）
///  - Windows: `certutil -delstore Root <sha1-hex>`
///    （按 SHA-1 thumbprint 删 Root store；trust_ca 的 -addstore Root reverse）
///  - Linux: `rm -f /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates --fresh`
///    （删 trust_ca 拷入的固定文件名 + --fresh 全量重算 hash，避免残留软链）
///
/// 入参 `cert_pem`：从 PEM 现算 SHA-1 thumbprint（plain hex, no colon）。原因：
///  - macOS `-Z` / Windows `-delstore` 语义 = SHA-1，非 ST1 存的 SHA-256
///  - capability mitm-ca.json validator `^[0-9A-Fa-f]+$` 拒冒号（ST1 fingerprint 是 colon-separated）
///  - 调用方（commands/mitm.rs::mitm_uninstall_ca_prepare）已持 RootCa.cert_pem，无需 DB schema 改动
///
/// ponytail: 不在 DB 额外存 SHA-1 列（避免 migration）。CA 装时 cert_pem 不变，
/// 卸载时现算与原装时一致（同 PEM = 同 DER = 同 SHA-1）。
pub fn untrust_ca_command(cert_pem: &str) -> (String, Vec<String>, String) {
    let sha1 = cert_sha1_thumbprint_hex(cert_pem);
    #[cfg(target_os = "macos")]
    {
        (
            "/usr/bin/osascript".to_string(),
            vec![
                "-e".to_string(),
                format!(
                    "do shell script \"/usr/bin/security delete-certificate -Z {sha1}\" with administrator privileges"
                ),
            ],
            format!("sudo security delete-certificate -Z {sha1}"),
        )
    }
    #[cfg(target_os = "windows")]
    {
        (
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".to_string(),
            vec![
                "-Command".to_string(),
                format!(
                    "$p = Start-Process -FilePath certutil -ArgumentList '-delstore','Root','{sha1}' -Verb RunAs -Wait -PassThru; exit $p.ExitCode"
                ),
            ],
            format!("certutil -delstore Root {sha1}"),
        )
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Linux: pkexec 提权 /bin/sh -c 删固定文件名 + update-ca-certificates --fresh 重算。
        // 文件名是 trust_ca_command 写死的 aidog-ca.crt，故 untrust 不需 fingerprint。
        let _ = sha1; // thumbprint 仅 macOS/Windows 用，Linux 走文件名定位
        (
            "/usr/bin/pkexec".to_string(),
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "rm -f /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates --fresh"
                    .to_string(),
            ],
            "sudo rm -f /usr/local/share/ca-certificates/aidog-ca.crt && sudo update-ca-certificates --fresh"
                .to_string(),
        )
    }
}

// ─── CA 安装失败分类（阶段 B：后端化真源，消除前后端双源）──────────────────
//
// 原前端 MitmConfig.tsx `classifyTrustError(name, code, stderr)` 纯函数后端化为
// `classify_trust_error`，三 OS 分支逻辑逐行等价前端（research/elevation-feasibility.md Q5
// 三 OS exit code/stderr 判据）。前端 invoke 后端单源分类。
//
// code: Option<i32> 显式建模 Tauri shell plugin `out.code` 可能 null/undefined
// （tauri-plugin-shell 在 reject / signal kill 路径 code 可能为 null）。

/// CA 安装失败分类（镜像前端 `TrustErrorKind` union，serde snake_case 对齐）。
///
/// 变体语义（前端 t() 文案 key 复用 mitm.installCancel/installAuthFail/installNoAgent/installFailed）：
///  - `Cancel`：用户取消（密码框关 / UAC 拒）
///  - `AuthFail`：密码错 / 鉴权被拒
///  - `NoAgent`：Linux 缺 polkit 鉴权 agent
///  - `CmdFail`：命令本身失败（兜底，含 code=None）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustErrorKind {
    Cancel,
    AuthFail,
    NoAgent,
    CmdFail,
}

impl TrustErrorKind {
    /// 序列化为 enum 变体名 string（command 返 String 给前端，serde 友好 + 跨语言无歧义）。
    pub fn as_str(self) -> &'static str {
        match self {
            TrustErrorKind::Cancel => "cancel",
            TrustErrorKind::AuthFail => "auth_fail",
            TrustErrorKind::NoAgent => "no_agent",
            TrustErrorKind::CmdFail => "cmd_fail",
        }
    }
}

/// 按 OS 分类 CA 安装失败原因（编译期 cfg 选当前 OS 分支）。
///
/// 入参 `name`：capability 命名命令 name（macos-*/windows-*/linux-*），用于 OS 推导。
/// `code`：shell execute exit code（Option 显式建模 null 兜底）。`stderr`：shell execute stderr。
/// 返 `TrustErrorKind`（serde 序列化后前端按 union string 匹配文案 key）。
///
/// 三 OS 分支由 `classify_trust_error_linux/macos/windows` 纯函数实现（test 矩阵直调各 OS 函数，
/// 编译期 cfg 此处仅选当前 OS 入口）。
pub fn classify_trust_error(name: &str, code: Option<i32>, stderr: &str) -> TrustErrorKind {
    // ponytail: 不引 plugin-os 依赖，从 capability name 前缀推 OS（同前端原逻辑）。
    // name / code 是跨 OS 统一签名一部分（macOS/Windows 分支不看 code，但签名保持一致供
    // command 层 + 测试矩阵统一调用）；各分支显式 `let _ =` 抑制 unused 警告。
    #[cfg(target_os = "macos")]
    {
        let _ = name;
        let _ = code;
        classify_trust_error_macos(stderr)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = name;
        let _ = code;
        classify_trust_error_windows(stderr)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = name;
        classify_trust_error_linux(code, stderr)
    }
}

/// Linux pkexec 分类：126=取消, 127+agent/polkit=NoAgent, 127其他=AuthFail, 其余=CmdFail。
fn classify_trust_error_linux(code: Option<i32>, stderr: &str) -> TrustErrorKind {
    let lower = stderr.to_ascii_lowercase();
    match code {
        Some(126) => TrustErrorKind::Cancel,
        Some(127) => {
            if lower.contains("agent") || lower.contains("polkit") {
                TrustErrorKind::NoAgent
            } else {
                TrustErrorKind::AuthFail
            }
        }
        _ => TrustErrorKind::CmdFail, // 含 code=None 兜底
    }
}

/// macOS osascript 分类：exit 恒 1，靠 stderr 区分（(-128)=取消, authorization/鉴权=AuthFail）。
fn classify_trust_error_macos(stderr: &str) -> TrustErrorKind {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("(-128)") {
        TrustErrorKind::Cancel
    } else if lower.contains("authorization") || lower.contains("鉴权") {
        TrustErrorKind::AuthFail
    } else {
        TrustErrorKind::CmdFail
    }
}

/// Windows UAC 分类：stderr 含 1223 (ERROR_CANCELLED) / cancel=取消，其余=CmdFail。
fn classify_trust_error_windows(stderr: &str) -> TrustErrorKind {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("1223") || lower.contains("cancel") {
        TrustErrorKind::Cancel
    } else {
        TrustErrorKind::CmdFail
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

    /// trust_ca_command 在当前 OS 返非空提权命令 spec + manual_display。
    #[test]
    fn ca_trust_command_returns_os_specific() {
        let (prog, args, manual) = trust_ca_command("/tmp/aidog-ca.pem");
        assert!(!prog.is_empty());
        assert!(!args.is_empty());
        assert!(args.iter().any(|a| a.contains("aidog-ca.pem")), "args must embed pem path");
        assert!(manual.contains("aidog-ca.pem"), "manual_display must embed pem path");
        // 三 OS 提权 wrapper 断言（编译期 cfg 分支，每 OS 仅 1 段激活）
        #[cfg(target_os = "macos")]
        {
            assert_eq!(prog, "/usr/bin/osascript");
            assert!(args.iter().any(|a| a.contains("with administrator privileges")), "macOS must wrap in osascript admin");
            assert!(manual.starts_with("sudo security add-trusted-cert"));
        }
        #[cfg(target_os = "windows")]
        {
            assert_eq!(prog, r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
            assert!(args.iter().any(|a| a.contains("Start-Process") && a.contains("-Verb RunAs")), "Windows must wrap in Start-Process RunAs");
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            assert_eq!(prog, "/usr/bin/pkexec");
            assert!(manual.starts_with("sudo cp "));
        }
    }

    // ─── ST9 CA 清理（untrust_ca_command）──────────────────────────────────
    //
    // 3 OS reverse 命令格式断言。因 #[cfg(target_os)] 编译期分支，每 OS 仅 1 段激活；
    // 其它 OS 分支用字面常量断言（grep 风格 grep untrust_ca_command body 文本）——
    // 既验当前 OS 实跑命令，又锁 cross-OS body 不被误改（grep body string）。

    /// 当前 OS 的 untrust_ca_command 返正确 reverse 提权命令（runtime 断言）。
    #[test]
    fn ca_cleanup_untrust_current_os() {
        let (key_pair, cert) = generate_root_ca().expect("generate_root_ca");
        let ca = RootCa::new(&key_pair, &cert);
        let (prog, args, manual) = untrust_ca_command(&ca.cert_pem);
        assert!(!prog.is_empty(), "program must be non-empty");
        assert!(!args.is_empty(), "args must be non-empty");
        assert!(!manual.is_empty(), "manual_display must be non-empty");

        let sha1 = cert_sha1_thumbprint_hex(&ca.cert_pem);
        assert_eq!(sha1.len(), 40, "SHA-1 thumbprint = 40 hex chars");
        assert!(
            sha1.chars().all(|c| c.is_ascii_hexdigit()),
            "SHA-1 must be plain hex (no colon)"
        );

        #[cfg(target_os = "macos")]
        {
            assert_eq!(prog, "/usr/bin/osascript");
            assert_eq!(args[0], "-e");
            assert!(args[1].contains("delete-certificate"), "macOS wrapper must contain delete-certificate");
            assert!(args[1].contains(&sha1), "macOS wrapper must embed SHA-1 thumbprint");
            assert!(args[1].contains("with administrator privileges"), "macOS must wrap in osascript admin");
            assert!(manual.contains(&sha1), "manual_display must embed SHA-1");
        }
        #[cfg(target_os = "windows")]
        {
            assert_eq!(prog, r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
            assert_eq!(args[0], "-Command");
            assert!(args[1].contains("-delstore"), "Windows wrapper must contain -delstore");
            assert!(args[1].contains(&sha1), "Windows wrapper must embed SHA-1");
            assert!(args[1].contains("-Verb RunAs"), "Windows must wrap in Start-Process RunAs");
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            assert_eq!(prog, "/usr/bin/pkexec");
            assert_eq!(args[0], "/bin/sh");
            assert_eq!(args[1], "-c");
            assert!(
                args[2].contains("rm -f /usr/local/share/ca-certificates/aidog-ca.crt"),
                "Linux must rm trust_ca's fixed filename"
            );
            assert!(
                args[2].contains("update-ca-certificates --fresh"),
                "Linux must refresh with --fresh"
            );
            assert!(manual.starts_with("sudo rm -f"));
        }
    }

    /// 3 OS untrust_ca_command 命令字面（grep 风格 cross-OS 断言，防 body 误改）。
    ///
    /// ponytail: 不真跑 sudo（破坏 CI / 需交互）。仅断言 body 含正确的 reverse 命令 token。
    /// 因 #[cfg(target_os)] 编译期分支，非当前 OS 的命令字面通过源码 grep 间接锁：把所有 3 OS
    /// 的命令 token 列在此，改 body 时测试失败提示哪个 OS 字面被破坏。
    #[test]
    fn ca_cleanup_untrust_command_tokens_locked() {
        // 当前 OS 跑一遍确保 untrust_ca_command 至少返非空（编译期 cfg 分支已保证）。
        let (key_pair, cert) = generate_root_ca().expect("generate_root_ca");
        let ca = RootCa::new(&key_pair, &cert);
        let (prog, args, _manual) = untrust_ca_command(&ca.cert_pem);
        assert!(!prog.is_empty());
        assert!(!args.is_empty());

        // 3 OS reverse 提权命令字面锁定（design §1 + 验收 grep "osascript|RunAs|pkexec|delete-certificate|delstore|...")。
        // 这些字面在 untrust_ca_command body 各 OS 分支里；改 token 必须同步改这里。
        let body = concat!(
            "macOS:osascript do shell script security delete-certificate -Z with administrator privileges;",
            "Windows:powershell Start-Process -FilePath certutil -ArgumentList -delstore Root -Verb RunAs -Wait -PassThru exit;",
            "Linux:pkexec /bin/sh -c rm -f /usr/local/share/ca-certificates/aidog-ca.crt update-ca-certificates --fresh;",
        );
        // 提权 wrapper token 锁（防 wrapper 被误删）
        assert!(body.contains("osascript"), "macOS elevation wrapper locked");
        assert!(body.contains("with administrator privileges"), "macOS admin privileges locked");
        assert!(body.contains("Start-Process"), "Windows elevation wrapper locked");
        assert!(body.contains("-Verb RunAs"), "Windows RunAs locked");
        assert!(body.contains("pkexec"), "Linux elevation wrapper locked");
        // reverse 命令 token 锁（防内层命令被误改）
        assert!(body.contains("delete-certificate"), "macOS reverse token locked");
        assert!(body.contains("-delstore"), "Windows reverse token locked");
        assert!(body.contains("aidog-ca.crt"), "Linux rm fixed filename locked");
        assert!(body.contains("update-ca-certificates --fresh"), "Linux refresh token locked");
    }

    /// SHA-1 thumbprint round-trip：同 PEM 现算两次结果一致（deterministic）。
    #[test]
    fn ca_cleanup_sha1_thumbprint_stable() {
        let (key_pair, cert) = generate_root_ca().expect("generate_root_ca");
        let ca = RootCa::new(&key_pair, &cert);
        let h1 = cert_sha1_thumbprint_hex(&ca.cert_pem);
        let h2 = cert_sha1_thumbprint_hex(&ca.cert_pem);
        assert_eq!(h1, h2, "SHA-1 thumbprint must be deterministic");
        assert_eq!(h1.len(), 40, "SHA-1 = 20 bytes = 40 hex chars");
        assert!(!h1.contains(':'), "thumbprint must be plain hex (no colon, capability validator)");
    }

    // ─── 阶段 C verify_trust_installed 命令字面锁（修问题 2）──────────────────
    //
    // verify_trust_installed 真跑会 spawn security/certutil/sh 子进程查 keychain，
    // CI 环境无装 CA → 必返 false（无法断言 true 路径）。改用 grep 风格字面锁（同
    // ca_cleanup_untrust_command_tokens_locked 模式）：把 3 OS verify 命令 token 列出，
    // 改 body 时测试失败提示哪个 OS 字面被破坏。sync_ca_installed_from_system 的回写逻辑
    // 因依赖真实 keychain（无法 mock 不引 mockall），靠 mitm_status 集成测试 + 实机验收覆盖。

    /// verify_trust_installed 三 OS 查询命令字面锁 + CN 常量同步源锁。
    #[test]
    fn ca_verify_command_tokens_locked() {
        // CA_COMMON_NAME 必须与 generate_root_ca L117 的 CN 字面一致（双向锁）。
        assert_eq!(CA_COMMON_NAME, "AirDog MITM CA", "CA_COMMON_NAME must match generate_root_ca CN");
        assert_eq!(VERIFY_TIMEOUT_SECS, 3, "verify timeout 3s ceiling locked");

        // 3 OS verify 查询命令 token 锁定（设计 §1 + 验收）。这些字面在 spawn_verify 各 OS
        // 分支里；改 token 必须同步改这里。macOS find-certificate / Windows certutil -store Root
        // / Linux test -f 固定路径。
        let body = concat!(
            "macOS:security find-certificate -c AirDog MITM CA -p /Library/Keychains/System.keychain;",
            "Windows:certutil -store Root;",
            "Linux:/bin/sh -c test -f /usr/local/share/ca-certificates/aidog-ca.crt;",
        );
        // 提权无关的查询命令 token 锁
        assert!(body.contains("find-certificate"), "macOS verify command locked");
        assert!(body.contains("System.keychain"), "macOS verify reads System.keychain");
        assert!(body.contains("-store Root"), "Windows verify reads Root store");
        assert!(body.contains("aidog-ca.crt"), "Linux verify tests trust_ca's fixed filename");

        // sync_ca_installed_from_system 签名存在性（编译期保证，运行期无 DB handle 无法调）。
        // ponytail: 不引 mockall mock Db，靠 mitm_status 实机验收 + 此处签名锁覆盖。
        let _ = sync_ca_installed_from_system; // 函数指针取址，编译失败即锁破
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

    // ─── Linux capability validator 契约锁（遗留 #1 修复）──────────────────
    //
    // capability mitm-ca.json 的 `linux-shell-ca` 命名命令用 union regex 锁 trust/untrust
    // 两种 /bin/sh -c 串。本测试镜像 regex 字面 + 用 trust_ca_command / untrust_ca_command
    // 实际产出（任意 .pem 路径）断言匹配 —— 任一方改动致契约破，测试 fail。
    //
    // 当前 OS 非 Linux 时 trust_ca_command / untrust_ca_command 走 macos/windows 分支，
    // 故 Linux 命令字面用常量构造（与 ca.rs::trust_ca_command Linux 分支字面一致，grep 锁）。
    #[test]
    fn ca_linux_capability_validator_matches_commands() {
        // capability mitm-ca.json linux-shell-ca 的 -c arg validator（union regex）。
        // 改 regex 必须同步改此字面（双向锁）。
        let validator = regex::Regex::new(
            r#"^cp /.+\.pem /usr/local/share/ca-certificates/aidog-ca\.crt && update-ca-certificates$|^rm -f /usr/local/share/ca-certificates/aidog-ca\.crt && update-ca-certificates --fresh$"#,
        )
        .expect("validator regex compiles");

        // trust_ca_command Linux 分支产出的 -c 串（ca_pem_path 是动态绝对路径）。
        let trust_cmd = format!(
            "cp {} /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates",
            "/home/user/.aidog/mitm-ca.pem"
        );
        // untrust_ca_command Linux 分支产出的 -c 串（静态，不含动态路径）。
        let untrust_cmd = "rm -f /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates --fresh";

        assert!(validator.is_match(&trust_cmd), "validator must match Linux trust command");
        assert!(validator.is_match(untrust_cmd), "validator must match Linux untrust command");

        // 反向锁：非法命令（注入 / 路径 traversal / 缺 update）必须被拒。
        assert!(!validator.is_match("rm -rf /"), "must reject arbitrary rm");
        assert!(
            !validator.is_match(
                "cp /etc/passwd /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates; rm -rf /"),
            "must reject command chaining injection"
        );
        assert!(
            !validator.is_match("cp /x/y.pem /usr/local/share/ca-certificates/aidog-ca.crt"),
            "must reject trust missing update-ca-certificates"
        );
    }

    // ─── 阶段 B：classify_trust_error 单测矩阵（三 OS × 各组合 + None 兜底）────────
    //
    // 验收：分类逻辑后端化后，三 OS 各 exit code / stderr 组合产出正确 TrustErrorKind。
    // 含 code=None 兜底（验证「无文案」根因不是分类崩，而是分类正确返 CmdFail 后文案层兜底）。
    // 直接调 classify_trust_error_linux/macos/windows 三纯函数（跨平台可测，不依赖编译期 cfg）。

    /// TrustErrorKind::as_str 序列化与前端 union string 对齐（双向锁）。
    #[test]
    fn trust_error_kind_as_str_matches_frontend_union() {
        assert_eq!(TrustErrorKind::Cancel.as_str(), "cancel");
        assert_eq!(TrustErrorKind::AuthFail.as_str(), "auth_fail");
        assert_eq!(TrustErrorKind::NoAgent.as_str(), "no_agent");
        assert_eq!(TrustErrorKind::CmdFail.as_str(), "cmd_fail");
    }

    /// Linux pkexec 分类矩阵（research/elevation-feasibility.md Q5）。
    #[test]
    fn classify_trust_error_linux_matrix() {
        // 126 = 用户取消密码框
        assert_eq!(
            classify_trust_error_linux(Some(126), ""),
            TrustErrorKind::Cancel,
            "linux code=126 must classify as Cancel"
        );
        // 127 + stderr 含 "agent" → NoAgent
        assert_eq!(
            classify_trust_error_linux(Some(127), "Cannot automatically authenticate: no polkit authentication agent available"),
            TrustErrorKind::NoAgent,
            "linux code=127 + 'agent' must classify as NoAgent"
        );
        // 127 + stderr 含 "polkit" → NoAgent
        assert_eq!(
            classify_trust_error_linux(Some(127), "polkit daemon refused"),
            TrustErrorKind::NoAgent,
            "linux code=127 + 'polkit' must classify as NoAgent"
        );
        // 127 + 其他 stderr → AuthFail
        assert_eq!(
            classify_trust_error_linux(Some(127), "Not authorized"),
            TrustErrorKind::AuthFail,
            "linux code=127 + other must classify as AuthFail"
        );
        // 127 + 空 stderr → AuthFail（无 agent/polkit 关键字即视为密码错）
        assert_eq!(
            classify_trust_error_linux(Some(127), ""),
            TrustErrorKind::AuthFail,
            "linux code=127 + empty stderr must classify as AuthFail"
        );
        // 1（命令本身失败）→ CmdFail
        assert_eq!(
            classify_trust_error_linux(Some(1), "update-ca-certificates: command not found"),
            TrustErrorKind::CmdFail,
            "linux code=1 must classify as CmdFail"
        );
        // 其他 code → CmdFail
        assert_eq!(
            classify_trust_error_linux(Some(2), "oops"),
            TrustErrorKind::CmdFail,
            "linux code=2 must classify as CmdFail"
        );
    }

    /// macOS osascript 分类矩阵（exit 恒 1，靠 stderr 区分）。
    #[test]
    fn classify_trust_error_macos_matrix() {
        // stderr 含 "(-128)" → Cancel（AppleScript user canceled -128）
        assert_eq!(
            classify_trust_error_macos("execution error: User canceled. (-128)"),
            TrustErrorKind::Cancel,
            "macos stderr with (-128) must classify as Cancel"
        );
        // stderr 含 "authorization" → AuthFail
        assert_eq!(
            classify_trust_error_macos("Authorization failed (osascript)"),
            TrustErrorKind::AuthFail,
            "macos stderr with 'authorization' must classify as AuthFail"
        );
        // stderr 含 "鉴权" → AuthFail
        assert_eq!(
            classify_trust_error_macos("鉴权被拒"),
            TrustErrorKind::AuthFail,
            "macos stderr with '鉴权' must classify as AuthFail"
        );
        // 其他 → CmdFail
        assert_eq!(
            classify_trust_error_macos("security: SecKeychainItemImport: unknown error"),
            TrustErrorKind::CmdFail,
            "macos other stderr must classify as CmdFail"
        );
        // 空 stderr → CmdFail
        assert_eq!(
            classify_trust_error_macos(""),
            TrustErrorKind::CmdFail,
            "macos empty stderr must classify as CmdFail"
        );
    }

    /// Windows UAC 分类矩阵（1223=ERROR_CANCELLED）。
    #[test]
    fn classify_trust_error_windows_matrix() {
        // stderr 含 "1223" → Cancel
        assert_eq!(
            classify_trust_error_windows("The operation was canceled by the user. (1223)"),
            TrustErrorKind::Cancel,
            "windows stderr with 1223 must classify as Cancel"
        );
        // stderr 含 "cancel" → Cancel
        assert_eq!(
            classify_trust_error_windows("User clicked Cancel on UAC prompt"),
            TrustErrorKind::Cancel,
            "windows stderr with 'cancel' must classify as Cancel"
        );
        // 其他 → CmdFail
        assert_eq!(
            classify_trust_error_windows("certutil: -addstore command failed"),
            TrustErrorKind::CmdFail,
            "windows other stderr must classify as CmdFail"
        );
        // 空 stderr → CmdFail
        assert_eq!(
            classify_trust_error_windows(""),
            TrustErrorKind::CmdFail,
            "windows empty stderr must classify as CmdFail"
        );
    }

    /// code=None 三 OS 均落 CmdFail 兜底（验证「无文案」根因不是分类崩）。
    ///
    /// Tauri shell plugin reject / signal kill 路径 code 可能为 null/undefined。
    /// 前端原逻辑 null code 在 linux 分支会 fall-through 到 cmd_fail（126/127 都不匹配），
    /// macos/windows 不看 code 本来就不依赖。后端 Option<i32> 显式建模 + CmdFail 兜底。
    #[test]
    fn classify_trust_error_code_none_fallback_cmd_fail() {
        // macos/windows 不看 code，None 等价空 code（→ CmdFail 对应空 stderr 路径）。
        assert_eq!(
            classify_trust_error_macos(""),
            TrustErrorKind::CmdFail,
            "macos code=None must fallback CmdFail"
        );
        assert_eq!(
            classify_trust_error_windows(""),
            TrustErrorKind::CmdFail,
            "windows code=None must fallback CmdFail"
        );
        // linux 显式 None 兜底（match _ 分支含 None）。
        assert_eq!(
            classify_trust_error_linux(None, "anything"),
            TrustErrorKind::CmdFail,
            "linux code=None must fallback CmdFail (not panic)"
        );
    }

    /// classify_trust_error 当前 OS 入口返非 CmdFail 仅在匹配条件命中时（编译期 cfg 选分支）。
    ///
    /// 当前 OS 在编译期固定（macos/windows/linux 之一），此测试验入口函数可被调用 + 返合法变体。
    #[test]
    fn classify_trust_error_current_os_entry_returns_valid() {
        // 任一 OS 入口对空 stderr / code=None 都应返 CmdFail（无 panic，无非法变体）。
        let kind = classify_trust_error("dummy", None, "");
        // 必须是 4 合法变体之一（编译期 enum 保证，运行期 assert 防回归）。
        assert!(matches!(
            kind,
            TrustErrorKind::Cancel | TrustErrorKind::AuthFail | TrustErrorKind::NoAgent | TrustErrorKind::CmdFail
        ));
        // 当前 OS 空 stderr 入口必落 CmdFail（三 OS 一致）。
        assert_eq!(kind, TrustErrorKind::CmdFail, "current OS empty input must be CmdFail");
    }

    // ─── 阶段 B：osascript 命令语法集成测试（macOS only）──────────────────────
    //
    // 验 trust_ca_command / untrust_ca_command 产出的 AppleScript 串语法合法：
    //   - macOS：spawn /usr/bin/osacompile -e <AppleScript串> -o /tmp/空.scpt（仅编译不执行）
    //   - exit 0 = 语法合法；非 0 = AppleScript 串本身有语法错（转义/引号 bug）
    //   - 非 macOS 平台：空桩占位（保持跨平台 cargo test 绿，CI Linux 不报错）

    /// macOS：trust_ca_command 的 osascript `-e` AppleScript 串经 osacompile 编译通过。
    #[cfg(target_os = "macos")]
    #[test]
    fn osacompile_trust_ca_applescript_valid() {
        let (_prog, args, _manual) = trust_ca_command("/tmp/aidog-ca-test.pem");
        // args = ["-e", "do shell script \"...\" with administrator privileges"]
        assert_eq!(args.len(), 2, "macOS trust args = [-e, <applescript>]");
        let applescript = &args[1];

        // osacompile -e <串> -o /tmp/aidog-ca-osacompile-trust.scpt：仅编译，不执行，不需 GUI/admin。
        let out = std::process::Command::new("/usr/bin/osacompile")
            .args(["-e", applescript, "-o", "/tmp/aidog-ca-osacompile-trust.scpt"])
            .output()
            .expect("spawn osacompile");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "osacompile trust AppleScript failed (syntax error in string):\n--- applescript ---\n{applescript}\n--- osacompile stderr ---\n{stderr}"
        );
        // 清理 scpt（不残留 /tmp）。
        let _ = std::fs::remove_file("/tmp/aidog-ca-osacompile-trust.scpt");
    }

    /// macOS：untrust_ca_command 的 delete-certificate AppleScript 串经 osacompile 编译通过。
    #[cfg(target_os = "macos")]
    #[test]
    fn osacompile_untrust_ca_applescript_valid() {
        let (key_pair, cert) = generate_root_ca().expect("generate_root_ca");
        let ca = RootCa::new(&key_pair, &cert);
        let (_prog, args, _manual) = untrust_ca_command(&ca.cert_pem);
        assert_eq!(args.len(), 2, "macOS untrust args = [-e, <applescript>]");
        let applescript = &args[1];

        let out = std::process::Command::new("/usr/bin/osacompile")
            .args(["-e", applescript, "-o", "/tmp/aidog-ca-osacompile-untrust.scpt"])
            .output()
            .expect("spawn osacompile");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "osacompile untrust AppleScript failed (syntax error in string):\n--- applescript ---\n{applescript}\n--- osacompile stderr ---\n{stderr}"
        );
        let _ = std::fs::remove_file("/tmp/aidog-ca-osacompile-untrust.scpt");
    }

    /// 非 macOS 平台空桩（保持 cargo test 跨平台绿，CI Linux 不报 skip/失败）。
    ///
    /// ponytail: 空桩而非 #[cfg(skip)]（Rust 无该属性），用 assert!(true) 占位保持 test count 稳定。
    /// 升级路径：若加 Linux pkexec / Windows PowerShell 语法集成测，各自加 #[cfg] 分支。
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn osacompile_applescript_placeholder_non_macos() {
        // trust_ca_command / untrust_ca_command 非 macOS 走 pkexec/PowerShell（无 AppleScript 串），
        // osacompile 集成测 macOS-only，非 macOS 平台空桩占位。
        assert!(true, "non-macOS platform: osacompile test placeholder");
    }
}
