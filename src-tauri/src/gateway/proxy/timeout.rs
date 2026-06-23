use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::models::{Group, ModelMapping, RoutingMode};

    fn system(req: u64, conn: u64) -> ProxyTimeoutSettings {
        ProxyTimeoutSettings { request_timeout_secs: req, connect_timeout_secs: conn }
    }

    fn group_with(req: u64, conn: u64) -> Group {
        Group {
            id: 1, name: "g".into(), group_key: "gk".into(),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            created_at: 0, updated_at: 0, deleted_at: 0,
            request_timeout_secs: req, connect_timeout_secs: conn,
            source_protocol: "anthropic".into(),
            sort_order: 0, max_retries: 2,
            model_mappings: vec![], is_default: false,
        }
    }

    fn mapping_with(req: u64, conn: u64) -> ModelMapping {
        ModelMapping {
            source_model: "m".into(), target_model: "m".into(),
            target_platform_id: 1,
            request_timeout_secs: req, connect_timeout_secs: conn,
        }
    }

    /// 全部 0 → 系统默认 (300, 10)
    #[test]
    fn all_zero_uses_system_defaults() {
        let (req, conn) = resolve_timeout(&None, &group_with(0, 0), &system(0, 0));
        assert_eq!(req, 300);
        assert_eq!(conn, 10);
    }

    /// group 覆盖系统默认
    #[test]
    fn group_overrides_system() {
        let (req, conn) = resolve_timeout(&None, &group_with(60, 5), &system(0, 0));
        assert_eq!(req, 60);
        assert_eq!(conn, 5);
    }

    /// mapping 覆盖 group
    #[test]
    fn mapping_overrides_group() {
        let m = mapping_with(120, 15);
        let (req, conn) = resolve_timeout(&Some(m), &group_with(60, 5), &system(0, 0));
        assert_eq!(req, 120);
        assert_eq!(conn, 15);
    }

    /// mapping=0 回退 group
    #[test]
    fn mapping_zero_falls_back_to_group() {
        let m = mapping_with(0, 0);
        let (req, conn) = resolve_timeout(&Some(m), &group_with(90, 8), &system(300, 10));
        assert_eq!(req, 90);
        assert_eq!(conn, 8);
    }

    /// 系统非零值被使用
    #[test]
    fn system_nonzero_used_when_group_zero() {
        let (req, conn) = resolve_timeout(&None, &group_with(0, 0), &system(120, 20));
        assert_eq!(req, 120);
        assert_eq!(conn, 20);
    }
}

/// Read system-level timeout settings from DB
pub(crate) async fn get_system_timeout(db: &Db) -> ProxyTimeoutSettings {
    super::db::get_setting(db, "proxy", "timeout")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Resolve timeout by priority: model_mapping > group > system
pub(crate) fn resolve_timeout(
    mapping: &Option<super::models::ModelMapping>,
    group: &Group,
    system: &ProxyTimeoutSettings,
) -> (u64, u64) {
    let sys_req = if system.request_timeout_secs > 0 { system.request_timeout_secs } else { 300 };
    let sys_conn = if system.connect_timeout_secs > 0 { system.connect_timeout_secs } else { 10 };

    let (grp_req, grp_conn) = (
        if group.request_timeout_secs > 0 { group.request_timeout_secs } else { sys_req },
        if group.connect_timeout_secs > 0 { group.connect_timeout_secs } else { sys_conn },
    );

    match mapping {
        Some(m) => (
            if m.request_timeout_secs > 0 { m.request_timeout_secs } else { grp_req },
            if m.connect_timeout_secs > 0 { m.connect_timeout_secs } else { grp_conn },
        ),
        None => (grp_req, grp_conn),
    }
}
