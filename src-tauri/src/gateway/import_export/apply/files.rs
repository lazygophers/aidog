//! 文件类（codex / claude-code）写入：先备份再写。

use std::collections::BTreeMap;

use super::super::{Decision, ImportReport, Payload, Selection};
use super::{bump, is_selected, should_skip};

pub(super) fn apply_files(
    payload: &Payload,
    dec: &BTreeMap<(String, String), &Decision>,
    selection: Option<&Selection>,
    report: &mut ImportReport,
) -> Result<(), String> {
    // codex_global
    if let Some(text) = &payload.codex_global {
        let key = "codex_global".to_string();
        if is_selected(selection, super::super::SCOPE_CODEX, &key) {
            let decision = dec
                .get(&(super::super::SCOPE_CODEX.to_string(), key.clone()))
                .copied();
            if should_skip(decision) {
                bump(&mut report.skipped, super::super::SCOPE_CODEX);
            } else {
                let path = crate::gateway::codex::codex_home_public()?.join("config.toml");
                backup_and_write(&path, text, report, super::super::SCOPE_CODEX)?;
                bump(&mut report.applied, super::super::SCOPE_CODEX);
            }
        }
    }
    for nt in &payload.codex_profiles {
        let key = format!("codex_profile:{}", nt.name);
        if !is_selected(selection, super::super::SCOPE_CODEX, &key) {
            continue;
        }
        let decision = dec
            .get(&(super::super::SCOPE_CODEX.to_string(), key.clone()))
            .copied();
        if should_skip(decision) {
            bump(&mut report.skipped, super::super::SCOPE_CODEX);
            continue;
        }
        let path = crate::gateway::codex::profile_path_public(&nt.name)?;
        backup_and_write(&path, &nt.text, report, super::super::SCOPE_CODEX)?;
        bump(&mut report.applied, super::super::SCOPE_CODEX);
    }

    // claude_code_global
    if let Some(text) = &payload.claude_code_global {
        let key = "claude_code_global".to_string();
        if is_selected(selection, super::super::SCOPE_CLAUDE_CODE, &key) {
            let decision = dec
                .get(&(super::super::SCOPE_CLAUDE_CODE.to_string(), key.clone()))
                .copied();
            if should_skip(decision) {
                bump(&mut report.skipped, super::super::SCOPE_CLAUDE_CODE);
            } else {
                let home = dirs::home_dir().ok_or("cannot resolve home")?;
                let dir = home.join(".claude");
                std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir ~/.claude: {e}"))?;
                let path = dir.join("settings.json");
                backup_and_write(&path, text, report, super::super::SCOPE_CLAUDE_CODE)?;
                bump(&mut report.applied, super::super::SCOPE_CLAUDE_CODE);
            }
        }
    }
    for nt in &payload.claude_code_group_settings {
        let key = format!("claude_code_group:{}", nt.name);
        if !is_selected(selection, super::super::SCOPE_CLAUDE_CODE, &key) {
            continue;
        }
        let decision = dec
            .get(&(super::super::SCOPE_CLAUDE_CODE.to_string(), key.clone()))
            .copied();
        if should_skip(decision) {
            bump(&mut report.skipped, super::super::SCOPE_CLAUDE_CODE);
            continue;
        }
        let home = dirs::home_dir().ok_or("cannot resolve home")?;
        let dir = home.join(".aidog");
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir ~/.aidog: {e}"))?;
        let path = dir.join(format!("settings.{}.json", nt.name));
        backup_and_write(&path, &nt.text, report, super::super::SCOPE_CLAUDE_CODE)?;
        bump(&mut report.applied, super::super::SCOPE_CLAUDE_CODE);
    }
    Ok(())
}

/// 写文件前备份原文件到 `<path>.aidogbak`（若存在）。
fn backup_and_write(
    path: &std::path::Path,
    text: &str,
    report: &mut ImportReport,
    scope: &str,
) -> Result<(), String> {
    if path.exists() {
        let bak = path.with_extension(format!(
            "{}.aidogbak",
            path.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        if let Err(e) = std::fs::copy(path, &bak) {
            report
                .errors
                .push(format!("backup {} → {:?}: {e}", path.display(), bak));
        }
    }
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
    let _ = scope;
    Ok(())
}
