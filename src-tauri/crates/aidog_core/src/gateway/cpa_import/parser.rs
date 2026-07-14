//! CPA(CLIProxyAPI) 配置解析器。
//!
//! 参考：router-for-me/CLIProxyAPI config.example.yaml

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use tempfile::TempDir;
use tracing::{info, warn};

// ─── 数据结构 ───────────────────────────────────────────────────────

/// CPA 配置段来源类型（对应 config.yaml 顶层 key 或 OAuth channel）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpaSourceSegment {
    /// `gemini-api-key` 段
    GeminiApiKey,
    /// `interactions-api-key` 段
    InteractionsApiKey,
    /// `codex-api-key` 段
    CodexApiKey,
    /// `claude-api-key` 段
    ClaudeApiKey,
    /// `openai-compatibility` 段
    OpenaiCompatibility,
    /// `vertex-api-key` 段
    VertexApiKey,
    /// OAuth channel（来自 auth-dir JSON）
    OAuth,
}

impl CpaSourceSegment {
    /// 返回 YAML 顶层 key 字符串。
    pub fn yaml_key(&self) -> &'static str {
        match self {
            CpaSourceSegment::GeminiApiKey => "gemini-api-key",
            CpaSourceSegment::InteractionsApiKey => "interactions-api-key",
            CpaSourceSegment::CodexApiKey => "codex-api-key",
            CpaSourceSegment::ClaudeApiKey => "claude-api-key",
            CpaSourceSegment::OpenaiCompatibility => "openai-compatibility",
            CpaSourceSegment::VertexApiKey => "vertex-api-key",
            CpaSourceSegment::OAuth => "oauth",
        }
    }

    /// 所有 CPA 段的 YAML key 集合（用于识别 config.yaml）。
    pub fn all_keys() -> &'static [&'static str] {
        &[
            "gemini-api-key",
            "interactions-api-key",
            "codex-api-key",
            "claude-api-key",
            "openai-compatibility",
            "vertex-api-key",
        ]
    }
}

/// OAuth 凭据类型（auth-dir JSON 中的 `type` 字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpaOAuthType {
    Codex,
    Claude,
    Kimi,
    Xai,
    Vertex,
    Aistudio,
    Antigravity,
}

impl CpaOAuthType {
    /// 从字符串解析（容错：不区分大小写）。
    pub fn parse_oauth_type(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "codex" => Some(CpaOAuthType::Codex),
            "claude" => Some(CpaOAuthType::Claude),
            "kimi" => Some(CpaOAuthType::Kimi),
            "xai" => Some(CpaOAuthType::Xai),
            "vertex" => Some(CpaOAuthType::Vertex),
            "aistudio" => Some(CpaOAuthType::Aistudio),
            "antigravity" => Some(CpaOAuthType::Antigravity),
            _ => None,
        }
    }
}

/// 从单个 config 文件解析出的 Provider（映射前的中间表示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpaProvider {
    /// 来源段类型
    pub source_segment: CpaSourceSegment,
    /// provider 名称（openai-compatibility 段有此字段；其他段可能为空）
    pub name: Option<String>,
    /// 上游 base URL
    pub base_url: String,
    /// API key（明文）
    pub api_key: String,
    /// 模型列表（来自 `models[].name`，alias 丢弃）
    pub models: Vec<String>,
    /// 模型前缀（可选）
    pub prefix: Option<String>,
    /// 自定义请求头（可选）
    pub headers: HashMap<String, String>,
    /// 是否禁用（来自 `disabled` 字段）
    pub disabled: bool,
    /// OAuth 类型（仅 OAuth 来源）
    pub oauth_type: Option<CpaOAuthType>,
}

/// 跳过文件的原因。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipReason {
    /// 文件路径
    pub path: String,
    /// 跳过原因类型
    pub reason: String,
}

/// 解析结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    /// 解析出的 provider 列表（已去重合并）
    pub providers: Vec<CpaProvider>,
    /// 跳过的文件列表（含原因）
    pub skipped: Vec<SkipReason>,
    /// 成功解析的源文件列表
    pub source_files: Vec<String>,
}

// ─── YAML/JSON 识别 ───────────────────────────────────────────────────

