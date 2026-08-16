//! 批量操作共享结果类型（cli-proxy-batch-delete s1）。
//!
//! 从 commands_platform::batch 提到核心层，供 commands_platform / commands_cli_proxy 复用。

/// 批量操作结果
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchReport {
    /// 成功应用的操作数（原子事务下 = 0 或 ids.len()）
    pub applied: u64,
    /// 跳过的项目（当前原子事务下必为空，保留结构供扩展）
    pub skipped: Vec<SkipReason>,
}

/// 跳过原因
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipReason {
    pub id: u64,
    pub reason: String,
}
