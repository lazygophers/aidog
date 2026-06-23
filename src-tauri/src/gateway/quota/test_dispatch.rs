//! query_quota 入口 dispatcher 覆盖：空 key / 不支持平台 早退分支。
//! provider 具体查询走硬编码 host，无法 stub（需真实网络），故仅覆盖可达的 host 路由判定早退。
use super::*;

#[tokio::test]
async fn empty_api_key_errors() {
    let q = query_quota(None, "https://api.deepseek.com", "  ", 0).await;
    assert!(!q.success);
    assert!(q.error.as_deref().unwrap().contains("API key"));
}

#[tokio::test]
async fn unsupported_platform_errors() {
    let q = query_quota(None, "https://unknown.example.com/v1", "sk-x", 0).await;
    assert!(!q.success);
    assert!(q.error.as_deref().unwrap().contains("Unsupported"));
}
