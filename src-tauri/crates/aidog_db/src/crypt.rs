//! platform.api_key 列级静态加密（db-field-encryption 票 01）。
//!
//! 主密钥 32B 随机，来源优先级（2026-10-21 拍板「永远静默」后调整）：
//! 1. env `AIDOG_DB_MASTER_KEY_FILE`（测试 / headless 显式注入，指向 key 文件，缺省创建）；
//! 2. key 文件 `<$HOME>/.aidog/master.key`（0600）——存在即用，**不碰 keychain**；
//! 3. keychain 存量条目 → 一次性读出落盘 key 文件（此后永走 2；不删 keychain 条目，
//!    留作文件丢失时的恢复手段）；
//! 4. 全新安装 → 生成新钥写 key 文件（**不写 keychain**：未签名 / 每次重签的二进制
//!    每读一次 keychain 就弹一次系统授权窗，make install / cicdbuild 产物全中招）。
//!    威胁面（spec 前置决策 2026-10-20）：防备份/同步带走 DB 但没带走 key 文件；
//!    不防同用户进程——能读家目录的进程本就在威胁模型边界外。
//!
//! 列值形态 `enc1:<base64(nonce12||ct+tag)>`；读取侧非该前缀按明文兼容处理（存量未迁移行）。
//! 主密钥成功取得后缓存在进程内（OnceLock），keychain 不进每请求路径。
//! 任何来源不可得 → 清晰 Err 不 panic；keychain 读失败且无降级文件时**不生成新钥**
//! （旧密文将永久不可读，宁可报错）。
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::Aes256Gcm;
use aes_gcm::aead::Nonce;
use std::sync::OnceLock;

use base64::Engine as _;

/// 密文列值前缀标记（读取侧以它判别密文/明文兼容）。
pub const ENC_PREFIX: &str = "enc1:";
const KEYCHAIN_SERVICE: &str = "aidog";
const KEYCHAIN_USER: &str = "db-master-key";
const ENV_KEY_FILE: &str = "AIDOG_DB_MASTER_KEY_FILE";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

static MASTER_KEY: OnceLock<[u8; KEY_LEN]> = OnceLock::new();

/// 进程内主密钥。成功即缓存（OnceLock）；失败不缓存，下次调用重新解析。
/// 并发首建的互斥由 install_key_file 的 create_new 兜住（输的一方改读赢方落盘的文件）。
pub fn master_key() -> Result<&'static [u8; KEY_LEN], String> {
    if let Some(k) = MASTER_KEY.get() {
        return Ok(k);
    }
    let k = resolve_master_key()?;
    let _ = MASTER_KEY.set(k);
    Ok(MASTER_KEY.get().expect("just set"))
}

/// 解析主密钥（不缓存）。`env_path` 即 `AIDOG_DB_MASTER_KEY_FILE` 的值，抽出参数便于测试。
pub(crate) fn resolve_master_key_from(env_path: Option<&std::path::Path>) -> Result<[u8; KEY_LEN], String> {
    if let Some(p) = env_path {
        return load_or_create_key_file(p);
    }
    // key 文件优先：存在即用，不碰 keychain（静默主路径）。
    let path = fallback_key_path()?;
    if path.exists() {
        return load_key_file(&path);
    }
    match keyring::v1::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER) {
        Ok(entry) => match entry.get_password() {
            // 存量迁移：读出旧钥落盘（这次读可能弹最后一次授权窗），此后永走文件。
            // 不删 keychain 条目——删除也可能触发授权，且留作文件丢失时的恢复手段。
            Ok(b64) => {
                let k = key_from_b64(&b64)?;
                let installed = install_key_file(&path, &k);
                if installed.is_ok() {
                    tracing::info!(path = %path.display(), "master key exported from keychain to key file");
                }
                installed
            }
            Err(keyring::v1::Error::NoEntry) => {
                // 全新安装：直接生成新钥落文件，不写 keychain（见模块头注释）。
                let k = new_random_key();
                install_key_file(&path, &k)
            }
            // keychain 读失败且无文件：不生成新钥（旧密文将永久不可读，宁可报错）。
            Err(e) => Err(format!(
                "master key unavailable: keychain read failed ({e}); no key file at {}",
                path.display()
            )),
        },
        Err(e) => Err(format!(
            "master key unavailable: keychain init failed ({e}); no key file at {}",
            path.display()
        )),
    }
}

/// 生成 / 迁移出的钥落盘；并发首建撞 create_new 时认另一进程已写好的文件。
fn install_key_file(
    path: &std::path::Path,
    key: &[u8; KEY_LEN],
) -> Result<[u8; KEY_LEN], String> {
    match create_key_file(path, key) {
        Ok(_) => Ok(*key),
        Err(_) if path.exists() => load_key_file(path),
        Err(e) => Err(e),
    }
}

