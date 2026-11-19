//! Read-only watcher for Claude Code plugin configuration changes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

struct WatchState {
    watcher: RecommendedWatcher,
    paths: HashSet<PathBuf>,
}

static WATCHER: OnceLock<Mutex<Option<WatchState>>> = OnceLock::new();

fn watch_paths(project: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    let claude_dir = super::plugin_bundle::claude_dir()?;
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let mut paths = Vec::new();
    if claude_dir.is_dir() {
        paths.push(claude_dir.clone());
        let plugins = claude_dir.join("plugins");
        if plugins.is_dir() {
            paths.push(plugins);
        }
    } else {
        // Observe ~/.claude creation without watching the whole home tree recursively.
        paths.push(home);
    }
    if let Some(project) = project {
        let settings = project.join(".claude");
        paths.push(if settings.is_dir() {
            settings
        } else {
            project.to_path_buf()
        });
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn relevant_event(event: &notify::Event) -> bool {
    event.paths.iter().any(|path| {
        path.file_name().is_some_and(|name| {
            matches!(
                name.to_str(),
                Some("settings.json")
                    | Some("settings.local.json")
                    | Some("installed_plugins.json")
                    | Some("plugins")
                    | Some(".claude")
            )
        })
    })
}

fn create_watcher(on_change: impl Fn() + Send + 'static) -> Result<RecommendedWatcher, String> {
    notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.as_ref().is_ok_and(relevant_event) {
            on_change();
        }
    })
    .map_err(|e| format!("initialize Claude plugin watcher: {e}"))
}

/// Start and extend the process watcher. It watches only Claude settings and the current project.
pub fn ensure_plugins_watcher(project: Option<&Path>) -> Result<(), String> {
    let paths = watch_paths(project)?;
    let mutex = WATCHER.get_or_init(|| Mutex::new(None));
    let mut state = mutex
        .lock()
        .map_err(|_| "Claude plugin watcher lock poisoned".to_string())?;
    if state.is_none() {
        *state = Some(WatchState {
            watcher: create_watcher(|| {
                aidog_ctx::emit("plugins-updated", serde_json::json!({}));
            })?,
            paths: HashSet::new(),
        });
    }
    let state = state.as_mut().expect("watcher initialized");
    for path in paths {
        if state.paths.insert(path.clone()) {
            state
                .watcher
                .watch(&path, RecursiveMode::NonRecursive)
                .map_err(|e| format!("watch {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn emits_when_claude_settings_or_plugin_registry_changes() {
        let temp = tempfile::tempdir().unwrap();
        let claude = temp.path().join(".claude");
        let plugins = claude.join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = create_watcher(move || {
            let _ = tx.send(());
        })
        .unwrap();
        let mut watcher = _watcher;
        watcher.watch(&claude, RecursiveMode::NonRecursive).unwrap();
        watcher
            .watch(&plugins, RecursiveMode::NonRecursive)
            .unwrap();

        std::fs::write(claude.join("settings.json"), "{}").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("settings change event");
        std::fs::write(plugins.join("installed_plugins.json"), "{}").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("installed plugins event");
    }
}
