//! sub2api 异源导入子模块。
//!
//! 解析 [sub2api](https://github.com/Wei-Shaw/sub2api) 管理后台导出的账号数据 JSON
//! （导出 API `GET /api/v1/admin/accounts/data`，格式标识 `type == "sub2api-data"`），
//! 仅提取核心连接信息（platform / credentials.api_key / credentials.base_url），
//! 原样透传成 [`Sub2ApiAccount`] DTO。**不做平台匹配** —— Protocol 映射在前端
//! 纯函数（preset 住前端，记忆 `aidog-add-platform-skill` 反直觉点 1）。
//!
//! 源 struct 实证（GitHub `Wei-Shaw/sub2api` ·
//! `backend/internal/handler/admin/account_data.go`，2026-06-19 查实）：
//! - 顶层 `DataPayload`：`type`（json:"type,omitempty"）/ `accounts`（json:"accounts"）。
//! - `DataAccount`：`name` / `platform`（"anthropic"/"openai"/"gemini"/...）/
//!   `credentials`（map[string]any，键 `api_key` + `base_url` 实证，
//!   见 `internal/service/account.go` `GetCredential("api_key")` / `GetCredential("base_url")`）。
//! - 丢弃：`proxies` 整段 / `type`(账号鉴权类型) / `extra` / `proxy_key` /
//!   `concurrency` / `priority` / `rate_multiplier` / `expires_at` / `notes`。
//! - models 不在 account 实体（在 Channel 实体）→ 导不到，非目标。

use serde::{Deserialize, Serialize};

use crate::gateway::db::Db;

/// sub2api 源 JSON 顶层结构（字段名 = sub2api json tag）。
/// 只解析 `type`（校验）+ `accounts`，其余（version/exported_at/proxies）丢弃。
#[derive(Deserialize)]
struct RawSub2ApiPayload {
    #[serde(rename = "type", default)]
    r#type: String,
    #[serde(default)]
    accounts: Vec<RawAccount>,
}

/// sub2api `DataAccount`（仅解析消费字段；其余 json tag 不声明 → serde 丢弃）。
#[derive(Deserialize)]
struct RawAccount {
    #[serde(default)]
    name: String,
    #[serde(default)]
    platform: String,
    #[serde(default)]
    credentials: RawCredentials,
}

/// `credentials` map 的消费子集（`api_key` + `base_url`，其余键丢弃）。
#[derive(Deserialize, Default)]
struct RawCredentials {
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
}

/// 透传给前端的账号 DTO（camelCase；Protocol 映射在前端做）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sub2ApiAccount {
    pub name: String,
    /// sub2api 原始 platform 值（小写），前端按 Protocol 映射 + 未识别兜底 openai。
    pub platform: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

/// 解析结果（返回前端预览）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sub2ApiReadResult {
    pub accounts: Vec<Sub2ApiAccount>,
}

/// 解析 sub2api-data JSON 文本 → 账号 DTO 列表。
///
/// 校验 `type == "sub2api-data"`；非法 JSON / type 不匹配 → Err。
/// credentials 的 api_key / base_url 空串归一化为 None（缺失回退在前端走预设默认）。
pub fn parse(json_text: &str) -> Result<Sub2ApiReadResult, String> {
    let raw: RawSub2ApiPayload = serde_json::from_str(json_text)
        .map_err(|e| format!("非 sub2api 导出文件（JSON 解析失败）：{e}"))?;
    if raw.r#type != "sub2api-data" {
        return Err(format!(
            "非 sub2api 导出文件（type != sub2api-data，实际：「{}」）",
            raw.r#type
        ));
    }
    let accounts = raw
        .accounts
        .into_iter()
        .map(|a| Sub2ApiAccount {
            name: a.name,
            platform: a.platform.trim().to_lowercase(),
            api_key: a.credentials.api_key.filter(|s| !s.trim().is_empty()),
            base_url: a.credentials.base_url.filter(|s| !s.trim().is_empty()),
        })
        .collect();
    Ok(Sub2ApiReadResult { accounts })
}

// ── apply 复用入口 ──────────────────────────────────────────

/// 把前端转换好的 platform payload + 决策应用进 aidog DB（复用 [`super::apply::apply`]）。
///
/// `auto_group=true` 时：apply 后 ensure-by-name 建/找 `sub2api` 分组并关联本次导入平台
/// （记忆 import-apply-bypasses-platform-create：apply 不触发命令级 auto-group，须显式做）。
pub async fn import(
    platform_payload: Vec<serde_json::Value>,
    decisions: &[super::ConflictDecision],
    auto_group: bool,
    db: &Db,
) -> Result<super::ImportReport, String> {
    // apply 前快照已有 platform id，供 auto-group 回出本次新建行。
    let before = if auto_group {
        super::apply::snapshot_platform_ids(db).await?
    } else {
        std::collections::BTreeSet::new()
    };

    let payload = super::Payload {
        manifest: super::Manifest {
            format_version: 1,
            aidog_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            source_machine: "sub2api-import".to_string(),
            scopes: vec![super::SCOPE_PLATFORM.to_string()],
            checksum: String::new(),
        },
        platform: platform_payload,
        group: Vec::new(),
        group_platform: Vec::new(),
        setting: Vec::new(),
        codex_global: None,
        codex_profiles: Vec::new(),
        claude_code_global: None,
        claude_code_group_settings: Vec::new(),
        skills: Vec::new(),
    };
    let report = super::apply::apply(payload, decisions, db).await?;

    if auto_group {
        super::apply::ensure_group_and_attach(db, "sub2api", &before).await?;
    }
    Ok(report)
}

