//! CPA(CLIProxyAPI) 配置导入 Tauri command。
//!
//! 三段式：parse(解析+映射) → preview_quota(惰性查余额, 不落库) → apply(逐个建平台, 非原子)。
//!
//! 见 `.skein/task/cpa-import/design.md`。

use std::sync::Arc;

use aidog_core::gateway::{
    self,
    cpa_import::{self, MappedPlatform},
    db::{self, Db},
    models::{CreatePlatform, Platform, PlatformStatus, UpdatePlatform},
    quota::PlatformQuota,
};
use serde::{Deserialize, Serialize};
use tauri::State;

/// 解析结果（mapper 转换后的 ParseResult：providers 换成 MappedPlatform）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpaImportParseResult {
    pub platforms: Vec<MappedPlatform>,
    pub skipped: Vec<cpa_import::SkipReason>,
    pub source_files: Vec<String>,
}

/// apply 批量报告（非原子：成功的入库，失败的收集原因）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpaBatchReport {
    pub created: Vec<Platform>,
    pub failed: Vec<CpaBatchFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpaBatchFailure {
    pub name: String,
    pub error: String,
}

/// 解析 CPA 配置文件/压缩包/目录（+ 可选 auth-dir）→ 映射后的 MappedPlatform 列表。
/// 纯读，不建平台。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cpa_import_parse(
    path: String,
    auth_dir: Option<String>,
) -> Result<CpaImportParseResult, String> {
    tracing::debug!(command = "cpa_import_parse", path = %path, "command invoked");
    let parsed = cpa_import::parse_cpa_config(&path, auth_dir.as_deref())?;
    let platforms = cpa_import::map_providers(parsed.providers);
    Ok(CpaImportParseResult {
        platforms,
        skipped: parsed.skipped,
        source_files: parsed.source_files,
    })
}

/// 预览期临时查余额，不落库。
///
/// platform_id=None → persist_quota_to_db None-guard 直接 return（quota.rs:46-48），
/// est_balance 不写库。仅 9 provider 支持（DeepSeek/OpenRouter/GLM/Kimi/MiniMax/NewAPI/
/// SiliconFlow/StepFun/Novita），不支持者 PlatformQuota.success=false 前端显「—」。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cpa_import_preview_quota(
    base_url: String,
    api_key: String,
    db: State<'_, Db>,
) -> Result<PlatformQuota, String> {
    tracing::debug!(command = "cpa_import_preview_quota", base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let db_arc = Arc::new(db.inner().clone());
    // platform_id=0（i64），但 persist_quota_to_db 接收 Option<u64>：
    // 调用方这里直接不调 persist，保证零落库（platform_id=0 对应 pid=0 也可能误命中，
    // 故 preview 路径绕过 persist 更稳妥——ponytail: None-guard 的等价直接绕过）。
    let q = gateway::quota::query_quota(Some(&db_arc), &base_url, &api_key, 0).await;
    tracing::info!(command = "cpa_import_preview_quota", success = q.success, "quota preview done");
    Ok(q)
}

/// 批量创建平台（非原子尽力：逐个 platform_create，失败收集不中断）。
///
/// disabled=true 的条目创建后追加一次 UpdatePlatform 置 status=Disabled
/// （CreatePlatform 无 status 字段，post-create 补）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cpa_import_apply(
    platforms: Vec<MappedPlatform>,
    db: State<'_, Db>,
) -> Result<CpaBatchReport, String> {
    tracing::debug!(command = "cpa_import_apply", count = platforms.len(), "command invoked");
    let mut created = Vec::new();
    let mut failed = Vec::new();

    for p in platforms {
        let name = p.name.clone();
        let disabled = p.disabled;
        let input = CreatePlatform {
            name: p.name,
            platform_type: p.protocol,
            base_url: p.base_url,
            api_key: p.api_key,
            extra: p.extra,
            models: None,
            available_models: if p.models.is_empty() { None } else { Some(p.models) },
            endpoints: None,
            manual_budgets: None,
            auto_group: Some(true),
            join_group_ids: None,
            default_level_priority: None,
            expires_at: None,
        };
        match db::create_platform(&db, input).await {
            Ok(mut platform) => {
                if disabled {
                    // post-create 置 disabled（CreatePlatform 无 status 字段）。
                    match db::update_platform(&db, UpdatePlatform {
                        id: platform.id,
                        name: None,
                        platform_type: None,
                        base_url: None,
                        api_key: None,
                        extra: None,
                        models: None,
                        available_models: None,
                        endpoints: None,
                        enabled: None,
                        status: Some(PlatformStatus::Disabled),
                        manual_budgets: None,
                        join_group_ids: None,
                        expires_at: None,
                    }).await {
                        Ok(updated) => platform = updated,
                        Err(e) => tracing::warn!(platform_id = platform.id, error = %e, "post-create disable failed"),
                    }
                }
                tracing::info!(platform_id = platform.id, name = %platform.name, "cpa import created");
                created.push(platform);
            }
            Err(e) => {
                tracing::warn!(name = %name, error = %e, "cpa import create failed");
                failed.push(CpaBatchFailure { name, error: e });
            }
        }
    }

    tracing::info!(created = created.len(), failed = failed.len(), "cpa_import_apply done");
    Ok(CpaBatchReport { created, failed })
}

// ─── 测试 ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_report_serialize() {
        let r = CpaBatchReport { created: vec![], failed: vec![] };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"created\":[]"));
        assert!(s.contains("\"failed\":[]"));
    }

    #[test]
    fn test_parse_result_serialize() {
        let r = CpaImportParseResult {
            platforms: vec![],
            skipped: vec![],
            source_files: vec![],
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"platforms\":[]"));
    }
}
