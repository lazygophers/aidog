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
//! 实际 est_cost 赋值在 s3 `handle_devin` 内，本模块只定约定 + 提供 quota 侧的 ACU 解析。
//!
//! BalanceInfo 映射：
//!   - `used`     = total_acus（累计已用 ACU）
//!   - `remaining`= 0.0（无余额端点，前端语义展示「ACU 用量」而非「$ 余额」，s8 处理 UI 标注）
//!   - `total`    = None（无总额度端点）
//!   - `currency` = "ACU"
//!   - `is_valid` = true（只要查询成功即认为 key 可用）
//!
//! org_id 来自 `platform.extra` JSON：`{"devin":{"org_id":"<id>"}}`。

use std::sync::Arc;

use crate::gateway::db::Db;

use super::http::{
    err_quota, err_quota_platform, now_millis, parse_f64_field, quota_get_json, BalanceInfo,
    PlatformQuota, QUOTA_PLATFORM_ID,
};

/// Devin API 根（无版本前缀，consumption 端点自带 /v3）。
const DEVIN_API_ROOT: &str = "https://api.devin.ai";

/// 从 platform.extra JSON 解析 Devin org_id。
/// 形态：`{"devin":{"org_id":"<id>"}}`（org_id 非空才返）。
pub fn parse_devin_extra(extra: &str) -> Option<String> {
    if extra.trim().is_empty() {
        return None;
    }
    let obj: serde_json::Value = serde_json::from_str(extra).ok()?;
    let org_id = obj
        .get("devin")?
        .get("org_id")?
        .as_str()?
        .trim()
        .to_string();
    if org_id.is_empty() {
        return None;
    }
    Some(org_id)
}

/// 解析 consumption/daily 响应 → PlatformQuota（纯函数，不触网）。
///
/// 字段契约（research §7）：
///   - `total_acus`：累计 ACU 数（顶层，f64）
///   - `acus_by_product`：按 product 分项，可选
///
/// 字段名存疑时按上述解析；缺失 `total_acus` → 失败。
pub(crate) fn parse_devin_quota(body: &serde_json::Value) -> PlatformQuota {
    let total_acus = match parse_f64_field(body, "total_acus") {
        Some(v) => v,
        None => return err_quota_platform("devin", "Missing total_acus field"),
    };
    PlatformQuota {
        success: true,
        error: None,
        queried_at: now_millis(),
        balance: Some(BalanceInfo {
            remaining: 0.0,
            total: None,
            used: Some(total_acus),
            currency: "ACU".into(),
            is_valid: true,
        }),
        coding_plan: None,
        newapi_user_id: None,
    }
}

/// Devin 用量查询入口。
///
/// `base_url`：平台 base_url（实际未使用，Devin 端点固定 `DEVIN_API_ROOT`，保留参数以与
///   `query_quota` / `query_quota_newapi` 签名对称，便于未来 base_url 可配置）。
/// `api_key`：`cog_` 前缀 API key。
/// `extra`：platform.extra JSON，需含 `{"devin":{"org_id":"<id>"}}`。
pub async fn query_quota_devin(
    db: Option<&Arc<Db>>,
    _base_url: &str,
    api_key: &str,
    extra: &str,
    platform_id: i64,
) -> PlatformQuota {
    QUOTA_PLATFORM_ID.scope(platform_id, query_quota_devin_inner(db, api_key, extra)).await
}

async fn query_quota_devin_inner(
    db: Option<&Arc<Db>>,
    api_key: &str,
    extra: &str,
) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("Devin: api_key required");
    }
    let Some(org_id) = parse_devin_extra(extra) else {
        return err_quota("Devin: missing org_id in platform.extra (expected {\"devin\":{\"org_id\":\"<id>\"}})");
    };

    let url = format!(
        "{DEVIN_API_ROOT}/v3/organizations/{org_id}/consumption/daily"
    );
    let body = match quota_get_json(
        db,
        &url,
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("devin", &e),
    };
    parse_devin_quota(&body)
}

#[cfg(test)]
#[path = "test_devin.rs"]
mod test_devin;
