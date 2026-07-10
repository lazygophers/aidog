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
/// 导入导出用：codex home 公开访问器。
pub fn codex_home_public() -> Result<PathBuf, String> {
    codex_home()
}

/// 导入导出用：group profile 路径公开访问器。
pub fn profile_path_public(group: &str) -> Result<PathBuf, String> {
    profile_path(group)
}

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

/// 某分组 profile 文件 `$CODEX_HOME/<group>.config.toml` 绝对路径。
/// `codex -p <group>` 会把它层叠在用户 config.toml 之上（用户级 → 可含 model_providers）。
fn profile_path(group: &str) -> Result<PathBuf, String> {
    Ok(codex_home()?.join(format!("{group}.config.toml")))
}

/// 生成某分组的 Codex profile TOML 内容。
///
/// profile 文件用顶层键（不嵌套 `[profiles.<name>]`），层叠在用户 config 之上。
/// 注入：`model_provider="aidog"` + `[model_providers.aidog]`（base_url 指向 aidog
/// 本地代理 `/proxy`，wire_api=responses，env_key=AIDOG_KEY）。
/// aidog 按 `Authorization: Bearer $AIDOG_KEY`（值=分组名）路由。
///
/// TOML 硬约束：标量根键必须在 table 之前 —— `toml` crate 的 Table 序列化器自动满足。
pub fn build_group_profile_toml(port: u16) -> Result<String, String> {
    let mut root = toml::map::Map::new();
    root.insert(
        "model_provider".to_string(),
        toml::Value::String("aidog".to_string()),
    );

    let mut aidog = toml::map::Map::new();
    aidog.insert(
        "name".to_string(),
        toml::Value::String("aidog proxy".to_string()),
    );
    aidog.insert(
        "base_url".to_string(),
        toml::Value::String(format!("http://127.0.0.1:{port}/proxy")),
    );
    aidog.insert(
        "wire_api".to_string(),
        toml::Value::String("responses".to_string()),
    );
    aidog.insert(
        "env_key".to_string(),
        toml::Value::String("AIDOG_KEY".to_string()),
    );

    let mut providers = toml::map::Map::new();
    providers.insert("aidog".to_string(), toml::Value::Table(aidog));
    root.insert(
        "model_providers".to_string(),
        toml::Value::Table(providers),
    );

    toml::to_string_pretty(&toml::Value::Table(root))
        .map_err(|e| format!("serialize codex profile toml: {e}"))
}

/// 为单个分组写 `$CODEX_HOME/<group>.config.toml`（仅当内容变化时写）。
/// 返回写入路径（若发生写入），否则 `None`。`$CODEX_HOME` 不存在则创建。
pub fn write_group_profile(group: &str, port: u16) -> Result<Option<String>, String> {
    let dir = codex_home()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create codex home: {e}"))?;
    let path = profile_path(group)?;
    let content = build_group_profile_toml(port)?;
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing == content {
        return Ok(None);
    }
    std::fs::write(&path, &content).map_err(|e| {
        tracing::error!(group, path = %path.display(), error = %e, "write codex group profile failed");
        format!("write codex profile {group}: {e}")
    })?;
    tracing::info!(group, path = %path.display(), "codex group profile written");
    Ok(Some(path.to_string_lossy().to_string()))
}

/// 清理已删除分组的 profile 文件：移除 `$CODEX_HOME/<name>.config.toml` 中
/// `<name>` 不在 `keep` 集合内者。`config.toml`（用户级基线）永不清理。
pub fn cleanup_group_profiles(keep: &std::collections::HashSet<String>) -> Result<(), String> {
    let dir = codex_home()?;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // 仅匹配 `<group>.config.toml`，排除用户级基线 `config.toml`。
        if name == "config.toml" {
            continue;
        }
        if let Some(group) = name.strip_suffix(".config.toml") {
            if !group.is_empty() && !keep.contains(group) {
                if let Err(e) = std::fs::remove_file(entry.path()) {
                    tracing::warn!(group, path = %entry.path().display(), error = %e, "cleanup codex profile: remove failed");
                } else {
                    tracing::debug!(group, "cleanup codex profile: removed stale profile");
                }
            }
        }
    }
    Ok(())
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
    let content = std::fs::read_to_string(&path).map_err(|e| {
        tracing::warn!(path = %path.display(), error = %e, "codex_config_read: read file failed");
        format!("read config.toml: {e}")
    })?;
    if content.trim().is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let toml_value: toml::Value = toml::from_str(&content).map_err(|e| {
        tracing::warn!(path = %path.display(), error = %e, "codex_config_read: parse toml failed");
        format!("parse config.toml: {e}")
    })?;
    // toml::Value → serde_json::Value（serde 桥接，类型一一对应）。
    let json_value = serde_json::to_value(&toml_value).map_err(|e| {
        tracing::warn!(error = %e, "codex_config_read: toml→json failed");
        format!("toml→json: {e}")
    })?;
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
    let toml_value: toml::Value = serde_json::from_value(cleaned).map_err(|e| {
        tracing::warn!(error = %e, "codex_config_write: json→toml failed");
        format!("json→toml: {e}")
    })?;
    // Table 序列化器自动把标量排在 table 之前，满足 TOML 根键约束。
    let toml_str = toml::to_string_pretty(&toml_value).map_err(|e| {
        tracing::error!(error = %e, "codex_config_write: serialize toml failed");
        format!("serialize toml: {e}")
    })?;

    let dir = codex_home()?;
    std::fs::create_dir_all(&dir).map_err(|e| {
        tracing::error!(dir = %dir.display(), error = %e, "codex_config_write: create dir failed");
        format!("create ~/.codex: {e}")
    })?;
    let path = dir.join("config.toml");
    std::fs::write(&path, toml_str).map_err(|e| {
        tracing::error!(path = %path.display(), error = %e, "codex_config_write: write file failed");
        format!("write config.toml: {e}")
    })?;
    tracing::info!(path = %path.display(), "codex config written");
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