fn resolve_master_key() -> Result<[u8; KEY_LEN], String> {
    if let Some(p) = std::env::var_os(ENV_KEY_FILE) {
        return resolve_master_key_from(Some(std::path::Path::new(&p)));
    }
    // shortcut: debug 构建（cargo test 全跑 debug）下 env 未设时先认 test_support 落的
    // 测试 key 文件——并行测试里未 ensure_test_master_key 的路径（迁移挂在 init_tables_raw
    // 后 test_rw_pool 等也摸 master_key）会直接读 macOS Keychain，未签名测试二进制每次弹
    // 授权窗（用户 2026-10-20 插话拍板修）。release 构建不走此短路，生产语义不变；
    // 升级条件：测试进程有统一注入点时删掉。
    #[cfg(debug_assertions)]
    {
        let test_key = std::env::temp_dir().join("aidog-db-test-master.key");
        if test_key.is_file() {
            return load_key_file(&test_key);
        }
    }
    resolve_master_key_from(None)
}

fn fallback_key_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| "master key unavailable: HOME not set for fallback key file".to_string())?;
    Ok(std::path::Path::new(&home).join(".aidog").join("master.key"))
}

/// key 文件：存在则读（严格 32B），不存在则生成新钥并以 0600 写入。
pub(crate) fn load_or_create_key_file(path: &std::path::Path) -> Result<[u8; KEY_LEN], String> {
    if path.exists() {
        load_key_file(path)
    } else {
        create_key_file(path, &new_random_key())
    }
}

fn load_key_file(path: &std::path::Path) -> Result<[u8; KEY_LEN], String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("master key file {} unreadable: {e}", path.display()))?;
    if bytes.len() != KEY_LEN {
        return Err(format!(
            "master key file {} must be exactly {KEY_LEN} bytes, got {}",
            path.display(),
            bytes.len()
        ));
    }
    let mut k = [0u8; KEY_LEN];
    k.copy_from_slice(&bytes);
    Ok(k)
}

fn create_key_file(path: &std::path::Path, key: &[u8; KEY_LEN]) -> Result<[u8; KEY_LEN], String> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    #[cfg(unix)]
    let mut opts = std::fs::OpenOptions::new();
    #[cfg(unix)]
    opts.mode(0o600);
    #[cfg(not(unix))]
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    let mut f = opts
        .open(path)
        .map_err(|e| format!("master key file {} not creatable: {e}", path.display()))?;
    std::io::Write::write_all(&mut f, key)
        .map_err(|e| format!("master key file {} not writable: {e}", path.display()))?;
    Ok(*key)
}

fn new_random_key() -> [u8; KEY_LEN] {
    let mut k = [0u8; KEY_LEN];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut k);
    k
}

fn key_from_b64(s: &str) -> Result<[u8; KEY_LEN], String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("master key in keychain not valid base64: {e}"))?;
    if bytes.len() != KEY_LEN {
        return Err(format!(
            "master key in keychain must be exactly {KEY_LEN} bytes, got {}",
            bytes.len()
        ));
    }
    let mut k = [0u8; KEY_LEN];
    k.copy_from_slice(&bytes);
    Ok(k)
}

/// 加密字段（主密钥取进程缓存）。空串保持空（保留 TRIM(api_key)='' 的「从未配置」语义）；
/// 已是密文形态则原样返回（防双重加密）。
pub fn encrypt_field(plain: &str) -> Result<String, String> {
    encrypt_with(master_key()?, plain)
}

/// 解密字段：非密文前缀按明文兼容原样返回（存量未迁移行）。
pub fn decrypt_field(stored: &str) -> Result<String, String> {
    decrypt_with(master_key()?, stored)
}

/// db-field-encryption：解密列 → rusqlite 行级错误（key 不可得 / 密文损坏 → 行级清晰报错不 panic）。
/// store 侧（mcp env/headers）与行编解码侧（platform.api_key）共用，禁各写一份 map_err。
pub fn decrypt_col_err(idx: usize, raw: &str) -> rusqlite::Result<String> {
    decrypt_field(raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other(e)),
        )
    })
}

pub(crate) fn encrypt_with(key: &[u8; KEY_LEN], plain: &str) -> Result<String, String> {
    if plain.is_empty() || plain.starts_with(ENC_PREFIX) {
        return Ok(plain.to_string());
    }
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_bytes);
    let nonce: Nonce<Aes256Gcm> = nonce_bytes.into();
    let ct = cipher
        .encrypt(&nonce, plain.as_bytes())
        .map_err(|_| "aes-gcm encrypt failed".to_string())?;
    let mut buf = Vec::with_capacity(NONCE_LEN + ct.len());
    buf.extend_from_slice(&nonce_bytes);
    buf.extend_from_slice(&ct);
    Ok(format!("{ENC_PREFIX}{}", base64::engine::general_purpose::STANDARD.encode(buf)))
}

pub(crate) fn decrypt_with(key: &[u8; KEY_LEN], stored: &str) -> Result<String, String> {
    let Some(b64) = stored.strip_prefix(ENC_PREFIX) else {
        return Ok(stored.to_string());
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("encrypted field not valid base64: {e}"))?;
    if bytes.len() <= NONCE_LEN {
        return Err("encrypted field too short".to_string());
    }
    let (n, ct) = bytes.split_at(NONCE_LEN);
    let nonce: Nonce<Aes256Gcm> = n.try_into().map_err(|_| "encrypted field: bad nonce".to_string())?;
    let cipher = Aes256Gcm::new(key.into());
    let pt = cipher
        .decrypt(&nonce, ct)
        .map_err(|_| "decrypt failed: master key mismatch or ciphertext corrupted".to_string())?;
    String::from_utf8(pt).map_err(|_| "decrypt failed: plaintext not UTF-8".to_string())
}
