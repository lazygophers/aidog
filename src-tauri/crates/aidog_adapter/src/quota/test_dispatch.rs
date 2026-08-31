//! query_quota 入口 dispatcher 覆盖：空 key / 不支持平台 早退分支。
//! provider 具体查询走 registry 脚本内固定 host，无法 stub（需真实网络），故仅覆盖
//! 可达的 host 路由判定早退。脚本等价断言见 test_balance / test_coding_plan /
//! test_special_scripts（stub + host retarget）。
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

#[tokio::test]
async fn query_quota_for_unsupported_protocol_falls_back_to_heuristic() {
    use aidog_db::models::Protocol;
    // 无 quota 脚本的协议 → 回落 base_url 启发式 → 无命中关键词 → Unsupported
    let q = query_quota_for(None, &Protocol::OpenAI, "https://unknown.example.com/v1", "sk-x", 0).await;
    assert!(!q.success);
    assert!(q.error.as_deref().unwrap().contains("Unsupported"));
}
