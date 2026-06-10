use super::db;
use super::models::*;

/// 路由结果
pub struct RouteResult {
    pub platform: Platform,
    pub target_model: String,
    /// 匹配到的模型映射（如果有），用于读取超时等维度配置
    pub mapping: Option<ModelMapping>,
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
        // 无显式映射
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
                mapping: mapping.cloned(),
            });
        }
    }

    // 4. 根据路由模式选择平台
    let platform = match group.routing_mode {
        RoutingMode::Failover => select_failover(&group_platforms),
        RoutingMode::LoadBalance => select_load_balance(&group_platforms),
    }?;

    // 5. 无显式映射时，按平台 PlatformModels 自动匹配模型
    let target_model = if mapping.is_none() {
        resolve_model(&platform.models, source_model)
    } else {
        target_model
    };

    Ok(RouteResult {
        platform,
        target_model,
        mapping: mapping.cloned(),
    })
}

/// 根据平台模型配置自动匹配请求模型。
/// 匹配规则：请求模型名（小写）包含槽位名（opus/sonnet/haiku/gpt）→ 使用该槽位值；
/// 全部不匹配 → 使用 default；无 default → 透传原始模型（去掉 [... ] 后缀）。
fn resolve_model(models: &PlatformModels, source_model: &str) -> String {
    // Strip Claude Code budget suffix like [1m], [128k]
    let base_model = source_model.split('[').next().unwrap_or(source_model);
    let lower = base_model.to_lowercase();
    let slots: [(&str, &Option<String>); 4] = [
        ("opus", &models.opus),
        ("sonnet", &models.sonnet),
        ("haiku", &models.haiku),
        ("gpt", &models.gpt),
    ];
    for (slot_name, slot_value) in &slots {
        if lower.contains(slot_name) {
            if let Some(v) = slot_value {
                return v.clone();
            }
        }
    }
    // 回退到 default
    if let Some(ref default) = models.default {
        return default.clone();
    }
    // 无匹配无 default — 透传（去掉 budget 后缀）
    base_model.to_string()
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
