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

/// pi provider 的线路协议（models.json 的 `api` 字段）。
///
/// 版本后缀规则在两类协议下**是相反的**，全部以 pi 源码里的内置 provider 常量为准：
/// - `anthropic-messages`：内置 `anthropic` 的 baseUrl 是裸 host `https://api.anthropic.com`
///   （`packages/ai/src/providers/anthropic.ts:47`），`/v1/messages` 由 `@anthropic-ai/sdk` 补
///   → aidog 给**根地址，不带版本后缀**。
/// - `openai-completions` / `openai-responses`：内置 `openai` 的 baseUrl 是
///   `https://api.openai.com/v1`（`packages/ai/src/providers/openai.ts:11`）→ 带 `/v1`。
/// - `google-generative-ai`：内置 `google` 的 baseUrl 是
///   `https://generativelanguage.googleapis.com/v1beta` → 带 `/v1beta`。
///
/// ⚠️ pi 官方文档 `models.md:300-329` 的 Anthropic 代理示例写成带 `/v1`，照抄会打到
/// `/v1/v1/messages`。**文档是错的，以源码常量为准，勿把这里「修」回去。**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PiApi {
    #[default]
    AnthropicMessages,
    OpenaiCompletions,
    OpenaiResponses,
    GoogleGenerativeAi,
}

impl PiApi {
    /// models.json `api` 字段的线上取值。
    fn wire(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "anthropic-messages",
            Self::OpenaiCompletions => "openai-completions",
            Self::OpenaiResponses => "openai-responses",
            Self::GoogleGenerativeAi => "google-generative-ai",
        }
    }

    /// 解析线上取值；未知值返回 `None`。
    fn from_wire(s: &str) -> Option<Self> {
        [
            Self::AnthropicMessages,
            Self::OpenaiCompletions,
            Self::OpenaiResponses,
            Self::GoogleGenerativeAi,
        ]
        .into_iter()
        .find(|a| a.wire() == s)
    }

    /// 接在代理根地址之后的版本后缀。见类型文档里的反向规则。
    fn base_url_suffix(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "",
            Self::OpenaiCompletions | Self::OpenaiResponses => "/v1",
            Self::GoogleGenerativeAi => "/v1beta",
        }
    }
}

/// 一个分组在 pi 侧的投影。与 DB 类型解耦，`build_pi_config` 因此是纯函数。
#[derive(Debug, Clone)]
pub struct PiGroup {
    pub group_key: String,
    /// 该分组可路由到的模型 id。
    pub models: Vec<String>,
    /// 该分组的线路协议。老分组无此配置时为 `AnthropicMessages`。
    pub api: PiApi,
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

/// `group.extra` 里存协议选择的键。沿用 platform 的 extra blob 惯例，不加数据库列。
pub const EXTRA_KEY_API: &str = "pi_api";

/// 从 `group.extra` JSON 解析该组的 pi 线路协议。
/// 缺失 / 空串 / 非法值一律回落 anthropic-messages（老分组向后兼容）。
pub fn parse_group_api(extra: &str) -> PiApi {
    serde_json::from_str::<Value>(extra)
        .ok()
        .and_then(|v| v.get(EXTRA_KEY_API)?.as_str().and_then(PiApi::from_wire))
        .unwrap_or_default()
}

/// 单个分组的 provider id。
pub fn provider_id(group_key: &str) -> String {
    format!("{PROVIDER_PREFIX}{group_key}")
}

/// 构造一个分组的 pi provider 对象。`baseUrl` 的版本后缀随协议变化，规则见 [`PiApi`]。
fn build_provider(group: &PiGroup, port: u16) -> Value {
    let models: Vec<Value> = group
        .models
        .iter()
        .map(|id| serde_json::json!({ "id": id }))
        .collect();

    serde_json::json!({
        "baseUrl": format!("http://127.0.0.1:{port}/proxy{}", group.api.base_url_suffix()),
        "api": group.api.wire(),
        "apiKey": escape_pi_literal(&group.group_key),
        "authHeader": true,
        "headers": { "User-Agent": pi_user_agent() },
        // pi 默认会发三样上游未必认的东西，这里关掉（`docs/models.md` compat 表）：
        // 每工具的 `eager_input_streaming`、非官方 baseUrl 下默认带 1h ttl 的
        // `cache_control`、以及开缓存时的 `x-session-affinity` 头。
        // 转换层另做容忍，两边都兜（pi 升级改行为也不炸）。
        "compat": {
            "supportsEagerToolInputStreaming": false,
            "supportsLongCacheRetention": false,
            "sendSessionAffinityHeaders": false,
        },
        "models": models,
    })
}

/// 写进 provider `headers` 的 User-Agent。
///
/// pi 只在内置 `kimi-coding` 下才设自己的 UA，自定义 provider 会落到匿名 SDK 默认值 ——
/// 上游日志里就看不出请求来自 pi。形态照 pi 自己的
/// `getPiUserAgent`（`packages/coding-agent/src/utils/pi-user-agent.ts`：
/// `pi/<version> (<platform>; <runtime>; <arch>)`），但**去掉 version 与 runtime**：
/// 那两个值随用户升级 pi / 换 node 而变，aidog 写配置时无从得知，写死只会是假数据。
fn pi_user_agent() -> String {
    // 对齐 node 的 `process.platform` / `process.arch` 取值，而非 Rust 的裸常量。
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    };
    format!("pi ({platform}; {arch})")
}