// ── 单测 ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_json() -> String {
        json!({
            "type": "sub2api-data",
            "version": 1,
            "exported_at": "2026-06-19T00:00:00Z",
            "proxies": [
                {"proxy_key": "p|h|1|u|w", "name": "px", "protocol": "http", "host": "h", "port": 1, "status": "ok"}
            ],
            "accounts": [
                {
                    "name": "claude-acc",
                    "platform": "anthropic",
                    "type": "api_key",
                    "credentials": {"api_key": "sk-ant-xxx", "base_url": "https://anthropic.example.com"},
                    "extra": {"foo": "bar"},
                    "proxy_key": "p|h|1|u|w",
                    "concurrency": 5,
                    "priority": 1,
                    "rate_multiplier": 1.5,
                    "expires_at": 99999,
                    "notes": "n"
                },
                {
                    "name": "openai-acc",
                    "platform": "openai",
                    "type": "api_key",
                    "credentials": {"api_key": "sk-oai-xxx", "base_url": "https://oai.example.com/v1"}
                },
                {
                    "name": "gemini-acc",
                    "platform": "gemini",
                    "type": "api_key",
                    "credentials": {"api_key": "AIza-xxx", "base_url": "https://gemini.example.com"}
                }
            ]
        })
        .to_string()
    }

    #[test]
    fn parse_valid() {
        let r = parse(&sample_json()).unwrap();
        assert_eq!(r.accounts.len(), 3);
        assert_eq!(r.accounts[0].name, "claude-acc");
        assert_eq!(r.accounts[0].platform, "anthropic");
        assert_eq!(r.accounts[0].api_key.as_deref(), Some("sk-ant-xxx"));
        assert_eq!(
            r.accounts[0].base_url.as_deref(),
            Some("https://anthropic.example.com")
        );
        assert_eq!(r.accounts[1].platform, "openai");
        assert_eq!(r.accounts[2].platform, "gemini");
    }

    #[test]
    fn parse_rejects_wrong_type() {
        let bad = json!({"type": "other", "accounts": []}).to_string();
        assert!(parse(&bad).is_err());
    }

    #[test]
    fn parse_rejects_malformed() {
        assert!(parse("{not valid json").is_err());
    }

    #[test]
    fn parse_missing_base_url() {
        let j = json!({
            "type": "sub2api-data",
            "accounts": [{"name": "a", "platform": "openai", "credentials": {"api_key": "k"}}]
        })
        .to_string();
        let r = parse(&j).unwrap();
        assert_eq!(r.accounts[0].base_url, None);
        assert_eq!(r.accounts[0].api_key.as_deref(), Some("k"));
    }

    #[test]
    fn parse_missing_api_key() {
        let j = json!({
            "type": "sub2api-data",
            "accounts": [{"name": "a", "platform": "openai", "credentials": {"base_url": "https://x.com"}}]
        })
        .to_string();
        let r = parse(&j).unwrap();
        assert_eq!(r.accounts[0].api_key, None);
        assert_eq!(r.accounts[0].base_url.as_deref(), Some("https://x.com"));
    }

    #[test]
    fn parse_empty_credentials_strings_become_none() {
        let j = json!({
            "type": "sub2api-data",
            "accounts": [{"name": "a", "platform": "openai", "credentials": {"api_key": "  ", "base_url": ""}}]
        })
        .to_string();
        let r = parse(&j).unwrap();
        assert_eq!(r.accounts[0].api_key, None);
        assert_eq!(r.accounts[0].base_url, None);
    }

    #[test]
    fn parse_drops_extra_fields() {
        // 含 proxy_key/concurrency/extra/proxies → 解析成功，DTO 不含这些字段。
        let r = parse(&sample_json()).unwrap();
        // DTO 仅 name/platform/api_key/base_url；serde 序列化无 drop 字段。
        let v = serde_json::to_value(&r.accounts[0]).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 4);
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("platform"));
        assert!(obj.contains_key("apiKey"));
        assert!(obj.contains_key("baseUrl"));
        assert!(!obj.contains_key("extra"));
        assert!(!obj.contains_key("proxyKey"));
        assert!(!obj.contains_key("concurrency"));
    }

    #[test]
    fn parse_normalizes_platform_case() {
        let j = json!({
            "type": "sub2api-data",
            "accounts": [{"name": "a", "platform": "  Anthropic  ", "credentials": {}}]
        })
        .to_string();
        let r = parse(&j).unwrap();
        assert_eq!(r.accounts[0].platform, "anthropic");
    }
}
