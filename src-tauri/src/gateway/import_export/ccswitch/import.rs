//! apply 复用入口：把前端转换好的 platform payload + 决策写入 aidog DB。

use crate::gateway::db::Db;

/// 把前端转换好的 platform payload + 决策应用进 aidog DB。
/// 复用 [`super::super::apply::apply`]，不另造一套写入路径。
///
/// `auto_group=true` 时：apply 后 ensure-by-name 建/找 `cc-switch` 分组并关联本次导入平台
/// （toggle 默认开；关时跳过 ensure，行为完全等同改造前的导入，向后兼容）。
pub async fn import(
    platform_payload: Vec<serde_json::Value>,
    decisions: &[super::super::ConflictDecision],
    auto_group: bool,
    db: &Db,
) -> Result<super::super::ImportReport, String> {
    use super::super::{apply, Manifest, Payload, SCOPE_PLATFORM};

    // apply 前快照已有 platform id，供 auto-group 回出本次新建行。
    let before = if auto_group {
        apply::snapshot_platform_ids(db).await?
    } else {
        std::collections::BTreeSet::new()
    };

    let payload = Payload {
        manifest: Manifest {
            format_version: 1,
            aidog_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            source_machine: "cc-switch-import".to_string(),
            scopes: vec![SCOPE_PLATFORM.to_string()],
            checksum: String::new(),
        },
        platform: platform_payload,
        group: Vec::new(),
        group_platform: Vec::new(),
        setting: Vec::new(),
        codex_global: None,
        codex_profiles: Vec::new(),
        claude_code_global: None,
        claude_code_group_settings: Vec::new(),
        skills: Vec::new(),
    };
    let report = apply::apply(payload, decisions, db).await?;

    if auto_group {
        apply::ensure_group_and_attach(db, "cc-switch", &before).await?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::import_export::ConflictDecision;

    async fn test_db() -> Db {
        let db = Db::new(":memory:").await.unwrap();
        db.init_tables().await.unwrap();
        db
    }

    /// 空平台 payload + no auto_group → ImportReport 成功，applied 空.
    #[tokio::test]
    async fn import_empty_payload_no_autogroup() {
        let db = test_db().await;
        let report = import(vec![], &[], false, &db).await.expect("should succeed");
        assert!(report.applied.values().sum::<usize>() == 0);
    }

    /// 空平台 payload + auto_group=true → ensures cc-switch group.
    #[tokio::test]
    async fn import_empty_payload_with_autogroup() {
        let db = test_db().await;
        let _report = import(vec![], &[], true, &db).await.expect("should succeed");
        // cc-switch group should exist
        let groups = crate::gateway::db::list_groups(&db).await.unwrap();
        assert!(groups.iter().any(|g| g.name == "cc-switch"), "cc-switch group should be created");
    }

    /// 单平台 payload + auto_group=false → applied platform=1.
    #[tokio::test]
    async fn import_single_platform_no_autogroup() {
        let db = test_db().await;
        let platform_json = serde_json::json!({
            "name": "TestPlatform",
            "platform_type": "openai",
            "base_url": "https://api.openai.com/v1",
            "api_key": "sk-test",
            "endpoints": [],
            "extra": "{}",
            "models": [],
            "available_models": [],
            "auto_group": true
        });
        let report = import(vec![platform_json], &[], false, &db).await.expect("should succeed");
        assert!(report.applied.values().sum::<usize>() >= 1);
    }
}