/// CPA config YAML/JSON 的最小结构（用于识别是否为 CPA 配置）。
#[derive(Deserialize)]
struct CpaConfigStub {
    #[serde(default)]
    gemini_api_key: Option<Value>,
    #[serde(default)]
    interactions_api_key: Option<Value>,
    #[serde(default)]
    codex_api_key: Option<Value>,
    #[serde(default)]
    claude_api_key: Option<Value>,
    #[serde(default)]
    openai_compatibility: Option<Value>,
    #[serde(default)]
    vertex_api_key: Option<Value>,
}

impl CpaConfigStub {
    /// 判断此 Value 是否含任一 CPA 段。
    fn is_cpa_config(&self) -> bool {
        self.gemini_api_key.is_some()
            || self.interactions_api_key.is_some()
            || self.codex_api_key.is_some()
            || self.claude_api_key.is_some()
            || self.openai_compatibility.is_some()
            || self.vertex_api_key.is_some()
    }
}

// ─── 单文件解析 ───────────────────────────────────────────────────────

/// 解析单个 YAML/JSON 文件，返回 CpaProvider 列表。
fn parse_single_file(path: &Path) -> Result<Vec<CpaProvider>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("读取文件失败: {e}"))?;

    let ext = path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // JSON 扩展名：先试 OAuth 凭据（CLIProxyAPI OAuth 凭据均 JSON 单文件）
    if ext == "json" {
        if let Some(oauth_providers) = parse_oauth_json(&content) {
            return Ok(oauth_providers);
        }
        // 是 OAuth 凭据但 parse_oauth_json None = 缺 access_token → 独立文案
        if is_oauth_credential(&content) {
            return Err("OAuth 凭据缺少 access_token".to_string());
        }
    }

    let stub: CpaConfigStub = if ext == "yaml" || ext == "yml" {
        serde_yaml::from_str(&content)
            .map_err(|e| format!("YAML 解析失败: {e}"))?
    } else if ext == "json" {
        serde_json::from_str(&content)
            .map_err(|e| format!("JSON 解析失败: {e}"))?
    } else {
        return Err(format!("不支持的文件类型: {ext}"));
    };

    if !stub.is_cpa_config() {
        return Err("不是 CPA 配置文件（无任何 CPA provider 段）".to_string());
    }

    let mut providers = Vec::new();

    // 解析各段
    if let Some(arr) = stub.gemini_api_key {
        providers.extend(parse_gemini_array(arr, CpaSourceSegment::GeminiApiKey));
    }
    if let Some(arr) = stub.interactions_api_key {
        providers.extend(parse_gemini_array(arr, CpaSourceSegment::InteractionsApiKey));
    }
    if let Some(arr) = stub.codex_api_key {
        providers.extend(parse_codex_array(arr));
    }
    if let Some(arr) = stub.claude_api_key {
        providers.extend(parse_claude_array(arr));
    }
    if let Some(arr) = stub.openai_compatibility {
        providers.extend(parse_openai_compat_array(arr));
    }
    if let Some(arr) = stub.vertex_api_key {
        providers.extend(parse_vertex_array(arr));
    }

    Ok(providers)
}

/// 解析 `gemini-api-key` 或 `interactions-api-key` 数组（两者结构相同）。
fn parse_gemini_array(arr: Value, segment: CpaSourceSegment) -> Vec<CpaProvider> {
    let mut providers = Vec::new();
    if let Some(arr) = arr.as_array() {
        for item in arr {
            if let Some(obj) = item.as_object() {
                let api_key = obj.get("api-key").and_then(|v| v.as_str()).unwrap_or("");
                let base_url = obj.get("base-url").and_then(|v| v.as_str()).unwrap_or("");
                if api_key.is_empty() {
                    continue;
                }
                let models = parse_models(obj.get("models"));
                let prefix = obj.get("prefix").and_then(|v| v.as_str()).map(String::from);
                let headers = parse_headers(obj.get("headers"));
                providers.push(CpaProvider {
                    source_segment: segment,
                    name: None,
                    base_url: base_url.to_string(),
                    api_key: api_key.to_string(),
                    models,
                    prefix,
                    headers,
                    disabled: false,
                    oauth_type: None,
                });
            }
        }
    }
    providers
}

