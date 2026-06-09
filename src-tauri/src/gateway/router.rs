use super::db;
use super::models::*;

/// 路由结果
pub struct RouteResult {
    pub platform: Platform,
    pub target_model: String,
}

/// 根据分组路由规则选择平台
pub fn select_platform(
    db: &db::Db,
    group: &Group,
    source_model: &str,
) -> Result<RouteResult, String> {
    // 1. 查找模型映射
    let mappings = db::list_model_mappings(db, &group.id)?;
    let mapping = mappings.iter().find(|m| m.source_model == source_model);

    let (target_platform_id, target_model) = if let Some(m) = mapping {
        (m.target_platform_id.clone(), m.target_model.clone())
    } else {
        // 无映射则透传原始模型名
        ("".to_string(), source_model.to_string())
    };

    // 2. 获取分组中的平台列表
    let group_platforms = db::get_group_platforms(db, &group.id)?;
    if group_platforms.is_empty() {
        return Err("group has no platforms".to_string());
    }

    // 3. 如果有指定目标平台，优先使用
    if !target_platform_id.is_empty() {
        if let Some(gp) = group_platforms.iter().find(|gp| gp.platform.id == target_platform_id) {
            return Ok(RouteResult {
                platform: gp.platform.clone(),
                target_model,
            });
        }
    }

    // 4. 根据路由模式选择平台
    let platform = match group.routing_mode {
        RoutingMode::Failover => select_failover(&group_platforms),
        RoutingMode::LoadBalance => select_load_balance(&group_platforms),
    }?;

    Ok(RouteResult {
        platform,
        target_model,
    })
}

/// 故障转移：按 priority 升序选第一个 enabled 的
fn select_failover(platforms: &[GroupPlatformDetail]) -> Result<Platform, String> {
    let mut sorted: Vec<_> = platforms.iter().collect();
    sorted.sort_by_key(|gp| gp.priority);

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

    let total_weight: i32 = enabled.iter().map(|gp| gp.weight).sum();
    if total_weight <= 0 {
        return Ok(enabled[0].platform.clone());
    }

    // 简单加权随机
    let mut rand_val = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i32)
        % total_weight;

    for gp in &enabled {
        rand_val -= gp.weight;
        if rand_val < 0 {
            return Ok(gp.platform.clone());
        }
    }

    Ok(enabled[0].platform.clone())
}
