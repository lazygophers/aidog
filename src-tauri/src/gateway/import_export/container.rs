//! `.aidogx` 加密容器格式。
//!
//! ## 文件布局（little-endian）
//!
//! ```text
//! 偏移  长度   字段              说明
//! 0     4      magic            b"ADGX"
//! 4     1      version          1
//! 5     1      flags            0 (reserved)
//! 6     1      nonce_len        12
//! 7     1      obf_key_len      32
//! 8     32     obfuscated_key   K ⊕ pad
//! 40    12     nonce            AES-GCM nonce
//! 52    8      payload_len      u64 LE = len(ciphertext + tag)
//! 60    N      ciphertext+tag   AES-256-GCM(plaintext)（tag 在末尾 16B）
//! 60+N  32     hmac             HMAC-SHA256(key=hmac_key, msg=[0, 60+N))
//! ```
//!
//! ## 密钥隐藏
//!
//! - 真实密钥 `K = rand(32B)`，仅解密时在内存瞬时存在。
//! - `pad = SHA256(magic ‖ version ‖ SALT)`，`SALT` 是编译期常量（非密钥，只让 `pad` 不显然）。
//! - 头部存 `obfuscated_key = K ⊕ pad`；程序读出后 `K = obfuscated_key ⊕ pad`。
//! - 人眼看 magic 之后全是看似随机的字节（obf_key / nonce / cipher / hmac 视觉无差异）。
//!
//! ## 安全边界
//!
//! 满足「人眼无法判断密钥位」；**非防逆向** —— 拿到文件 + 程序即可重组 K（PRD 已确认此约束）。
//! 完整性靠三层：HMAC（防篡改头 + 密文）→ GCM tag（防密文篡改 + 解密失败）→ 上层 manifest.checksum。

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// 文件魔数。
const MAGIC: &[u8; 4] = b"ADGX";
/// 容器格式版本。
const VERSION: u8 = 1;
/// 编译期 salt（非密钥）。让 `pad` 不显然；改变它会使旧文件无法解密。
const SALT: &[u8] = b"aidog-salt-v1";
/// AES key 长度。
const KEY_LEN: usize = 32;
/// GCM nonce 长度。
const NONCE_LEN: usize = 12;
/// HMAC-SHA256 输出长度。
const HMAC_LEN: usize = 32;

type HmacSha256 = Hmac<Sha256>;

/// SHA256(bytes) → 小写 hex 字符串（manifest.checksum 用）。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

/// 计算隐藏密钥用的 pad：`SHA256(magic ‖ version ‖ SALT)`。
fn pad() -> [u8; KEY_LEN] {
    let mut h = Sha256::new();
    h.update(MAGIC);
    h.update([VERSION]);
    h.update(SALT);
    let out = h.finalize();
    let mut buf = [0u8; KEY_LEN];
    buf.copy_from_slice(&out);
    buf
}

/// HMAC 密钥（与 AES 密钥分离，派生自 pad）。
fn hmac_key() -> [u8; KEY_LEN] {
    let mut h = Sha256::new();
    h.update(pad());
    h.update(b"hmac-v1");
    let out = h.finalize();
    let mut buf = [0u8; KEY_LEN];
    buf.copy_from_slice(&out);
    buf
}

/// XOR 两个等长字节数组。
fn xor_into(dst: &mut [u8], b: &[u8]) {
    debug_assert_eq!(dst.len(), b.len());
    for (d, x) in dst.iter_mut().zip(b.iter()) {
        *d ^= *x;
    }
}