/// 解析 `codex-api-key` 数组。
fn parse_codex_array(arr: Value) -> Vec<CpaProvider> {
    let mut providers = Vec::new();
    if let Some(arr) = arr.as_array() {
        for item in arr {
            if let Some(obj) = item.as_object() {
                let api_key = obj.get("api-key").and_then(|v| v.as_str()).unwrap_or("");
                let base_url = obj.get("base-url").and_then(|v| v.as_str()).unwrap_or("");
                if api_key.is_empty() {
                    continue;
                }
                let models = parse_models(obj.get("models"));
                let prefix = obj.get("prefix").and_then(|v| v.as_str()).map(String::from);
                let headers = parse_headers(obj.get("headers"));
                providers.push(CpaProvider {
                    source_segment: CpaSourceSegment::CodexApiKey,
                    name: None,
                    base_url: base_url.to_string(),
                    api_key: api_key.to_string(),
                    models,
                    prefix,
                    headers,
                    disabled: false,
                    oauth_type: None,
                });
            }
        }
    }
    providers
}

/// 解析 `claude-api-key` 数组。
fn parse_claude_array(arr: Value) -> Vec<CpaProvider> {
    let mut providers = Vec::new();
    if let Some(arr) = arr.as_array() {
        for item in arr {
            if let Some(obj) = item.as_object() {
                let api_key = obj.get("api-key").and_then(|v| v.as_str()).unwrap_or("");
                let base_url = obj.get("base-url").and_then(|v| v.as_str()).unwrap_or("");
                if api_key.is_empty() {
                    continue;
                }
                let models = parse_models(obj.get("models"));
                let prefix = obj.get("prefix").and_then(|v| v.as_str()).map(String::from);
                let headers = parse_headers(obj.get("headers"));
                providers.push(CpaProvider {
                    source_segment: CpaSourceSegment::ClaudeApiKey,
                    name: None,
                    base_url: base_url.to_string(),
                    api_key: api_key.to_string(),
                    models,
                    prefix,
                    headers,
                    disabled: false,
                    oauth_type: None,
                });
            }
        }
    }
    providers
}

/// 解析 `openai-compatibility` 数组。
fn parse_openai_compat_array(arr: Value) -> Vec<CpaProvider> {
    let mut providers = Vec::new();
    if let Some(arr) = arr.as_array() {
        for item in arr {
            if let Some(obj) = item.as_object() {
                let base_url = obj.get("base-url").and_then(|v| v.as_str()).unwrap_or("");
                let name = obj.get("name").and_then(|v| v.as_str()).map(String::from);
                let disabled = obj.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);

                // api-key-entries 数组，取首个
                let api_key = if let Some(entries) = obj.get("api-key-entries").and_then(|v| v.as_array()) {
                    entries.iter()
                        .filter_map(|e| e.get("api-key").and_then(|v| v.as_str()))
                        .next()
                        .unwrap_or("")
                } else {
                    ""
                };

                if api_key.is_empty() {
                    continue;
                }

                let models = parse_models(obj.get("models"));
                let prefix = obj.get("prefix").and_then(|v| v.as_str()).map(String::from);
                let headers = parse_headers(obj.get("headers"));
                providers.push(CpaProvider {
                    source_segment: CpaSourceSegment::OpenaiCompatibility,
                    name,
                    base_url: base_url.to_string(),
                    api_key: api_key.to_string(),
                    models,
                    prefix,
                    headers,
                    disabled,
                    oauth_type: None,
                });
            }
        }
    }
    providers
}

/// 解析 `vertex-api-key` 数组。
fn parse_vertex_array(arr: Value) -> Vec<CpaProvider> {
    let mut providers = Vec::new();
    if let Some(arr) = arr.as_array() {
        for item in arr {
            if let Some(obj) = item.as_object() {
                let api_key = obj.get("api-key").and_then(|v| v.as_str()).unwrap_or("");
                let base_url = obj.get("base-url").and_then(|v| v.as_str()).unwrap_or("");
                if api_key.is_empty() {
                    continue;
                }
                let models = parse_models(obj.get("models"));
                let prefix = obj.get("prefix").and_then(|v| v.as_str()).map(String::from);
                let headers = parse_headers(obj.get("headers"));
                providers.push(CpaProvider {
                    source_segment: CpaSourceSegment::VertexApiKey,
                    name: None,
                    base_url: base_url.to_string(),
                    api_key: api_key.to_string(),
                    models,
                    prefix,
                    headers,
                    disabled: false,
                    oauth_type: None,
                });
            }
        }
    }
    providers
}

/// 解析 `models` 数组，提取模型名（丢弃 alias）。
fn parse_models(models_val: Option<&Value>) -> Vec<String> {
    let mut model_names = Vec::new();
    if let Some(arr) = models_val.and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(obj) = item.as_object()
                && let Some(name) = obj.get("name").and_then(|v| v.as_str())
            {
                model_names.push(name.to_string());
            }
        }
    }
    model_names
}

