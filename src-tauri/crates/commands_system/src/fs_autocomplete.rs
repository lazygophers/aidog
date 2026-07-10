use aidog_core::gateway::{self};
#[allow(unused_imports)]
use aidog_core::logging;
#[allow(unused_imports)]
use gateway::models::*;
#[allow(unused_imports)]
use tauri::State;
#[allow(unused_imports)]
use serde_json::Value;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use tauri::Manager;


use serde::Serialize;

#[derive(Serialize)]
pub struct PathEntry {
    name: String,
    full_path: String,
    is_dir: bool,
    /// Unix timestamp (seconds)
    modified: i64,
}

/// Expand `~` to home directory and resolve path
pub(crate) fn expand_path(input: &str) -> std::path::PathBuf {
    if input.starts_with("~/") || input == "~" {
        if let Some(home) = dirs::home_dir() {
            if input == "~" {
                return home;
            }
            return home.join(&input[2..]);
        }
    }
    std::path::PathBuf::from(input)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub fn fs_autocomplete(input: String) -> Result<Vec<PathEntry>, String> {
    tracing::debug!(command = "fs_autocomplete", "command invoked");
    let path = expand_path(input.trim());

    // Determine parent dir and prefix filter
    let (parent, prefix) = if input.ends_with('/') || input == "~" || input.ends_with('~') {
        (path.clone(), "".to_string())
    } else {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
        });
        (parent, file_name)
    };

    if !parent.exists() || !parent.is_dir() {
        return Ok(vec![]);
    }

    let entries: Vec<PathEntry> = std::fs::read_dir(&parent)
        .map_err(|e| { tracing::warn!(command = "fs_autocomplete", error = %e, "read_dir failed"); e.to_string() })?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Filter by prefix
            if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                return None;
            }

            let metadata = entry.metadata().ok()?;
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let full_path = entry.path().to_string_lossy().to_string();

            Some(PathEntry {
                name,
                full_path,
                is_dir: metadata.is_dir(),
                modified,
            })
        })
        .collect();

    // Sort: directories first, then by modification time descending
    let mut sorted = entries;
    sorted.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.modified.cmp(&a.modified),
        }
    });

    // Limit results
    sorted.truncate(20);

    Ok(sorted)
}

#[cfg(test)]
#[path = "test_fs_autocomplete.rs"]
mod test_fs_autocomplete;
