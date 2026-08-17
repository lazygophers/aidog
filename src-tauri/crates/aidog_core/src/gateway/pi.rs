//! pi CLI（`earendil-works/pi`）配置生成。
//!
//! pi 的 endpoint 只认单一全局 `~/.pi/agent/models.json`（`src/config.ts:528-530`），
//! 没有项目级同名文件、没有 base URL env、没有 `--base-url` 参数（`src/cli/args.ts:90-94`）。
//! 因此每个分组映射成一个自定义 provider `aidog-<group_key>`，全部写进同一份文件，
//! 用户以 `pi --provider aidog-<group>` 切换。详见
//! `docs/adr/0001-pi-group-mapping-via-custom-providers.md`。
//!
//! token 不走 env：`apiKey` 直接写分组名 + `authHeader: true`，pi 发
//! `Authorization: Bearer <group>`。这样 `auth.json` 无法覆盖 —— 该文件按 provider id
//! 索引，`aidog-*` 不与 pi 内置 id 冲突。
//!
//! 与 `codex.rs` 的差别：codex 是每组一个 profile 文件，pi 是所有组共用一个文件，
//! 因此写入必须一次性拿到全部分组（不能在分组循环里逐个写）。

use std::path::PathBuf;

use serde_json::{Map, Value};

/// aidog 生成的 provider id 前缀。清理与「是否 aidog 所有」的判定都以它为准。
pub const PROVIDER_PREFIX: &str = "aidog-";

/// 一个分组在 pi 侧的投影。与 DB 类型解耦，`build_pi_config` 因此是纯函数。
#[derive(Debug, Clone)]
pub struct PiGroup {
    pub group_key: String,
    /// 该分组可路由到的模型 id。
    pub models: Vec<String>,
}

/// pi 两份配置文件的完整内容。写盘是外面一层薄壳，测试直接断言这里。
#[derive(Debug, Clone)]
pub struct PiConfig {
    pub models_json: Value,
    pub settings_json: Value,
}

/// `~/.pi/agent` 根目录（遵循 `PI_CODING_AGENT_DIR`，默认 `~/.pi/agent`）。
fn agent_dir() -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("PI_CODING_AGENT_DIR")
        && !custom.trim().is_empty()
    {
        return Ok(PathBuf::from(custom));
    }
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".pi").join("agent"))
}

fn models_path() -> Result<PathBuf, String> {
    Ok(agent_dir()?.join("models.json"))
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(agent_dir()?.join("settings.json"))
}

/// 把任意字符串转成 pi 值解析下的字面量。
///
/// pi 对 `apiKey` / `headers` 的值做三种解释（`docs/models.md` Value Resolution）：
/// 开头 `!` 执行 shell 命令；`$VAR` / `${VAR}` 取环境变量，**且插值在长字面量内部也生效**；
/// `$$` 转义出字面 `$`，`$!` 转义出字面 `!`。分组名由用户自由输入，含 `$` 或以 `!` 开头
/// 时不转义就会被当成命令或环境变量 —— 那是任意命令执行，不是显示问题。
fn escape_pi_literal(raw: &str) -> String {
    let dollars_escaped = raw.replace('$', "$$");
    match dollars_escaped.strip_prefix('!') {
        Some(rest) => format!("$!{rest}"),
        None => dollars_escaped,
    }
}

/// 单个分组的 provider id。
pub fn provider_id(group_key: &str) -> String {
    format!("{PROVIDER_PREFIX}{group_key}")
}

/// 构造一个分组的 pi provider 对象。
///
/// `baseUrl` 用代理根地址、**不带版本后缀** —— pi 的 anthropic 内置 provider 常量就是
/// `https://api.anthropic.com`（`packages/ai/src/providers/anthropic.ts:47`），`/v1/messages`
/// 由 `@anthropic-ai/sdk` 自己补。pi 官方文档 `models.md` 的代理示例写成带 `/v1`，那会打到
/// `/v1/v1/messages`；以源码常量为准，勿照文档「修正」。
fn build_provider(group: &PiGroup, port: u16) -> Value {
    let models: Vec<Value> = group
        .models
        .iter()
        .map(|id| serde_json::json!({ "id": id }))
        .collect();

    serde_json::json!({
        "baseUrl": format!("http://127.0.0.1:{port}/proxy"),
        "api": "anthropic-messages",
        "apiKey": escape_pi_literal(&group.group_key),
        "authHeader": true,
        "models": models,
    })
}

/// 生成 pi 的两份配置内容。
///
/// `existing_models` / `existing_settings` 是用户当前文件的解析结果（缺失传空对象）。
/// aidog 只增删自己前缀的 provider，pi 内置 provider 与用户自建 provider 原样保留；
/// `settings.json` 本票原样透传（默认分组与代理由后续票接入）。
pub fn build_pi_config(
    existing_models: &Value,
    existing_settings: &Value,
    groups: &[PiGroup],
    port: u16,
) -> PiConfig {
    let mut root = existing_models
        .as_object()
        .cloned()
        .unwrap_or_else(Map::new);

    let mut providers = root
        .get("providers")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_else(Map::new);

    // 先扫掉所有 aidog 前缀的旧 provider，再按当前分组重建 —— 删掉的分组因此自动消失，
    // 不需要单独一趟清理。用户与 pi 内置的 provider 不带前缀，不受影响。
    providers.retain(|id, _| !id.starts_with(PROVIDER_PREFIX));
    for group in groups {
        providers.insert(provider_id(&group.group_key), build_provider(group, port));
    }

    if providers.is_empty() {
        root.remove("providers");
    } else {
        root.insert("providers".to_string(), Value::Object(providers));
    }

    PiConfig {
        models_json: Value::Object(root),
        settings_json: existing_settings.clone(),
    }
}

/// 读一个 JSON 文件为对象。文件不存在或为空返回空对象；内容损坏返回 Err。
fn read_json_object(path: &PathBuf) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&content).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// 内容有变化才写。返回写入路径，未变返回 `None`。
fn write_if_changed(path: &PathBuf, value: &Value) -> Result<Option<String>, String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|e| format!("serialize {}: {e}", path.display()))?;
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    if existing == content {
        return Ok(None);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create_dir_all {}: {e}", parent.display()))?;
    }
    std::fs::write(path, &content).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(Some(path.to_string_lossy().to_string()))
}

/// 把全部分组同步进 pi 配置。返回实际发生写入的文件路径。
pub fn sync_groups(groups: &[PiGroup], port: u16) -> Result<Vec<String>, String> {
    let models_file = models_path()?;
    let settings_file = settings_path()?;

    let config = build_pi_config(
        &read_json_object(&models_file)?,
        &read_json_object(&settings_file)?,
        groups,
        port,
    );

    let mut written = Vec::new();
    if let Some(p) = write_if_changed(&models_file, &config.models_json)? {
        tracing::info!(path = %p, groups = groups.len(), "pi models.json written");
        written.push(p);
    }
    if let Some(p) = write_if_changed(&settings_file, &config.settings_json)? {
        tracing::info!(path = %p, "pi settings.json written");
        written.push(p);
    }
    Ok(written)
}

/// `~/.pi/agent/models.json` 绝对路径（前端展示用）。
#[tauri::command]
pub fn pi_models_path() -> Result<String, String> {
    Ok(models_path()?.to_string_lossy().to_string())
}

#[cfg(test)]
#[path = "test_pi.rs"]
mod test_pi;