/// 解析 `headers` 对象。
fn parse_headers(headers_val: Option<&Value>) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    if let Some(obj) = headers_val.and_then(|v| v.as_object()) {
        for (k, v) in obj.iter() {
            if let Some(s) = v.as_str() {
                headers.insert(k.clone(), s.to_string());
            }
        }
    }
    headers
}

// ─── 压缩解压 ───────────────────────────────────────────────────────

/// 解压 ZIP 文件，返回临时目录路径。
fn unzip_archive(zip_path: &Path) -> Result<TempDir, String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("打开 ZIP 失败: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("ZIP 解析失败: {e}"))?;

    let temp_dir = TempDir::new()
        .map_err(|e| format!("创建临时目录失败: {e}"))?;

    archive.extract(temp_dir.path())
        .map_err(|e| format!("ZIP 解压失败: {e}"))?;

    Ok(temp_dir)
}

/// 解压 TAR/TAR.GZ/TGZ 文件，返回临时目录路径。
fn untar_archive(tar_path: &Path) -> Result<TempDir, String> {
    let file = fs::File::open(tar_path)
        .map_err(|e| format!("打开 TAR 失败: {e}"))?;

    let temp_dir = TempDir::new()
        .map_err(|e| format!("创建临时目录失败: {e}"))?;

    let is_gz = tar_path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s == "gz" || tar_path.file_stem()
            .and_then(|st| st.to_str())
            .map(|st| st.ends_with(".tar"))
            .unwrap_or(false))
        .unwrap_or(false);

    if is_gz {
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(temp_dir.path())
            .map_err(|e| format!("TAR.GZ 解压失败: {e}"))?;
    } else {
        let mut archive = tar::Archive::new(file);
        archive.unpack(temp_dir.path())
            .map_err(|e| format!("TAR 解压失败: {e}"))?;
    }

    Ok(temp_dir)
}

/// 判断路径是否为支持的压缩文件。
fn is_supported_archive(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        match ext {
            "zip" => return true,
            "gz" => return true,
            "tgz" => return true,
            "tar" => {
                // .tar 无扩展名，检查文件名
                return path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|stem| !stem.ends_with(".tar"))
                    .unwrap_or(true);
            }
            _ => return false,
        }
    }
    // 检查 .tar 结尾（无扩展名）
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.ends_with(".tar"))
        .unwrap_or(false)
}

/// 判断路径是否为不支持的压缩文件（rar/7z）。
fn is_unsupported_archive(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        matches!(ext, "rar" | "7z" | "xz" | "bz" | "bz2")
    } else {
        false
    }
}

// ─── auth-dir OAuth 扫描 ───────────────────────────────────────────────

/// auth-dir JSON 凭据结构。
#[derive(Deserialize)]
struct OAuthCredential {
    #[serde(rename = "type")]
    cred_type: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    /// refresh_token 解析但丢弃（CPA 导入策略：access_token 当 api_key，refresh 不持久化，
    /// 过期由用户手补。serde 字段保留以容错解析含该字段的 JSON）。见 design.md OAuth 取舍。
    #[serde(default)]
    #[allow(dead_code)]
    refresh_token: Option<String>,
    #[serde(default)]
    model_aliases: Option<Vec<OAuthModelAlias>>,
}

#[derive(Deserialize)]
struct OAuthModelAlias {
    #[serde(default)]
    name: String,
    /// alias 解析但未用（CPA 不导入上游 alias 映射；preset model_list 给默认模型集）。
    /// serde 字段保留以容错解析含该字段的 JSON。
    #[serde(default)]
    #[allow(dead_code)]
    alias: String,
}

/// 解析 JSON 内容为 OAuth provider 列表（单文件/单凭据，Vec 长度 0 或 1）。
/// 返回 None = 不是 OAuth 凭据或缺少 access_token（交回 CPA config 流程或跳过）。
fn parse_oauth_json(content: &str) -> Option<Vec<CpaProvider>> {
    let cred: OAuthCredential = serde_json::from_str(content).ok()?;
    let oauth_type = CpaOAuthType::parse_oauth_type(&cred.cred_type)?;
    let access_token = cred.access_token?;
    let models: Vec<String> = cred.model_aliases
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.name)
        .collect();
    Some(vec![CpaProvider {
        source_segment: CpaSourceSegment::OAuth,
        name: cred.email.clone(),
        base_url: String::new(), // OAuth 平台 base_url 由后续映射确定
        api_key: access_token,
        models,
        prefix: None,
        headers: HashMap::new(),
        disabled: false,
        oauth_type: Some(oauth_type),
    }])
}