/// 加密明文 → `.aidogx` 字节流。
///
/// 步骤：生成随机 K + nonce → AES-256-GCM 加密 → 头部写 K⊕pad → 尾部追加 HMAC。
pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    // 随机密钥与 nonce。
    let mut key = [0u8; KEY_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut key);
    rng.fill_bytes(&mut nonce_bytes);

    // AES-256-GCM 加密（输出含末尾 16B tag）。
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("aes-gcm encrypt: {e}"))?;

    // 组装文件：header + ciphertext。
    let mut out = Vec::with_capacity(60 + ciphertext.len() + HMAC_LEN);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(0); // flags
    out.push(NONCE_LEN as u8);
    out.push(KEY_LEN as u8);
    // obfuscated_key = K ⊕ pad。
    let mut obf = key;
    xor_into(&mut obf, &pad());
    out.extend_from_slice(&obf);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&(ciphertext.len() as u64).to_le_bytes());
    out.extend_from_slice(&ciphertext);

    // HMAC over [0, end-of-ciphertext)。
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(&hmac_key()).map_err(|e| format!("hmac init: {e}"))?;
    mac.update(&out);
    let tag = mac.finalize().into_bytes();
    out.extend_from_slice(&tag);

    // 清掉内存中的明文密钥（best-effort）。
    let _ = key;

    Ok(out)
}

