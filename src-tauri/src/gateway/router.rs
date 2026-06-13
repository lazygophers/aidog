use super::db;
use super::models::*;

/// 路由结果
pub struct RouteResult {
    pub platform: Platform,
    pub target_model: String,
    /// 匹配到的模型映射（如果有），用于读取超时等维度配置
    pub mapping: Option<ModelMapping>,
}

/// 候选选取结果：有序的候选平台列表（首个为最优先），用于失败逐个重试。
/// `target_model` / `mapping` 对每个候选独立解析（显式映射命中时全部候选共享映射目标模型；
/// 否则按各平台 PlatformModels 自动匹配）。
pub struct CandidateSet {
    pub candidates: Vec<RouteResult>,
}

/// 判定平台当前是否可作为候选纳入：
/// - Enabled：始终纳入
/// - AutoDisabled 且已过退避试探时间（now >= until）：纳入（末尾试探）
/// - Disabled（用户手动）/ AutoDisabled 未到试探时间：排除
fn candidate_state(platform: &Platform, now_ms: i64) -> Option<bool> {
    match platform.status {
        PlatformStatus::Enabled => Some(false),
        PlatformStatus::AutoDisabled if now_ms >= platform.auto_disabled_until => Some(true),
        _ => None,
    }
}

/// 根据分组路由规则选择**有序候选平台列表**，用于失败逐个重试。
///
/// 排序：
/// - Failover：按 priority 升序
/// - LoadBalance：按权重加权随机决定首选，其余按权重降序排在后面（保证不同候选都能被试到）
///
/// 过滤：status==Enabled 优先纳入；auto_disabled 且已过退避试探时间的平台排在**末尾**惰性试探。
/// 显式 model_mapping 命中时，映射目标平台排在候选首位（最高优先），其余候选作为 failover 后备。
pub async fn select_candidates(
    db: &db::Db,
    group: &Group,
    source_model: &str,
) -> Result<CandidateSet, String> {
    let mapping = group.model_mappings.iter().find(|m| m.source_model == source_model);
    let mapped_target_model = mapping.map(|m| m.target_model.clone());
    let mapped_platform_id = mapping.map(|m| m.target_platform_id).filter(|id| *id != 0);

    let group_platforms = db::get_group_platforms(db, group.id).await?;
    if group_platforms.is_empty() {
        return Err("group has no platforms".to_string());
    }

    let now_ms = db::now();

    // 1. 拆分为「正常候选(enabled)」与「试探候选(auto_disabled 过期)」两组，分别保持模式排序，
    //    最终正常组在前、试探组在后。
    let mut active: Vec<&GroupPlatformDetail> = Vec::new();
    let mut probe: Vec<&GroupPlatformDetail> = Vec::new();
    for gp in &group_platforms {
        match candidate_state(&gp.platform, now_ms) {
            Some(false) => active.push(gp),
            Some(true) => probe.push(gp),
            None => {}
        }
    }

    // 2. 按路由模式排序两组
    match group.routing_mode {
        RoutingMode::Failover => {
            active.sort_by_key(|gp| gp.priority);
            probe.sort_by_key(|gp| gp.priority);
        }
        RoutingMode::LoadBalance => {
            order_load_balance(&mut active, now_ms);
            order_load_balance(&mut probe, now_ms);
        }
    }

    // 3. 合并：正常候选在前，试探候选在后
    let mut ordered: Vec<&GroupPlatformDetail> = Vec::with_capacity(active.len() + probe.len());
    ordered.extend(active);
    ordered.extend(probe);

    // 4. 显式映射目标平台提到最前（若它本身在候选集中）
    if let Some(target_id) = mapped_platform_id {
        if let Some(pos) = ordered.iter().position(|gp| gp.platform.id == target_id) {
            let gp = ordered.remove(pos);
            ordered.insert(0, gp);
        } else {
            tracing::warn!(
                group = %group.name, target_platform_id = target_id,
                "mapped target platform not an available candidate, falling back to routing order"
            );
        }
    }

    if ordered.is_empty() {
        return Err("no available platform (all disabled or backing off)".to_string());
    }

    // 5. 为每个候选解析目标模型
    let candidates: Vec<RouteResult> = ordered
        .into_iter()
        .map(|gp| {
            let target_model = if let Some(ref tm) = mapped_target_model {
                tm.clone()
            } else {
                resolve_model(&gp.platform.models, source_model)
            };
            RouteResult {
                platform: gp.platform.clone(),
                target_model,
                mapping: mapping.cloned(),
            }
        })
        .collect();

    tracing::info!(
        group = %group.name, source_model = %source_model,
        candidate_count = candidates.len(), mode = ?group.routing_mode,
        first_platform = %candidates[0].platform.name,
        "candidates selected"
    );

    Ok(CandidateSet { candidates })
}

/// 负载均衡排序：加权随机决定首个，其余按 weight 降序，保证所有候选都可被重试。
fn order_load_balance(platforms: &mut Vec<&GroupPlatformDetail>, seed: i64) {
    if platforms.len() <= 1 {
        return;
    }
    let total_weight: i32 = platforms.iter().map(|gp| gp.weight.max(0)).sum();
    // 先按 weight 降序作为基础顺序
    platforms.sort_by_key(|gp| std::cmp::Reverse(gp.weight));
    if total_weight <= 0 {
        return;
    }
    // 加权随机选首个（用时间种子），将其移到最前
    let mut rand_val = (seed.unsigned_abs() as i32) % total_weight;
    let mut pick = 0usize;
    for (i, gp) in platforms.iter().enumerate() {
        rand_val -= gp.weight.max(0);
        if rand_val < 0 {
            pick = i;
            break;
        }
    }
    if pick != 0 {
        let gp = platforms.remove(pick);
        platforms.insert(0, gp);
    }
}

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
        RoutingMode::LoadBalance => select_load_balance(&group_platforms),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_platform(status: PlatformStatus, until: i64) -> Platform {
        Platform {
            id: 1,
            name: "p".into(),
            platform_type: Protocol::Anthropic,
            base_url: String::new(),
            api_key: String::new(),
            extra: String::new(),
            models: PlatformModels::default(),
            available_models: vec![],
            endpoints: vec![],
            enabled: status == PlatformStatus::Enabled,
            status,
            auto_disabled_until: until,
            auto_disable_strikes: 0,
            created_at: 0,
            updated_at: 0,
            deleted_at: 0,
            est_balance_remaining: 0.0,
            est_coding_plan: String::new(),
            last_real_query_at: 0,
            estimate_count: 0,
            show_in_tray: false,
            tray_display: String::new(),
            sort_order: 0,
            manual_budgets: vec![],
            balance_level: String::new(),
        }
    }

    #[test]
    fn candidate_state_filtering() {
        let now = 1_000_000i64;
        // enabled → 始终纳入（非试探）
        assert_eq!(candidate_state(&mk_platform(PlatformStatus::Enabled, 0), now), Some(false));
        // 用户手动 disabled → 排除
        assert_eq!(candidate_state(&mk_platform(PlatformStatus::Disabled, 0), now), None);
        // auto_disabled 未到退避时间 → 排除
        assert_eq!(candidate_state(&mk_platform(PlatformStatus::AutoDisabled, now + 5000), now), None);
        // auto_disabled 已过退避时间 → 纳入（末尾试探）
        assert_eq!(candidate_state(&mk_platform(PlatformStatus::AutoDisabled, now - 1), now), Some(true));
    }
}
