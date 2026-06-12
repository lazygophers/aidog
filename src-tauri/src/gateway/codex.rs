//! Codex CLI 全局配置（`~/.codex/config.toml`）读写。
//!
//! 前端用 JSON 编辑，后端在 JSON ↔ TOML 之间往返：
//! - 读：解析 `config.toml` → `toml::Value` → `serde_json::Value` 给前端。
//! - 写：前端 JSON → `toml::Value` → 序列化为 TOML 文件。
//!
//! TOML 硬约束：根级（top-level）标量键必须出现在所有 table（`[xxx]`）之前。
//! `toml` crate 的 `Value` 序列化器会自动把非 table 值排在 table 之前，
//! 因此只要先归一化成 `toml::Value::Table` 再 `toml::to_string` 即合法。
//! 未知键（前端 schema 未覆盖的）会原样保留在 JSON 往返中，不丢失。

use std::path::PathBuf;

/// 解析 `~/.codex` 根目录（遵循 `CODEX_HOME` 环境变量，默认 `~/.codex`）。
fn codex_home() -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("CODEX_HOME") {
        if !custom.trim().is_empty() {
            return Ok(PathBuf::from(custom));
        }
    }
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".codex"))
}

/// `~/.codex/config.toml` 绝对路径。
fn config_path() -> Result<PathBuf, String> {
    Ok(codex_home()?.join("config.toml"))
}

/// 返回 `~/.codex/config.toml` 的绝对路径（字符串），供前端展示。
#[tauri::command]
pub fn codex_config_path() -> Result<String, String> {
    Ok(config_path()?.to_string_lossy().to_string())
}

/// 读取 `~/.codex/config.toml` 并转为 JSON。
/// 文件不存在 → 返回空对象 `{}`（前端据此填充推荐默认）。
#[tauri::command]
pub fn codex_config_read() -> Result<serde_json::Value, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("read config.toml: {e}"))?;
    if content.trim().is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let toml_value: toml::Value =
        toml::from_str(&content).map_err(|e| format!("parse config.toml: {e}"))?;
    // toml::Value → serde_json::Value（serde 桥接，类型一一对应）。
    let json_value =
        serde_json::to_value(&toml_value).map_err(|e| format!("toml→json: {e}"))?;
    Ok(json_value)
}

/// 将前端 JSON 写回 `~/.codex/config.toml`（先转 TOML）。
/// `~/.codex/` 不存在则创建。已知/未知键经 JSON 往返尽量保留。
#[tauri::command]
pub fn codex_config_write(value: serde_json::Value) -> Result<(), String> {
    // 顶层必须是对象，否则不是合法的 TOML 文档。
    if !value.is_object() {
        return Err("codex config must be a JSON object".into());
    }
    // serde_json::Value → toml::Value。toml::Value 不支持 null，
    // 写入前剔除值为 null 的键（前端清空字段时表现为删除）。
    let cleaned = strip_nulls(value);
    let toml_value: toml::Value =
        serde_json::from_value(cleaned).map_err(|e| format!("json→toml: {e}"))?;
    // Table 序列化器自动把标量排在 table 之前，满足 TOML 根键约束。
    let toml_str = toml::to_string_pretty(&toml_value).map_err(|e| format!("serialize toml: {e}"))?;

    let dir = codex_home()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create ~/.codex: {e}"))?;
    let path = dir.join("config.toml");
    std::fs::write(&path, toml_str).map_err(|e| format!("write config.toml: {e}"))?;
    Ok(())
}

/// 递归剔除 JSON 中值为 null 的键与数组元素（toml::Value 无 null 表示）。
fn strip_nulls(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let cleaned: serde_json::Map<String, serde_json::Value> = map
                .into_iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k, strip_nulls(v)))
                .collect();
            serde_json::Value::Object(cleaned)
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.into_iter()
                .filter(|v| !v.is_null())
                .map(strip_nulls)
                .collect(),
        ),
        other => other,
    }
}
