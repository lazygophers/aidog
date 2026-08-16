//! proxy-port-no-drift s1 回归门：绑定层占用即失败 + 停止回写端口设置。
//! 接缝 = `start_proxy` 返回值 + 启动前后设置里的端口值（design.md「测试接缝」）。
use aidog_db as db;
use super::*;
use aidog_db::test_support::test_db;
use crate::gateway::middleware::MiddlewareEngine;

/// 端口被占用时 `start_proxy` 必须直接返回 Err（不再 +1 递增重试），且错误可判别为
/// 「端口占用」（`ProxyBindError::AddrInUse`，非靠字符串前缀匹配）。
#[tokio::test]
async fn start_proxy_fails_fast_when_port_occupied() {
    // 先占住一个端口（bind 到 127.0.0.1 固定端口，模拟外部程序已占用）。
    let occupier = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let occupied_port = occupier.local_addr().unwrap().port();

    let db = Arc::new(test_db().await);
    let err = start_proxy(db, occupied_port, None, Arc::new(MiddlewareEngine::new()), false)
        .await
        .expect_err("端口被占用时 start_proxy 必须返回 Err，不再递增换端口");

    assert!(
        matches!(err, ProxyBindError::AddrInUse(p) if p == occupied_port),
        "错误必须可判别为端口占用且携带原端口号，实际: {err:?}"
    );

    drop(occupier);
}

/// 根因 2 回归门：`start_proxy` 本身不持久化端口 —— 启动前 seed 的设置值，占用失败后
/// 读回必须原样不变（不存在「回写实际使用端口」这一步）。
#[tokio::test]
async fn start_proxy_never_rewrites_port_setting_on_failure() {
    let db = test_db().await;
    let seeded = crate::shared::ProxySettings {
        port: 9890,
        autostart: true,
        silent_launch: false,
        bind_lan: false,
    };
    crate::shared::save_proxy_settings_to_db(&db, &seeded).await.unwrap();

    // 占用一个与设置无关的端口，触发 start_proxy 走 Err 路径。
    let occupier = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let occupied_port = occupier.local_addr().unwrap().port();

    let db = Arc::new(db);
    let result = start_proxy(db.clone(), occupied_port, None, Arc::new(MiddlewareEngine::new()), false).await;
    assert!(result.is_err());

    let raw = db::get_setting(&db, "proxy", "settings").await.unwrap().unwrap();
    let after: crate::shared::ProxySettings = serde_json::from_value(raw).unwrap();
    assert_eq!(after.port, 9890, "start_proxy 失败路径禁改写设置里的端口值");

    drop(occupier);
}
