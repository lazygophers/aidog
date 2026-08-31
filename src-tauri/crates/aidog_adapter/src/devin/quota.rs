//! Devin（Cognition）用量查询。
//!
//! Devin 按 **ACU（Agent Compute Units）** 计费，单位非 token 非时长非 $。
//! 文档未见实时余额端点，只能通过累计用量反推。
//!
//! 端点：`GET https://api.devin.ai/v3/organizations/{org_id}/consumption/daily`
//!   - 需 `ViewOrgConsumption` 权限
//!   - Bearer `cog_` API key
//!   - 响应含 `total_acus`（累计 ACU）+ `acus_by_product`（devin/cascade/terminal/review 分项）
//!
//! ## est_cost 约定（契约 9，跨 subtask 共享）
//!
//! `proxy_log.est_cost` 对 Devin 平台 **记录 session.acus_consumed（ACU 数，float）**，
//! **禁 token→$ 折算**（Devin 单价未公开，无可靠折算源）。
//! 实际 est_cost 赋值在 proxy 侧 handle_devin 内，本模块只定约定。
//!
//! BalanceInfo 映射（由 registry 脚本产出，quota-scripts T4 统一执行）：
//!   - `used`     = total_acus（累计已用 ACU）
//!   - `remaining`= 0.0（无余额端点，前端语义展示「ACU 用量」而非「$ 余额」）
//!   - `total`    = None（无总额度端点）
//!   - `currency` = "ACU"
//!   - `is_valid` = true（只要查询成功即认为 key 可用）
//!
//! org_id 来自 `platform.extra` JSON：`{"devin":{"org_id":"<id>"}}`。
//! 执行已统一走 registry quota 脚本（`registry/platforms/devin`）；本文件保留
//! extra 解析纯函数（proxy handle_devin 复用的真值源）。旧 Rust 查询实现已随 T4 移除。

use std::sync::Arc;

use aidog_db::Db;

use crate::quota::capability::QuotaCapability;
use crate::quota::http::{err_quota, PlatformQuota};

/// 从 platform.extra JSON 解析 Devin org_id。
/// 形态：`{"devin":{"org_id":"<id>"}}`（org_id 非空才返）。
pub fn parse_devin_extra(extra: &str) -> Option<String> {
    let org_id = aidog_db::models::PlatformExtra::parse(extra)
        .devin?
        .org_id?
        .trim()
        .to_string();
    if org_id.is_empty() {
        return None;
    }
    Some(org_id)
}

/// Devin 用量查询入口。
///
/// `_base_url`：保留参数以与 `query_quota` / `query_quota_newapi` 签名对称。
/// `api_key`：`cog_` 前缀 API key。
/// `extra`：platform.extra JSON，需含 `{"devin":{"org_id":"<id>"}}`。
///
/// 统一脚本路径（`quota::run_quota_script`）：物化列 → 自定义脚本 → registry 选中
/// （或首条）变体；registry 缺脚本（理论上不可能，bundled 编译期内置）→ Unsupported err。
pub async fn query_quota_devin(
    db: Option<&Arc<Db>>,
    base_url: &str,
    api_key: &str,
    extra: &str,
    platform_id: i64,
) -> PlatformQuota {
    crate::quota::run_quota_script(db, "devin", base_url, api_key, extra, platform_id)
        .await
        .unwrap_or_else(|| err_quota(&format!("Unsupported base_url for quota query: {base_url}")))
}

/// 平台 quota 能力配置（三函数模式之一；registry 侧由变体 returns 派生，T5 删除本函数）
pub fn quota_config() -> QuotaCapability {
    QuotaCapability {
        supports_balance: true,
        tier_names: vec![],
        ..Default::default()
    }.with_custom()
}

#[cfg(test)]
#[path = "test_quota.rs"]
mod test_quota;