/// 探测 JSON 是否为 OAuth 凭据（仅看 `type` 能否识别为 CpaOAuthType，不要求 access_token）。
/// 供 parse_single_file 区分「非 OAuth JSON」vs「OAuth 缺 token」给独立错误文案。
fn is_oauth_credential(content: &str) -> bool {
    #[derive(Deserialize)]
    struct TypeProbe {
        #[serde(rename = "type")]
        t: String,
    }
    serde_json::from_str::<TypeProbe>(content)
        .ok()
        .and_then(|p| CpaOAuthType::parse_oauth_type(&p.t))
        .is_some()
}

/// 递归扫描 auth-dir 目录，解析 OAuth JSON 凭据。
fn scan_auth_dir(auth_dir: &Path) -> Vec<CpaProvider> {
    let mut providers = Vec::new();

    if !auth_dir.exists() {
        warn!("auth-dir 不存在: {}", auth_dir.display());
        return providers;
    }

    let entries = match fs::read_dir(auth_dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("读取 auth-dir 失败: {e}");
            return providers;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // 递归子目录
            providers.extend(scan_auth_dir(&path));
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            // 解析 JSON 凭据（无 token / 非 OAuth → 静默跳过，同原行为）
            if let Ok(content) = fs::read_to_string(&path)
                && let Some(oauth_providers) = parse_oauth_json(&content)
            {
                providers.extend(oauth_providers);
            }
        }
    }

    providers
}

// ─── 递归扫描目录 ───────────────────────────────────────────────────

/// 递归扫描目录，收集所有 YAML/JSON 文件。
fn collect_config_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return files,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_config_files(&path));
        } else if let Some(ext) = path.extension().and_then(|s| s.to_str())
            && matches!(ext, "yaml" | "yml" | "json")
        {
            files.push(path);
        }
    }

    files
}

// ─── 去重合并 ───────────────────────────────────────────────────────

/// 对 CpaProvider 列表进行去重合并。
/// 规则：
/// - openai-compatibility: 按 name 去重
/// - 其他段: 按 source_segment + base_url 去重
/// - 保留首个，api_key 取首，models 并集去重
fn deduplicate_providers(providers: Vec<CpaProvider>) -> Vec<CpaProvider> {
    let mut result: Vec<CpaProvider> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_keys: HashSet<(CpaSourceSegment, String)> = HashSet::new();

    for provider in providers {
        let dedup_key = if provider.source_segment == CpaSourceSegment::OpenaiCompatibility {
            // openai-compatibility 按 name 去重
            if let Some(ref name) = provider.name {
                if !seen_names.insert(name.clone()) {
                    // 已存在，合并模型
                    if let Some(existing) = result.iter_mut().find(|p| p.name.as_ref() == Some(name)) {
                        merge_models(existing, &provider);
                    }
                    continue;
                }
                true
            } else {
                // 无 name，按 base_url 去重
                if !seen_keys.insert((provider.source_segment, provider.base_url.clone())) {
                    if let Some(existing) = result.iter_mut().find(|p| {
                        p.source_segment == provider.source_segment && p.base_url == provider.base_url
                    }) {
                        merge_models(existing, &provider);
                    }
                    continue;
                }
                true
            }
        } else if provider.source_segment == CpaSourceSegment::OAuth {
            // OAuth base_url 全空，按 (segment, name/email) 去重；
            // CLIProxyAPI 多账号语义：各凭据 email 不同 → 各自独立 key 不撞
            let key_name = provider.name.clone().unwrap_or_default();
            if !seen_keys.insert((provider.source_segment, key_name.clone())) {
                if let Some(existing) = result.iter_mut().find(|p| {
                    p.source_segment == CpaSourceSegment::OAuth
                        && p.name.as_deref() == Some(&key_name)
                }) {
                    merge_models(existing, &provider);
                }
                continue;
            }
            true
        } else {
            // 其他段按 source_segment + base_url 去重
            if !seen_keys.insert((provider.source_segment, provider.base_url.clone())) {
                if let Some(existing) = result.iter_mut().find(|p| {
                    p.source_segment == provider.source_segment && p.base_url == provider.base_url
                }) {
                    merge_models(existing, &provider);
                }
                continue;
            }
            true
        };

        if dedup_key {
            result.push(provider);
        }
    }

    result
}