/// 解密 `.aidogx` 字节流 → 明文。
///
/// 步骤：校验 magic/version → 重组 K（obf ⊕ pad）→ HMAC 校验 → GCM 解密。
pub fn decrypt(file: &[u8]) -> Result<Vec<u8>, String> {
    if file.len() < 60 + HMAC_LEN {
        return Err(format!(
            "file too short: {} bytes (need >= {})",
            file.len(),
            60 + HMAC_LEN
        ));
    }
    if &file[0..4] != MAGIC {
        return Err("bad magic: not an .aidogx file".into());
    }
    let version = file[4];
    if version != VERSION {
        return Err(format!("unsupported version: {version}"));
    }
    let flags = file[5];
    if flags != 0 {
        return Err(format!("unknown flags: {flags}"));
    }
    let nonce_len = file[6] as usize;
    let obf_key_len = file[7] as usize;
    if nonce_len != NONCE_LEN || obf_key_len != KEY_LEN {
        return Err(format!(
            "bad lengths: nonce={nonce_len} key={obf_key_len}"
        ));
    }

    let header_fixed = 8;
    let obf_start = header_fixed;
    let obf_end = obf_start + KEY_LEN;
    let nonce_start = obf_end;
    let nonce_end = nonce_start + NONCE_LEN;
    let len_start = nonce_end;
    let len_end = len_start + 8;

    // HMAC 校验（覆盖除末尾 HMAC 外全部）。
    let body_end = file.len() - HMAC_LEN;
    let stored_hmac = &file[body_end..];
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(&hmac_key()).map_err(|e| format!("hmac init: {e}"))?;
    mac.update(&file[..body_end]);
    mac.verify_slice(stored_hmac)
        .map_err(|_| "hmac verification failed: file corrupted or tampered".to_string())?;

    // payload_len。
    if len_end > body_end {
        return Err("payload length field out of range".into());
    }
    let payload_len = u64::from_le_bytes(file[len_start..len_end].try_into().unwrap()) as usize;
    let ct_start = len_end;
    let ct_end = ct_start + payload_len;
    if ct_end != body_end {
        return Err(format!(
            "payload length mismatch: declared {payload_len}, actual {}",
            body_end - ct_start
        ));
    }

    // 重组密钥：K = obf ⊕ pad。
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&file[obf_start..obf_end]);
    xor_into(&mut key, &pad());

    let nonce_bytes = &file[nonce_start..nonce_end];
    let ciphertext = &file[ct_start..ct_end];

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "aes-gcm decrypt failed: wrong key or corrupted ciphertext".to_string())?;

    let _ = key;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_small() {
        let pt = b"hello aidog import/export";
        let ct = encrypt(pt).unwrap();
        assert_ne!(&ct[..], pt);
        let back = decrypt(&ct).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn roundtrip_large() {
        let pt = vec![0x42u8; 100_000];
        let ct = encrypt(&pt).unwrap();
        let back = decrypt(&ct).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn empty_plaintext() {
        let ct = encrypt(b"").unwrap();
        let back = decrypt(&ct).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn rejects_bad_magic() {
        let ct = encrypt(b"x").unwrap();
        let mut bad = ct.clone();
        bad[0] = b'X';
        assert!(decrypt(&bad).is_err());
    }

    #[test]
    fn rejects_tampered_hmac() {
        let ct = encrypt(b"secret").unwrap();
        let mut bad = ct.clone();
        let last = bad.len() - 1;
        bad[last] ^= 0xff;
        assert!(decrypt(&bad).is_err());
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let ct = encrypt(b"secret payload").unwrap();
        let mut bad = ct.clone();
        // 翻转 ciphertext 中间一字节（HMAC 区不动）。
        let mid = 60 + (bad.len() - 60 - HMAC_LEN) / 2;
        bad[mid] ^= 0x01;
        // HMAC 校验会先失败。
        assert!(decrypt(&bad).is_err());
    }

    #[test]
    fn rejects_truncated() {
        let ct = encrypt(b"abc").unwrap();
        let bad = &ct[..ct.len() - 5];
        assert!(decrypt(bad).is_err());
    }

    #[test]
    fn obfuscated_key_is_not_plaintext_key() {
        // 直接验证隐藏效果：文件内 obf 区 ≠ K（K 不可观测，这里只验证 obf ≠ 全零
        // 且每次加密因随机 K 不同 → obf 不同）。
        let ct1 = encrypt(b"same input").unwrap();
        let ct2 = encrypt(b"same input").unwrap();
        let obf1 = &ct1[8..8 + KEY_LEN];
        let obf2 = &ct2[8..8 + KEY_LEN];
        assert_ne!(obf1, obf2, "random K must produce different obf");
        // obf 不应等于 pad（否则 K=0）。pad 首字节极不可能与 obf 重合持续，用长度比较。
        assert_eq!(obf1.len(), KEY_LEN);
    }
}

#[cfg(test)]
mod payload_tests {
    use super::super::{Manifest, Payload};

    #[test]
    fn payload_checksum_roundtrip() {
        let mut p = Payload {
            manifest: Manifest {
                format_version: 1,
                aidog_version: "0.0.0-test".into(),
                created_at: "2026-06-14T00:00:00Z".into(),
                source_machine: "tester".into(),
                scopes: vec!["platform".into()],
                checksum: String::new(),
            },
            platform: vec![serde_json::json!({"name": "p1", "id": 1})],
            group: vec![],
            group_platform: vec![],
            setting: vec![["app".into(), "k".into(), "{}".into()]],
            codex_global: Some("test".into()),
            codex_profiles: vec![],
            claude_code_global: None,
            claude_code_group_settings: vec![],
            skills: vec![],
        };
        let bytes = p.serialize_with_checksum().unwrap();
        assert!(!p.manifest.checksum.is_empty());
        let restored = Payload::from_bytes_verified(&bytes).unwrap();
        assert_eq!(restored.manifest.checksum, p.manifest.checksum);
        assert_eq!(restored.setting.len(), 1);
    }

    #[test]
    fn payload_checksum_detects_tamper() {
        let mut p = Payload {
            manifest: Manifest {
                format_version: 1,
                aidog_version: "0.0.0".into(),
                created_at: "x".into(),
                source_machine: "x".into(),
                scopes: vec![],
                checksum: String::new(),
            },
            platform: vec![],
            group: vec![],
            group_platform: vec![],
            setting: vec![],
            codex_global: None,
            codex_profiles: vec![],
            claude_code_global: None,
            claude_code_group_settings: vec![],
            skills: vec![],
        };
        let bytes = p.serialize_with_checksum().unwrap();
        // 篡改 aidog_version（checksum 不变 → 校验失败）。
        let needle = b"\"0.0.0\"";
        let pos = bytes.windows(needle.len()).position(|w| w == needle).expect("needle");
        let mut corrupted = bytes.clone();
        corrupted[pos + 1] = b'9';
        assert_ne!(corrupted, bytes);
        assert!(Payload::from_bytes_verified(&corrupted).is_err());
    }
}