/// 默认分组：把 aidog profile 注入用户全局 `~/.codex/config.toml`。
///
/// 注入内容（与 `build_group_profile_toml` 等价，但 merge 进用户级 config 而非
/// 独立 profile 文件）：
///   - 顶层 `model_provider = "aidog"`
///   - `[model_providers.aidog]`（name / base_url / wire_api=responses / env_key=AIDOG_KEY）
///
/// merge 策略：用户已有的 model_providers / 顶层键保留（除非冲突 aidog 键，则覆盖）。
///
/// **Codex 固有限制**：profile 的 `env_key="AIDOG_KEY"` 让 codex 从环境变量读 token，
/// 全局 config.toml 无法内联 token —— 用户须在 shell `export AIDOG_KEY=<group_key>`。
/// UI 默认组提示文案需说明此限制（与 Claude Code 全局免 env 不同）。
///
/// 返回 Some(path) 表示发生写入；None 表示内容未变跳过。
pub fn write_default_profile_to_config(port: u16) -> Result<Option<String>, String> {
    let mut config = codex_config_read()?;

    let aidog_profile = serde_json::json!({
        "name": "aidog proxy",
        "base_url": format!("http://127.0.0.1:{port}/proxy"),
        "wire_api": "responses",
        "env_key": "AIDOG_KEY",
    });

    // 写 model_provider（顶层标量，覆盖任何现有 aidog/其它 provider 选择）
    set_obj_path(&mut config, &["model_provider"], serde_json::Value::String("aidog".into()));
    // merge [model_providers.aidog]
    set_obj_path(&mut config, &["model_providers", "aidog"], aidog_profile);

    let before = match codex_config_read() {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
        Err(_) => String::new(),
    };
    let after = serde_json::to_string_pretty(&config).unwrap_or_default();
    if before == after {
        return Ok(None);
    }

    codex_config_write(config)?;
    let path = config_path()?;
    tracing::info!(path = %path.display(), "codex default profile merged into config.toml");
    Ok(Some(path.to_string_lossy().to_string()))
}

/// 默认分组取消时从 `~/.codex/config.toml` 移除 aidog 默认 profile。
///
/// 仅清除 aidog 注入的 3 处标识：顶层 `model_provider` 若为 "aidog" 则删除、
/// `[model_providers.aidog]` 整块删除。用户其它字段一律保留。
/// 不删 model_providers 表本身（用户可能有其它自定义 provider）。
pub fn remove_default_profile_from_config() -> Result<Option<String>, String> {
    let mut config = codex_config_read()?;

    let mut changed = false;
    // 顶层 model_provider == "aidog" → 移除（用户手动设的其它值保留）
    if config.get("model_provider").and_then(|v| v.as_str()) == Some("aidog") {
        if let Some(obj) = config.as_object_mut() {
            obj.remove("model_provider");
        }
        changed = true;
    }
    // [model_providers.aidog] → 整块删
    if let Some(providers) = config.get_mut("model_providers").and_then(|v| v.as_object_mut()) {
        if providers.remove("aidog").is_some() {
            changed = true;
        }
        // 空 providers 表也清理（避免留下空 [model_providers]）
        if providers.is_empty() {
            if let Some(obj) = config.as_object_mut() {
                obj.remove("model_providers");
            }
        }
    }

    if !changed {
        return Ok(None);
    }

    let path = config_path()?;
    codex_config_write(config)?;
    tracing::info!(path = %path.display(), "codex default profile removed from config.toml");
    Ok(Some(path.to_string_lossy().to_string()))
}

/// JSON pointer 写入：按路径在 base 下深定位，叶子覆盖（非 object 直接替换）。
fn set_obj_path(base: &mut serde_json::Value, path: &[&str], value: serde_json::Value) {
    if path.is_empty() {
        *base = value;
        return;
    }
    let head = path[0];
    if !base.is_object() {
        *base = serde_json::Value::Object(serde_json::Map::new());
    }
    let obj = base.as_object_mut().expect("ensured object");
    if path.len() == 1 {
        obj.insert(head.to_string(), value);
        return;
    }
    let entry = obj.entry(head.to_string()).or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    set_obj_path(entry, &path[1..], value);
}

#[cfg(test)]
#[path = "test_codex.rs"]
mod test_codex;