/// aidog 要同步进 pi 全局 `settings.json` 的两项。
#[derive(Debug, Clone, Default)]
pub struct PiSettings {
    /// 默认分组的 group_key；None = 无默认组。
    pub default_group: Option<String>,
    /// 出站 HTTP 代理 URL；None = aidog 未配代理。
    pub http_proxy: Option<String>,
}

/// 生成 pi 的两份配置内容。
///
/// `existing_models` / `existing_settings` 是用户当前文件的解析结果（缺失传空对象）。
/// aidog 只增删自己前缀的 provider，pi 内置 provider 与用户自建 provider 原样保留；
/// `settings.json` 只碰 `defaultProvider` / `httpProxy` 两键，其余（含 aidog 不认识的键）原样保留。
pub fn build_pi_config(
    existing_models: &Value,
    existing_settings: &Value,
    groups: &[PiGroup],
    port: u16,
    settings: &PiSettings,
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
        settings_json: build_settings(existing_settings, settings),
    }
}

/// pi 全局 `settings.json`：只碰 `defaultProvider` / `httpProxy`，其余键原样保留。
fn build_settings(existing: &Value, settings: &PiSettings) -> Value {
    let mut root = existing.as_object().cloned().unwrap_or_else(Map::new);

    match &settings.default_group {
        Some(group_key) => {
            root.insert("defaultProvider".into(), provider_id(group_key).into());
        }
        // 取消默认组：只删 aidog 自己写的值。用户手设的 `anthropic` 等留着
        //（与 codex `remove_default_profile_from_config` 同一守卫）。
        None => {
            let ours = root
                .get("defaultProvider")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v.starts_with(PROVIDER_PREFIX));
            if ours {
                root.remove("defaultProvider");
            }
        }
    }

    // 代理只在 aidog 配了值时写。aidog 没配就不动这个键 —— 无法区分「用户手设的代理」
    // 与「aidog 上次写的代理」，贸然删会吞掉用户自己填的值。
    if let Some(proxy) = &settings.http_proxy {
        root.insert("httpProxy".into(), proxy.clone().into());
    }

    Value::Object(root)
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
pub fn sync_groups(
    groups: &[PiGroup],
    port: u16,
    settings: &PiSettings,
) -> Result<Vec<String>, String> {
    let models_file = models_path()?;
    let settings_file = settings_path()?;

    let config = build_pi_config(
        &read_json_object(&models_file)?,
        &read_json_object(&settings_file)?,
        groups,
        port,
        settings,
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

crate::tauri_command! {
/// `~/.pi/agent/models.json` 绝对路径（前端展示用）。
pub fn pi_models_path() -> Result<String, String> {
    Ok(models_path()?.to_string_lossy().to_string())
}
}

crate::tauri_command! {
/// 读 `~/.pi/agent/settings.json`。文件不存在 → 空对象（前端据此填推荐默认）。
pub fn pi_settings_read() -> Result<Value, String> {
    read_json_object(&settings_path()?)
}
}

crate::tauri_command! {
/// 整份写回 `~/.pi/agent/settings.json`。
/// 前端读的是整份文件、改的是其中几个键，因此整份写回不会丢 schema 未覆盖的键。
pub fn pi_settings_write(config: Value) -> Result<(), String> {
    write_if_changed(&settings_path()?, &config).map(|_| ())
}
}

#[cfg(test)]
#[path = "test_pi.rs"]
mod test_pi;
