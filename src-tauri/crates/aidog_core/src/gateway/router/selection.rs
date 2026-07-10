//! 旧版平台选择路径（已 deprecated，仅保留编译兼容；完整策略见 [`super::candidates`]）。

use super::super::db;
use super::super::models::*;
use super::effective_weight;
use super::model_mapping::resolve_model;
use super::RouteResult;

/// 根据分组路由规则选择平台
#[allow(dead_code)]
pub async fn select_platform(
    db: &db::Db,
    group: &Group,
    source_model: &str,
) -> Result<RouteResult, String> {
    // 1. 查找模型映射（内联于 group.model_mappings）
    let mapping = group.model_mappings.iter().find(|m| m.source_model == source_model);

    let (target_platform_id, target_model) = if let Some(m) = mapping {
        (m.target_platform_id, m.target_model.clone())
    } else {
        // 无显式映射（0 表示未指定）
        (0u64, source_model.to_string())
    };

    // 2. 获取分组中的平台列表
    let group_platforms = db::get_group_platforms(db, group.id).await?;
    if group_platforms.is_empty() {
        return Err("group has no platforms".to_string());
    }

    // 3. 如果有指定目标平台，优先使用
    if target_platform_id != 0 {
        if let Some(gp) = group_platforms.iter().find(|gp| gp.platform.id == target_platform_id) {
            tracing::info!(
                group = %group.name, source_model = %source_model, target_model = %target_model,
                platform = %gp.platform.name, platform_id = gp.platform.id,
                strategy = "explicit-mapping", "route selected"
            );
            return Ok(RouteResult {
                platform: gp.platform.clone(),
                target_model,
                mapping: mapping.cloned(),
            });
        }
        tracing::warn!(
            group = %group.name, target_platform_id,
            "mapped target platform not in group, falling back to routing mode"
        );
    }

    // 4. 根据路由模式选择平台
    let platform = match group.routing_mode {
        RoutingMode::Failover => select_failover(&group_platforms),
        // LoadBalance / HealthAware / LeastLatency / Sticky 在此简化路径均按加权随机
        // （本 fn 已 deprecated，仅 select_candidates_ctx 实现完整策略；保留编译）。
        _ => select_load_balance(&group_platforms),
    }?;

    // 5. 无显式映射时，按平台 PlatformModels 自动匹配模型
    let target_model = if mapping.is_none() {
        resolve_model(&platform.models, source_model)
    } else {
        target_model
    };

    tracing::info!(
        group = %group.name, source_model = %source_model, target_model = %target_model,
        platform = %platform.name, platform_id = platform.id,
        mode = ?group.routing_mode,
        strategy = if mapping.is_some() { "mapping+mode" } else { "auto-match" },
        "route selected"
    );

    Ok(RouteResult {
        platform,
        target_model,
        mapping: mapping.cloned(),
    })
}

/// 故障转移：按 priority 升序选第一个 enabled 的
fn select_failover(platforms: &[GroupPlatformDetail]) -> Result<Platform, String> {
    let mut sorted: Vec<_> = platforms.iter().collect();
    sorted.sort_by_key(|gp| (std::cmp::Reverse(gp.level_priority), gp.priority));

    sorted
        .into_iter()
        .find(|gp| gp.platform.enabled)
        .map(|gp| gp.platform.clone())
        .ok_or_else(|| "no enabled platform for failover".to_string())
}

/// 负载均衡：加权随机选择
fn select_load_balance(platforms: &[GroupPlatformDetail]) -> Result<Platform, String> {
    let enabled: Vec<_> = platforms.iter().filter(|gp| gp.platform.enabled).collect();
    if enabled.is_empty() {
        return Err("no enabled platform for load balance".to_string());
    }

    let total_weight: i32 = enabled.iter().map(|gp| effective_weight(gp)).sum();
    if total_weight <= 0 {
        return Ok(enabled[0].platform.clone());
    }

    // 简单加权随机（有效权重 = weight × level_priority）
    let mut rand_val = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i32)
        % total_weight;

    for gp in &enabled {
        rand_val -= effective_weight(gp);
        if rand_val < 0 {
            return Ok(gp.platform.clone());
        }
    }

    Ok(enabled[0].platform.clone())
}

#[cfg(test)]
#[path = "test_selection.rs"]
mod test_selection;
