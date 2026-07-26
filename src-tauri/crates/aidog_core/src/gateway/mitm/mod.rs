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
//! - `whitelist`: 全局 host suffix 匹配（D6），默认 AI host + 已配平台 host（migration 20260727-15，原 041/043 填）
//! - `cert_signer`: 按 SNI 动态签 host 证书（复用 ca.rs Root CA），缓存 CertifiedKey
//! - `tls`: tokio-rustls accept（假证书）+ connect 上游（真证书验证）+ pinning 降级标记
//!
//! 进程级状态（`mitm_state()`）:
//! - `suspects`: pinning_suspect host → 标记时间戳（进程内，非 DB）；查询时 TTL 自动 expire
//!   超龄条目（C8 收敛：原只增不减集合 → 带 TTL 自愈，pinning 短暂场景如上游临时证书错
//!   不再永久禁该 host）；TTL 上限 `SUSPECT_TTL_SECS`。用户可调 `reset_suspects` 手动清空
//!   （commands_proxy 暴露 `mitm_reset_suspects` 命令）。
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

/// pinning_suspect 标记的 TTL（秒）—— 超龄后该 host 重新进入 MITM 候选。
///
/// ponytail: 10 分钟覆盖上游临时证书错 / 短暂网络黑屏场景；pinning 是上游固定属性时
/// TTL 到期再 mark_suspect 即可（无重试上限 YAGNI）。值常量化，未来按平台调参再提 env。
const SUSPECT_TTL_SECS: u64 = 600;

/// 进程级 MITM 状态（OnceLock 单例，首次 `mitm_state()` 调用惰性初始化）。
///
/// ponytail: 用 `std::sync::OnceLock` 而非 once_cell / lazy_static 依赖（std 1.70+ 自带）。
/// 全进程共享一份 suspect map + signer —— CONNECT 是全局能力，suspect 标记跨连接复用
/// 才有意义（首次 pinning fail 后续连接降级）。
pub struct MitmState {
    /// pinning_suspect host → 标记时的 Unix 秒（进程内缓存，不持久化）。
    ///
    /// 上游 TLS 握手 fail（疑似 cert pinning）即加入，附 `now_secs()` 时间戳；查询时
    /// `is_suspect` 先剔超 TTL 的条目再判存在。`reset_suspects` 一键清空（用户手动重试 MITM）。
    suspects: Mutex<std::collections::HashMap<String, u64>>,

    /// CertSigner 懒构造锁。
    ///
    /// 首次 MITM 命中时从 DB load_root_ca 构造；DB 无 CA（用户未启用 MITM）→ None，
    /// MITM 路径降级盲转。构造后缓存（CA 轮换走重启进程，YAGNI 不做运行时 reload）。
    signer: Mutex<Option<Arc<CertSigner>>>,
}

impl MitmState {
    /// 测试专用：构造隔离实例（避全局 OnceLock 单例在并行 cargo test 间串扰）。
    ///
    /// ponytail: 生产代码禁用 —— 只走 `mitm_state()` 单例。cfg(test) 限定防误用。
    #[cfg(test)]
    pub(crate) fn fresh_for_test() -> Self {
        MitmState {
            suspects: Mutex::new(std::collections::HashMap::new()),
            signer: Mutex::new(None),
        }
    }

    /// host 是否在 pinning_suspect 集合（命中 → 跳过 MITM 降级盲转）。
    ///
    /// TTL 自动 expire：查询时若该 host 时间戳 + `SUSPECT_TTL_SECS` ≤ now → 视为过期，
    /// 从 map 剔除 + 返 false（host 重新进 MITM 候选）。
    pub async fn is_suspect(&self, host: &str) -> bool {
        let mut guard = self.suspects.lock().await;
        match guard.get(host).copied() {
            Some(ts) => {
                let now = now_secs();
                if ts + SUSPECT_TTL_SECS <= now {
                    // 过期：剔 + 视为非 suspect（host 重新可尝试 MITM）。
                    guard.remove(host);
                    tracing::info!(
                        host, marked_at = ts, now, ttl = SUSPECT_TTL_SECS,
                        "mitm: pinning_suspect expired, host re-eligible for MITM"
                    );
                    false
                } else {
                    true
                }
            }
            None => false,
        }
    }

    /// 标记 host 为 pinning_suspect（上游握手 fail 后调）。覆盖原时间戳（重置 TTL 计时）。
    pub async fn mark_suspect(&self, host: String) {
        let now = now_secs();
        self.suspects.lock().await.insert(host, now);
    }

    /// 一键清空 pinning_suspect 集合（C8 收敛：用户手动重试 MITM / 调试用）。
    ///
    /// 返清空前条目数（前端 toast 反馈用）。`mitm_reset_suspects` 命令直调本方法。
    pub async fn reset_suspects(&self) -> usize {
        let mut guard = self.suspects.lock().await;
        let n = guard.len();
        guard.clear();
        n
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
        suspects: Mutex::new(std::collections::HashMap::new()),
        signer: Mutex::new(None),
    })
}

/// 当前 Unix 秒（封装便于测试不依赖系统时钟时替换）。
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TTL 行为：mark 后 is_suspect=true， manipulated 时间戳过期后自动剔。
    #[tokio::test]
    async fn suspect_ttl_expires() {
        let state = MitmState {
            suspects: Mutex::new(std::collections::HashMap::new()),
            signer: Mutex::new(None),
        };
        state.mark_suspect("pinning.host.example".into()).await;
        assert!(
            state.is_suspect("pinning.host.example").await,
            "刚 mark 的 host 必须在 suspect 集合"
        );
        // 手动把时间戳拨到 TTL 之前模拟过期（直接操作内部 map）。
        {
            let mut g = state.suspects.lock().await;
            g.insert("pinning.host.example".into(), now_secs().saturating_sub(SUSPECT_TTL_SECS + 1));
        }
        let still = state.is_suspect("pinning.host.example").await;
        assert!(
            !still,
            "TTL 过期后 is_suspect 必须返 false 且剔条目（C8 自愈行为）"
        );
    }

    /// reset_suspects 清空 + 返清点数。
    #[tokio::test]
    async fn reset_suspects_clears_and_returns_count() {
        let state = MitmState {
            suspects: Mutex::new(std::collections::HashMap::new()),
            signer: Mutex::new(None),
        };
        state.mark_suspect("a.example".into()).await;
        state.mark_suspect("b.example".into()).await;
        let n = state.reset_suspects().await;
        assert_eq!(n, 2, "reset_suspects 返清点数 = 清前集合大小");
        assert!(!state.is_suspect("a.example").await, "reset 后 a 不再 suspect");
        assert!(!state.is_suspect("b.example").await, "reset 后 b 不再 suspect");
    }
}
