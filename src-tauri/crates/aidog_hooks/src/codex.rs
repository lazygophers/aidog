//! Codex `config.toml` notify 注入 / 移除。
//!
//! Codex `notify` 是顶层字符串数组，进程会以脚本 + 事件 JSON 调用。复用 claude_code 子模块的
//! `ensure_object` / `references_aidog_script` helper（pub(crate)）做对象保障与 aidog 识别。

use super::claude_code::{ensure_object, references_aidog_script};
use serde_json::{json, Value};

/// 在 Codex `config.toml` 的 JSON 视图中注入顶层 `notify`。
///
/// Codex `notify` 是顶层字符串数组，进程会以脚本 + 事件 JSON 调用。
/// 任务完成事件由 Codex `agent-turn-complete` 触发 → 指向 complete 脚本。
/// （Codex 当前仅 turn-complete 事件，等待输入无独立 notify event，故仅注入 complete。）
pub fn inject_codex_notify(config: &mut Value, complete_script: &str) {
    let obj = ensure_object(config);
    obj.insert("notify".to_string(), json!([complete_script]));
}

/// 移除 Codex 中 aidog 注入的 `notify`（仅当其指向 aidog 脚本时；保留用户自定义 notify）。
pub fn remove_codex_notify(config: &mut Value) {
    let Some(obj) = config.as_object_mut() else { return };
    let is_aidog = obj
        .get("notify")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter().any(|v| {
                v.as_str()
                    .map(references_aidog_script)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if is_aidog {
        obj.remove("notify");
    }
}

#[cfg(test)]
mod test_codex;