/// 合并模型列表（去重）。
fn merge_models(existing: &mut CpaProvider, new: &CpaProvider) {
    let new_models: Vec<_> = new.models.iter()
        .filter(|m| !existing.models.contains(m))
        .cloned()
        .collect();
    existing.models.extend(new_models);
}

// ─── 公开 API ───────────────────────────────────────────────────────

/// 解析 CPA 配置。
///
/// # 参数
/// - `path`: 文件/目录路径（单文件/压缩包/文件夹）
/// - `auth_dir`: 可选 OAuth 凭据目录路径
///
/// # 返回
/// 成功返回 `ParseResult`，失败返回错误字符串。
pub fn parse_cpa_config(path: &str, auth_dir: Option<&str>) -> Result<ParseResult, String> {
    let path = Path::new(path);
    let mut all_providers = Vec::new();
    let mut skipped = Vec::new();
    let mut source_files = Vec::new();

    // 检查不支持的压缩格式
    if is_unsupported_archive(path) {
        return Ok(ParseResult {
            providers: vec![],
            skipped: vec![SkipReason {
                path: path.display().to_string(),
                reason: "不支持的压缩格式（rar/7z），请先解压再选择文件夹".to_string(),
            }],
            source_files: vec![],
        });
    }

    // 判断类型并解析
    if path.is_file() {
        if is_supported_archive(path) {
            // 压缩包：解压后扫描
            let temp_dir = if path.extension()
                .and_then(|s| s.to_str()) == Some("zip") {
                unzip_archive(path)?
            } else {
                untar_archive(path)?
            };

            let files = collect_config_files(temp_dir.path());
            info!("压缩包解压后发现 {} 个配置文件", files.len());

            for file in &files {
                match parse_single_file(file) {
                    Ok(providers) => {
                        source_files.push(file.display().to_string());
                        all_providers.extend(providers);
                    }
                    Err(e) => {
                        skipped.push(SkipReason {
                            path: file.display().to_string(),
                            reason: e,
                        });
                    }
                }
            }
            // temp_dir 自动清理
        } else {
            // 单文件
            match parse_single_file(path) {
                Ok(providers) => {
                    source_files.push(path.display().to_string());
                    all_providers = providers;
                }
                Err(e) => {
                    skipped.push(SkipReason {
                        path: path.display().to_string(),
                        reason: e,
                    });
                }
            }
        }
    } else if path.is_dir() {
        // 目录：递归扫描
        let files = collect_config_files(path);
        info!("目录扫描发现 {} 个配置文件", files.len());

        for file in &files {
            match parse_single_file(file) {
                Ok(providers) => {
                    source_files.push(file.display().to_string());
                    all_providers.extend(providers);
                }
                Err(e) => {
                    skipped.push(SkipReason {
                        path: file.display().to_string(),
                        reason: e,
                    });
                }
            }
        }
    } else {
        return Err(format!("路径不存在: {}", path.display()));
    }

    // OAuth auth-dir 扫描
    if let Some(auth_dir_path) = auth_dir {
        let auth_path = Path::new(auth_dir_path);
        let oauth_providers = scan_auth_dir(auth_path);
        info!("auth-dir 扫描发现 {} 个 OAuth 凭据", oauth_providers.len());
        if !oauth_providers.is_empty() {
            source_files.push(auth_path.display().to_string());
        }
        all_providers.extend(oauth_providers);
    }

    // 去重合并
    let providers = deduplicate_providers(all_providers);

    Ok(ParseResult {
        providers,
        skipped,
        source_files,
    })
}

