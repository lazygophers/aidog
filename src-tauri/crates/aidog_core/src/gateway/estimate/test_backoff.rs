use super::*;
use std::cell::RefCell;

#[test]
fn delay_doubles_from_30s_and_caps_at_30min() {
    let got: Vec<i64> = (0..9).map(backoff_delay_ms).collect();
    assert_eq!(
        got,
        vec![30_000, 60_000, 120_000, 240_000, 480_000, 960_000, 1_800_000, 1_800_000, 1_800_000]
    );
    assert_eq!(backoff_delay_ms(u32::MAX), 1_800_000, "大次数不溢出");
}

#[test]
fn exhausted_starts_chain_once_only() {
    let c = BackoffChains::default();
    let ex = QuotaVerdict::Exhausted { until_ms: 5 };
    assert!(c.on_verdict(1, ex), "首次用完起链");
    assert!(!c.on_verdict(1, ex), "同平台不重复起链");
    assert!(c.on_verdict(2, ex), "别的平台独立");
    assert_eq!(c.attempt(1), Some(0));
}

#[test]
fn available_stops_chain_and_resets_backoff() {
    let c = BackoffChains::default();
    c.on_verdict(1, QuotaVerdict::Exhausted { until_ms: 5 });
    c.advance(1);
    assert_eq!(c.attempt(1), Some(1));
    assert!(!c.on_verdict(1, QuotaVerdict::Available));
    assert!(!c.is_active(1));
    assert!(c.on_verdict(1, QuotaVerdict::Exhausted { until_ms: 5 }));
    assert_eq!(c.attempt(1), Some(0), "恢复后再用完，退避从 30s 重新算");
}

#[test]
fn unknown_verdict_leaves_chain_alone() {
    let c = BackoffChains::default();
    assert!(!c.on_verdict(1, QuotaVerdict::Unknown), "无链时 Unknown 不起链");
    c.on_verdict(1, QuotaVerdict::Exhausted { until_ms: 5 });
    c.on_verdict(1, QuotaVerdict::Unknown);
    assert!(c.is_active(1));
}

#[tokio::test]
async fn chain_probes_with_backoff_until_recovered() {
    let c = BackoffChains::default();
    c.start(7);
    let delays = RefCell::new(Vec::new());
    let probes = RefCell::new(0u32);
    run_chain(
        &c,
        7,
        |ms| {
            delays.borrow_mut().push(ms);
            async {}
        },
        || {
            *probes.borrow_mut() += 1;
            let r = if *probes.borrow() >= 4 { Probe::Recovered } else { Probe::Exhausted };
            async move { r }
        },
    )
    .await;
    assert_eq!(*delays.borrow(), vec![30_000, 60_000, 120_000, 240_000]);
    assert_eq!(*probes.borrow(), 4);
    assert!(!c.is_active(7), "恢复后链终止");
}

#[tokio::test]
async fn chain_has_no_attempt_cap() {
    let c = BackoffChains::default();
    c.start(7);
    let n = RefCell::new(0u32);
    let last = RefCell::new(0i64);
    run_chain(
        &c,
        7,
        |ms| {
            *last.borrow_mut() = ms;
            async {}
        },
        || {
            *n.borrow_mut() += 1;
            let r = if *n.borrow() >= 50 { Probe::Gone } else { Probe::Exhausted };
            async move { r }
        },
    )
    .await;
    assert_eq!(*n.borrow(), 50);
    assert_eq!(*last.borrow(), 1_800_000, "封顶后保持 30 分钟");
}

#[tokio::test]
async fn chain_stops_without_probe_when_cancelled_during_sleep() {
    let c = BackoffChains::default();
    c.start(7);
    let probes = RefCell::new(0u32);
    run_chain(
        &c,
        7,
        |_| {
            c.stop(7); // 手动刷新在等待期间已解除
            async {}
        },
        || {
            *probes.borrow_mut() += 1;
            async { Probe::Exhausted }
        },
    )
    .await;
    assert_eq!(*probes.borrow(), 0, "链已被终止，不再真查");
}

#[tokio::test]
async fn chain_stops_when_platform_gone() {
    let c = BackoffChains::default();
    c.start(7);
    run_chain(&c, 7, |_| async {}, || async { Probe::Gone }).await;
    assert!(!c.is_active(7));
}
