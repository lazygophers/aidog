//! P3 MITM 解密隧道子系统入口。
//!
//! 当前 ST1（假 CA）+ ST2（白名单）+ ST3（TLS 层）+ ST4（CONNECT 分流）落地。
//! ST5（forward 接入：明文 Request 灌 handle_proxy_core）/ ST6（HTTP/2 ALPN 细化）由后续
//! subtask 补。本模块已接入代理热路径（connect.rs 调 `handle_mitm`），但 ST4 阶段只做
//! TLS 双向桥接（密文透传，不解 HTTP）——明文 Request 解析 + forward 链复用是 ST5。
//!
//! 子模块:
//! - `ca`: rcgen 生成 Root CA + DB 持久化（明文 + DB 文件权限 0600，D4/D5）
//!   + 装信任库（macOS/Windows/Linux 经 tauri-plugin-shell + sudo，D1/D8）+ 清理（ST9）
//! - `whitelist`: 全局 host suffix 匹配（D6），默认 AI host + 已配平台 host（migration 041 填）
//! - `cert_signer`: 按 SNI 动态签 host 证书（复用 ca.rs Root CA），缓存 CertifiedKey
//! - `tls`: tokio-rustls accept（假证书）+ connect 上游（真证书验证）+ pinning 降级标记
//!
//! 进程级状态（`mitm_state()`）:
//! - `suspects`: pinning_suspect host 集合（进程内，YAGNI 不持久化；上游握手 fail 即标记，
//!   后续 CONNECT 该 host 直接降级 P1 盲转，design §3 弱点 6）
//! - `signer`: CertSigner 懒构造（首次 MITM 命中时从 DB load_root_ca 构造；DB 无 CA 即
//!   用户未启用 MITM，MITM 路径降级盲转）
//!
//! 设计依据：`.trellis/tasks/07-03-proxy-relay-mitm/design.md`、
//! `.trellis/spec/backend/proxy-connect-relay.md`（P1 契约，P3 待扩展）。

#![allow(dead_code)]

pub mod ca;
pub mod cert_signer;
pub mod tls;
pub mod whitelist;

use std::sync::{Arc, OnceLock};

use tokio::sync::Mutex;

use crate::gateway::db::Db;

use self::cert_signer::CertSigner;

/// 进程级 MITM 状态（OnceLock 单例，首次 `mitm_state()` 调用惰性初始化）。
///
/// ponytail: 用 `std::sync::OnceLock` 而非 once_cell / lazy_static 依赖（std 1.70+ 自带）。
/// 全进程共享一份 suspect set + signer —— CONNECT 是全局能力，suspect 标记跨连接复用
/// 才有意义（首次 pinning fail 后续连接降级）。
pub struct MitmState {
    /// pinning_suspect host 集合（进程内缓存，不持久化）。
    ///
    /// 上游 TLS 握手 fail（疑似 cert pinning）即加入；后续 CONNECT 该 host 命中集合 →
    /// 跳过 MITM 直接 P1 盲转（design 失败模式表第 3 行）。集合只增不减（无 TTL / 无清理）——
    /// pinning 是上游固定属性，不会自愈；用户重试上游仍会 fail。若未来需重试 MITM，
    /// 加前端「重置 suspect」按钮清集合。
    suspects: Mutex<std::collections::HashSet<String>>,

    /// CertSigner 懒构造锁。
    ///
    /// 首次 MITM 命中时从 DB load_root_ca 构造；DB 无 CA（用户未启用 MITM）→ None，
    /// MITM 路径降级盲转。构造后缓存（CA 轮换走重启进程，YAGNI 不做运行时 reload）。
    signer: Mutex<Option<Arc<CertSigner>>>,
}

impl MitmState {
    /// host 是否在 pinning_suspect 集合（命中 → 跳过 MITM 降级盲转）。
    pub async fn is_suspect(&self, host: &str) -> bool {
        self.suspects.lock().await.contains(host)
    }

    /// 标记 host 为 pinning_suspect（上游握手 fail 后调）。
    pub async fn mark_suspect(&self, host: String) {
        self.suspects.lock().await.insert(host);
    }

    /// 取或构造 CertSigner（首次从 DB 加载 RootCa；DB 无 CA 返 None）。
    ///
    /// ponytail: 锁内不做 IO（load_root_ca 在锁外 await 完成才进锁写入），避免持锁跨 await。
    /// 双检：已构造直接 clone 返；未构造才 load + 写入。
    pub async fn signer_or_init(&self, db: &Db) -> Result<Option<Arc<CertSigner>>, String> {
        {
            let guard = self.signer.lock().await;
            if let Some(s) = guard.as_ref() {
                return Ok(Some(s.clone()));
            }
        }
        // 锁外 load（DB IO 不持锁）。
        let ca = match ca::load_root_ca(db).await? {
            None => return Ok(None),
            Some(c) => c,
        };
        let signer = Arc::new(CertSigner::new(ca));
        let mut guard = self.signer.lock().await;
        // 并发下可能另一协程已先写入；以已存在优先（等价），覆盖浪费一次构造但无副作用。
        if let Some(existing) = guard.as_ref() {
            return Ok(Some(existing.clone()));
        }
        *guard = Some(signer.clone());
        Ok(Some(signer))
    }
}

/// 进程级 MITM 状态单例。
pub fn mitm_state() -> &'static MitmState {
    static STATE: OnceLock<MitmState> = OnceLock::new();
    STATE.get_or_init(|| MitmState {
        suspects: Mutex::new(std::collections::HashSet::new()),
        signer: Mutex::new(None),
    })
}