// ─── 测试 ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpa_source_segment_yaml_keys() {
        assert_eq!(CpaSourceSegment::GeminiApiKey.yaml_key(), "gemini-api-key");
        assert_eq!(CpaSourceSegment::OpenaiCompatibility.yaml_key(), "openai-compatibility");
        assert!(CpaSourceSegment::all_keys().contains(&"codex-api-key"));
    }

    #[test]
    fn test_oauth_type_from_str() {
        assert_eq!(CpaOAuthType::parse_oauth_type("xai"), Some(CpaOAuthType::Xai));
        assert_eq!(CpaOAuthType::parse_oauth_type("XAi"), Some(CpaOAuthType::Xai));
        assert_eq!(CpaOAuthType::parse_oauth_type("vertex"), Some(CpaOAuthType::Vertex));
        assert_eq!(CpaOAuthType::parse_oauth_type("unknown"), None);
    }

    #[test]
    fn test_deduplicate_providers() {
        let providers = vec![
            CpaProvider {
                source_segment: CpaSourceSegment::OpenaiCompatibility,
                name: Some("test".to_string()),
                base_url: "https://a.com".to_string(),
                api_key: "key1".to_string(),
                models: vec!["gpt-4".to_string()],
                prefix: None,
                headers: HashMap::new(),
                disabled: false,
                oauth_type: None,
            },
            CpaProvider {
                source_segment: CpaSourceSegment::OpenaiCompatibility,
                name: Some("test".to_string()),
                base_url: "https://a.com".to_string(),
                api_key: "key2".to_string(),
                models: vec!["gpt-3.5".to_string()],
                prefix: None,
                headers: HashMap::new(),
                disabled: false,
                oauth_type: None,
            },
        ];

        let result = deduplicate_providers(providers);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].api_key, "key1"); // 保留首个
        assert_eq!(result[0].models.len(), 2); // 模型合并
    }

    #[test]
    fn test_parse_models() {
        let json = r#"[
            {"name": "gpt-4", "alias": "g4"},
            {"name": "gpt-3.5", "alias": "g35"}
        ]"#;
        let val: Value = serde_json::from_str(json).unwrap();
        let models = parse_models(Some(&val));
        assert_eq!(models.len(), 2);
        assert!(models.contains(&"gpt-4".to_string()));
        assert!(models.contains(&"gpt-3.5".to_string()));
    }

    #[test]
    fn test_is_unsupported_archive() {
        assert!(is_unsupported_archive(Path::new("test.rar")));
        assert!(is_unsupported_archive(Path::new("test.7z")));
        assert!(!is_unsupported_archive(Path::new("test.zip")));
        assert!(!is_unsupported_archive(Path::new("test.tar.gz")));
    }

    #[test]
    fn test_merge_models() {
        let mut existing = CpaProvider {
            source_segment: CpaSourceSegment::GeminiApiKey,
            name: None,
            base_url: "https://a.com".to_string(),
            api_key: "key".to_string(),
            models: vec!["gemini-2".to_string()],
            prefix: None,
            headers: HashMap::new(),
            disabled: false,
            oauth_type: None,
        };
        let new = CpaProvider {
            source_segment: CpaSourceSegment::GeminiApiKey,
            name: None,
            base_url: "https://a.com".to_string(),
            api_key: "key2".to_string(),
            models: vec!["gemini-2".to_string(), "gemini-1".to_string()],
            prefix: None,
            headers: HashMap::new(),
            disabled: false,
            oauth_type: None,
        };
        merge_models(&mut existing, &new);
        assert_eq!(existing.models.len(), 2);
    }

    #[test]
    fn test_parse_oauth_json_single() {
        let content = r#"{"type":"xai","email":"a@b","access_token":"tok","model_aliases":[{"name":"grok-1","alias":"g1"}]}"#;
        let result = parse_oauth_json(content).expect("应识别为 OAuth 凭据");
        assert_eq!(result.len(), 1);
        let p = &result[0];
        assert_eq!(p.source_segment, CpaSourceSegment::OAuth);
        assert_eq!(p.oauth_type, Some(CpaOAuthType::Xai));
        assert_eq!(p.name.as_deref(), Some("a@b"));
        assert_eq!(p.api_key, "tok");
        assert_eq!(p.models, vec!["grok-1".to_string()]);
        assert!(p.base_url.is_empty());
    }

    #[test]
    fn test_parse_oauth_json_no_token() {
        // 是 OAuth 类型但无 access_token → None
        let content = r#"{"type":"xai","email":"a@b"}"#;
        assert!(parse_oauth_json(content).is_none());
    }

    #[test]
    fn test_parse_oauth_json_unknown_type() {
        // type 不可识别为 CpaOAuthType → None
        let content = r#"{"type":"unknown","access_token":"tok"}"#;
        assert!(parse_oauth_json(content).is_none());
    }

    #[test]
    fn test_is_oauth_credential() {
        assert!(is_oauth_credential(r#"{"type":"xai"}"#));
        assert!(is_oauth_credential(r#"{"type":"claude","email":"x"}"#)); // 缺 token 也是 OAuth
        assert!(!is_oauth_credential(r#"{"type":"unknown"}"#));
        assert!(!is_oauth_credential(r#"{"gemini-api-key":["x"]}"#));
        assert!(!is_oauth_credential("not json"));
    }

    #[test]
    fn test_dedup_oauth_distinct_emails() {
        // 2 个 xai OAuth 凭据 email 不同 → 保留两个，不合并
        let providers = vec![
            CpaProvider {
                source_segment: CpaSourceSegment::OAuth,
                name: Some("a@b".to_string()),
                base_url: String::new(),
                api_key: "tok1".to_string(),
                models: vec!["grok-1".to_string()],
                prefix: None,
                headers: HashMap::new(),
                disabled: false,
                oauth_type: Some(CpaOAuthType::Xai),
            },
            CpaProvider {
                source_segment: CpaSourceSegment::OAuth,
                name: Some("c@d".to_string()),
                base_url: String::new(),
                api_key: "tok2".to_string(),
                models: vec!["grok-2".to_string()],
                prefix: None,
                headers: HashMap::new(),
                disabled: false,
                oauth_type: Some(CpaOAuthType::Xai),
            },
        ];
        let result = deduplicate_providers(providers);
        assert_eq!(result.len(), 2);
        let names: Vec<_> = result.iter().map(|p| p.name.as_deref()).collect();
        assert!(names.contains(&Some("a@b")));
        assert!(names.contains(&Some("c@d")));
    }

    #[test]
    fn test_dedup_oauth_same_email_merges() {
        // 同 email OAuth 凭据 → 合并模型
        let providers = vec![
            CpaProvider {
                source_segment: CpaSourceSegment::OAuth,
                name: Some("a@b".to_string()),
                base_url: String::new(),
                api_key: "tok1".to_string(),
                models: vec!["grok-1".to_string()],
                prefix: None,
                headers: HashMap::new(),
                disabled: false,
                oauth_type: Some(CpaOAuthType::Xai),
            },
            CpaProvider {
                source_segment: CpaSourceSegment::OAuth,
                name: Some("a@b".to_string()),
                base_url: String::new(),
                api_key: "tok2".to_string(),
                models: vec!["grok-2".to_string()],
                prefix: None,
                headers: HashMap::new(),
                disabled: false,
                oauth_type: Some(CpaOAuthType::Xai),
            },
        ];
        let result = deduplicate_providers(providers);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].models.len(), 2);
    }

    #[test]
    fn test_parse_single_file_oauth_missing_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cred.json");
        std::fs::write(&path, r#"{"type":"xai","email":"a@b"}"#).unwrap();
        let result = parse_single_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("access_token"),
            "应提示 OAuth 缺 token，实际: {err}"
        );
    }

    #[test]
    fn test_parse_single_file_oauth_complete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cred.json");
        std::fs::write(
            &path,
            r#"{"type":"xai","email":"a@b","access_token":"tok","model_aliases":[{"name":"grok-1"}]}"#,
        )
        .unwrap();
        let providers = parse_single_file(&path).expect("应解析为 OAuth provider");
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].oauth_type, Some(CpaOAuthType::Xai));
        assert_eq!(providers[0].api_key, "tok");
    }

    #[test]
    fn test_parse_single_file_unknown_type_not_oauth() {
        // type=unknown → 非 OAuth 凭据 → 走 CPA stub → 无 CPA 段
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cred.json");
        std::fs::write(&path, r#"{"type":"unknown","access_token":"tok"}"#).unwrap();
        let result = parse_single_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("CPA provider 段"),
            "应走 CPA stub 文案，实际: {err}"
        );
    }

    #[test]
    fn test_scan_auth_dir_via_parse_oauth_json() {
        // 回归：scan_auth_dir 抽函数后行为不变
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.json"),
            r#"{"type":"claude","email":"x@y","access_token":"tok","model_aliases":[]}"#,
        )
        .unwrap();
        // 无 token 的 OAuth 凭据 → 静默跳过
        std::fs::write(dir.path().join("b.json"), r#"{"type":"claude","email":"z"}"#).unwrap();
        // 非 OAuth JSON → 静默跳过
        std::fs::write(dir.path().join("c.json"), r#"{"type":"unknown"}"#).unwrap();

        let providers = scan_auth_dir(dir.path());
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name.as_deref(), Some("x@y"));
        assert_eq!(providers[0].api_key, "tok");
    }
}
