//! 事前花费模拟器（票 11）：前端传模型 + 四维 token 数，走既有 `calc_est_cost`
//! 计价链（resolve_price 三层顺序 + peak / fallback 语义，见 billing.rs doc）返回
//! 预估费用与计价依据。只算价，不发请求；透传 / 未知协议平台返 0 + unsupported。

use aidog_db::Db;

/// 模拟器计价结果 DTO（字段名 snake 直出，与 TS 契约对齐）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SimulatorEstCostDto {
    pub est_cost: f64,
    /// 条目价 / fallback / unsupported（透传、未知协议平台）。
    pub price_source: String,
    pub is_peak: bool,
    pub multiplier: f64,
}

/// [`crate::gateway::billing::calc_est_cost_detail`] 的平台侧薄壳：按 platform_id
/// 取平台行（platform_type + extra），计价链本身零复制。
#[allow(clippy::too_many_arguments)]
pub async fn estimate_platform(
    db: &Db,
    platform_id: i64,
    model_name: &str,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    cache_write_tokens: i32,
    at_ms: Option<i64>,
) -> Result<SimulatorEstCostDto, String> {
    let platform = aidog_db::get_platform(db, platform_id as u64)
        .await?
        .ok_or_else(|| format!("platform {platform_id} not found"))?;
    let platform_type = platform.platform_type.wire_str();
    let created_at_ms = at_ms.unwrap_or_else(aidog_db::now);
    let detail = crate::gateway::billing::calc_est_cost_detail(
        db,
        model_name,
        &platform_type,
        &platform.extra,
        input_tokens,
        output_tokens,
        cache_tokens,
        cache_write_tokens,
        0,
        platform_id,
        created_at_ms,
        &[],
    )
    .await;
    Ok(SimulatorEstCostDto {
        est_cost: detail.est_cost,
        price_source: detail.price_source,
        is_peak: detail.is_peak,
        multiplier: detail.multiplier,
    })
}

crate::tauri_command! {
/// 事前花费模拟器（票 11）：模型 + 四维 token 数 → 单次请求预估花费（$）与计价依据。
/// 透传 / 未知协议平台返 est_cost=0 + price_source="unsupported"。
pub async fn simulator_est_cost(
    platform_id: i64,
    model_name: String,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    cache_write_tokens: i32,
    at_ms: Option<i64>,
) -> Result<SimulatorEstCostDto, String> {
    let db = aidog_ctx::db();
    estimate_platform(
        db,
        platform_id,
        &model_name,
        input_tokens,
        output_tokens,
        cache_tokens,
        cache_write_tokens,
        at_ms,
    )
    .await
}
}

#[cfg(test)]
#[path = "test_cost_simulator.rs"]
mod tests;
