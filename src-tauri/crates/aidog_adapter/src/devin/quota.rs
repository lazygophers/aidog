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
//! org_id 取值（quota-scripts T5 与脚本对齐的两层兜底）：`extra.devin.org_id` 嵌套
//! 优先，缺失/空回落顶层 `extra.org_id`——真值源是 registry devin 脚本
//! （`registry/platforms/devin/platform.json` quota_scripts），proxy 侧 handle_devin
//! 经 `resolve_devin_org_id` 同款读取。执行统一走 registry quota 脚本；本文件仅剩
//! 特化 command 薄委托（旧 Rust 查询实现已随 T4 移除，parse_devin_extra 已随 T5 删除）。

use std::sync::Arc;

use aidog_db::Db;

use crate::quota::http::{err_quota, PlatformQuota};

/// Devin 用量查询入口。
///
/// `_base_url`：保留参数以与 `query_quota` / `query_quota_newapi` 签名对称。
/// `api_key`：`cog_` 前缀 API key。
/// `extra`：platform.extra JSON，org_id 读取见文件头。
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
