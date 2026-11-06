//! 额度用完后的自动真查退避链：30s → 60s → 120s → … 封顶 30 分钟，不设次数上限。
//! 纯函数 + 注册表 + 可注入 sleep/probe 的循环，测试不真 sleep、不外呼。

use super::algo::QuotaVerdict;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

pub const BACKOFF_BASE_MS: i64 = 30_000;
pub const BACKOFF_MAX_MS: i64 = 30 * 60_000;

/// 第 `attempt` 次（从 0 起）真查前的等待。
pub fn backoff_delay_ms(attempt: u32) -> i64 {
    // 6 次翻倍后已超 30 分钟，min(6) 防移位溢出
    (BACKOFF_BASE_MS << attempt.min(6)).min(BACKOFF_MAX_MS)
}

/// 一次退避真查的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// 平台已删除。
    Gone,
    /// 真查显示恢复。
    Recovered,
    /// 仍用完 / 查询失败 / 无信息：继续退避。
    Exhausted,
}

/// 活跃退避链注册表（platform_id → (已完成的真查次数, 代数令牌)）。同平台至多一条链；
/// 每次 `start` 换新代数，旧任务醒来发现令牌已换即退出，防「停链再起链」后双任务并行。
#[derive(Default)]
pub struct BackoffChains {
    chains: Mutex<HashMap<u64, (u32, u64)>>,
    next_gen: std::sync::atomic::AtomicU64,
}

impl BackoffChains {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<u64, (u32, u64)>> {
        self.chains.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 当前链的代数令牌；链不存在 → None。
    pub fn generation(&self, platform_id: u64) -> Option<u64> {
        self.map().get(&platform_id).map(|c| c.1)
    }

    pub fn is_active(&self, platform_id: u64) -> bool {
        self.map().contains_key(&platform_id)
    }

    /// 登记新链；已有 → false（不重复起链）。
    pub fn start(&self, platform_id: u64) -> bool {
        match self.map().entry(platform_id) {
            std::collections::hash_map::Entry::Occupied(_) => false,
            std::collections::hash_map::Entry::Vacant(v) => {
                let g = self
                    .next_gen
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                v.insert((0, g));
                true
            }
        }
    }

    pub fn stop(&self, platform_id: u64) {
        self.map().remove(&platform_id);
    }

    /// 当前已完成的真查次数；链不存在 → None。
    pub fn attempt(&self, platform_id: u64) -> Option<u32> {
        self.map().get(&platform_id).map(|c| c.0)
    }

    fn advance(&self, platform_id: u64) {
        if let Some(c) = self.map().get_mut(&platform_id) {
            c.0 = c.0.saturating_add(1);
        }
    }

    /// 判定驱动：Exhausted → 尝试起链（返回 true = 调用方需 spawn）；Available → 终止链（退避归零）；Unknown 不动。
    pub fn on_verdict(&self, platform_id: u64, verdict: QuotaVerdict) -> bool {
        match verdict {
            QuotaVerdict::Exhausted { .. } => self.start(platform_id),
            QuotaVerdict::Available => {
                self.stop(platform_id);
                false
            }
            QuotaVerdict::Unknown => false,
        }
    }
}

/// 退避循环：sleep(延迟) → 链仍在才 probe → Recovered/Gone 终止，否则退避加一档继续。
pub async fn run_chain<S, SF, P, PF>(
    chains: &BackoffChains,
    platform_id: u64,
    generation: u64,
    sleep: S,
    probe: P,
) where
    S: Fn(i64) -> SF,
    SF: Future<Output = ()>,
    P: Fn() -> PF,
    PF: Future<Output = Probe>,
{
    while let Some(attempt) = chains.attempt(platform_id) {
        sleep(backoff_delay_ms(attempt)).await;
        // 代数令牌：链被停并重起（如真查 Available 后再 Exhausted）后令牌已换，旧任务直接退出，
        // 不再 probe / advance 新链。
        if chains.generation(platform_id) != Some(generation) {
            return;
        }
        if !chains.is_active(platform_id) {
            return;
        }
        match probe().await {
            Probe::Exhausted => chains.advance(platform_id),
            Probe::Recovered | Probe::Gone => chains.stop(platform_id),
        }
    }
}

#[cfg(test)]
#[path = "test_backoff.rs"]
mod test_backoff;
