//! 跨 crate 共享的 SQL 小工具（从 aidog_core::gateway::db 下沉，2026-08-16 logs-stats-crates）。

use rusqlite::Result as SqlResult;
use serde_json::Value;

/// 在给定连接上跑 `PRAGMA incremental_vacuum(N)`，回收至多 N 页 free pages。
///
/// auto_vacuum != INCREMENTAL 时为 no-op（SQLite 不报错）；失败仅 warn 不上抛，
/// 因为回收失败不影响数据正确性，下次 retention/手动压缩仍可重试。
pub fn incremental_vacuum_conn(conn: &rusqlite::Connection, max_pages: i64) {
    // PRAGMA incremental_vacuum 接受一个参数（要回收的最大页数）。rusqlite 用 query
    // 执行（pragma 返回行集），errors_here 仅 warn。
    let sql = format!("PRAGMA incremental_vacuum({max_pages})");
    if let Err(e) = conn.execute_batch(&sql) {
        tracing::warn!(error = %e, "incremental_vacuum failed (auto_vacuum != INCREMENTAL or busy), will retry later");
    }
}

/// 平台 id → name 内存映射（含软删平台，名仍可显示）。供统计维度按 platform_id GROUP BY
/// 后内存回填平台名用，替代旧 `LEFT JOIN platform` 取名（today_platform_stats J6 同模式）。
pub fn platform_id_name_map(
    conn: &rusqlite::Connection,
) -> SqlResult<std::collections::HashMap<i64, String>> {
    let mut stmt = conn.prepare_cached("SELECT id, name FROM platform")?;
    let map = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
        .collect::<SqlResult<Vec<_>>>()?
        .into_iter()
        .collect();
    Ok(map)
}

/// 从上游错误体提取人类可读 message，优先嵌套 `error.message`，回退顶层 `message`。
/// 非 JSON / 无字段 / 空白 → None（调用方回退 truncate_attempt_error）。
pub fn extract_error_message(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let msg = v
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .or_else(|| v.get("message").and_then(|m| m.as_str()))?;
    let msg = msg.trim();
    if msg.is_empty() {
        None
    } else {
        Some(msg.to_string())
    }
}


/// 按入站 User-Agent 推断客户端"原生" wire 协议（仅用于 UA 透传分支，见 [protocol-same-proto-passthrough] 扩展）。
///
/// 复用现有出站合成 UA 的子串特征规则（现由 client-types.json `simulation.user_agent` 配置驱动，
/// 详见 `headers.rs::simulation_map`）应用到入站匹配：
/// - 含 `claude-cli`（Claude Code CLI/VSCode/SDK/GhAction 全家族）→ `"anthropic"`
/// - 含 `codex`（codex_cli_rs / Codex/ / codex desktop / codex-vscode 全家族）→ `"openai_responses"`
/// - 其它（Cursor / Windsurf / gemini-cli / 未知 / 缺失）→ None（回退现有处理）
///
/// 大小写不敏感（Codex TUI UA 为 `Codex/...`，需匹配 `codex`）。返回的字面量与
/// `detect_source_protocol` / `ep_proto` 产出的协议名一致，便于直接比对 endpoint。
/// 按入站协议(`source_protocol`)从平台端点中选目标 endpoint。
///
/// 通用原则：**尽可能用原协议直发，避免有损转换**（[protocol-same-proto-passthrough]）。
/// 优先级链（从最优到兜底）：
///   1. coding_plan 端点中按入站协议精确匹配（同协议 coding，直发不转换）
///      —— 平台同时含多个 coding 端点（如 GLM/千帆/小米：openai coding + anthropic coding）时，
///      anthropic 入站选 anthropic coding 端点、openai 入站选 openai coding 端点，各走原协议。
///   2. coding_plan 端点中回退 openai coding（入站无对应同协议 coding 端点时，转换出站）
///      —— Kimi coding 仅有 openai coding 端点，anthropic 入站经此回退，`convert_request` 转 openai。
///   3. 非 coding 端点按入站协议精确匹配（普通双协议平台，同协议直发）。
///   4. `openai_responses` 源(Codex)无 Responses 端点时回退到 openai 端点（出站经 to_openai 转换）。
///
/// ── coding-plan 端点排他（防 401，务必保留）──
/// coding-plan 平台的 api_key **仅对 coding endpoint(`coding_plan:true`)有效**；其非 coding endpoint
/// (如 kimi 的 `api.moonshot.cn/anthropic`，指向常规 API host)需另一把常规 key，被 coding key 打成 401
/// → 连累整个平台 auto_disabled。故**平台含任一 coding 端点时，绝不落到非 coding 端点**：优先级链 1→2
/// 全部限定 `coding_plan==true`，仅当无任何 coding 端点(普通平台)才进入 3/4。
/// 这同时满足通用原则：coding 平台的同协议 coding 端点（步骤 1）优先于跨协议转换（步骤 2）。
/// 从 endpoint 的 `base_url` 提取 host（authority 主机名，小写、不含端口/路径）。
///
/// 规则：剥离 `scheme://` 前缀后，取到首个 `/`、`?`、`#` 或 `:`（端口分隔）之前的部分，
/// 并去掉可能的 `user@` 凭证段，最后小写化。解析失败（空 host）返回 None——
/// 调用方据此**保守处理**：host 解析不出 → 不视为同 host（宁可走转换也不误用 coding key）。
pub fn endpoint_host(base_url: &str) -> Option<String> {
    let after_scheme = base_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(base_url);
    // authority 段：截到首个路径/查询/锚点分隔符之前
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    // 去掉 userinfo（user:pass@host）
    let host_port = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    // 去掉端口（注意 IPv6 字面量含 ':'，但 base_url 平台预设均为域名，简单截端口即可）
    let host = host_port.split(':').next().unwrap_or(host_port);
    let host = host.trim().to_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}


/// 默认白名单规则集（37 条：Claude 3 + OpenAI 34）。
///
/// 来源：blackmatrix7/ios_rule_script OpenAI/Claude 规则集（Clash DOMAIN/SUFFIX/KEYWORD/IPCIDR）。
/// 元组 `(rule_type, pattern)`：rule_type ∈ {domain, suffix, keyword, ipcidr}，
/// pattern 存规则值（host 域名 / CIDR 串）。
///
/// 单源（schema migration 20260727-15（原 041/043）seed + 本模块 import_defaults command + 测试 共用此常量）。
/// 舍弃：IP-ASN 20473（不支持）；GeoIP/DNS 解析（不要）。
pub const DEFAULT_RULES: &[(&str, &str)] = &[
    // ── Claude（3 条）─────────────────────────────────────────
    ("domain", "cdn.usefathom.com"),
    ("suffix", "anthropic.com"),
    ("suffix", "claude.ai"),
    // ── OpenAI domain（7 条）──────────────────────────────────
    ("domain", "browser-intake-datadoghq.com"),
    ("domain", "chat.openai.com.cdn.cloudflare.net"),
    ("domain", "openai-api.arkoselabs.com"),
    ("domain", "openaicom-api-bdcpf8c6d2e9atf6.z01.azurefd.net"),
    ("domain", "openaicomproductionae4b.blob.core.windows.net"),
    ("domain", "production-openaicom-storage.azureedge.net"),
    ("domain", "static.cloudflareinsights.com"),
    // ── OpenAI suffix（24 条）─────────────────────────────────
    ("suffix", "ai.com"),
    ("suffix", "algolia.net"),
    ("suffix", "api.statsig.com"),
    ("suffix", "auth0.com"),
    ("suffix", "chatgpt.com"),
    ("suffix", "chatgpt.livekit.cloud"),
    ("suffix", "client-api.arkoselabs.com"),
    ("suffix", "events.statsigapi.net"),
    ("suffix", "featuregates.org"),
    ("suffix", "host.livekit.cloud"),
    ("suffix", "identrust.com"),
    ("suffix", "intercom.io"),
    ("suffix", "intercomcdn.com"),
    ("suffix", "launchdarkly.com"),
    ("suffix", "oaistatic.com"),
    ("suffix", "oaiusercontent.com"),
    ("suffix", "observeit.net"),
    ("suffix", "openai.com"),
    ("suffix", "openaiapi-site.azureedge.net"),
    ("suffix", "openaicom.imgix.net"),
    ("suffix", "segment.io"),
    ("suffix", "sentry.io"),
    ("suffix", "stripe.com"),
    ("suffix", "turn.livekit.cloud"),
    // ── OpenAI keyword（1 条）──────────────────────────────────
    ("keyword", "openai"),
    // ── OpenAI ipcidr（2 条，仅匹配 IP 字面 CONNECT 目标）──────
    ("ipcidr", "24.199.123.28/32"),
    ("ipcidr", "64.23.132.171/32"),
];

// 序列化小工具（platform.rs 下沉，schema 迁移 + core 领域层共用）
use crate::models::{PlatformModels, PlatformEndpoint};
/// 从 JSON 字符串反序列化 models
pub fn parse_models(json: &str) -> PlatformModels {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse platform models failed, using default (stored JSON corrupt?)");
        PlatformModels::default()
    })
}

/// 将 models 序列化为 JSON 字符串
pub fn serialize_models(models: &PlatformModels) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize platform models failed, persisting empty object");
        "{}".to_string()
    })
}

/// 从 JSON 字符串反序列化可用模型列表
pub fn parse_available_models(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse available_models failed, using empty list (stored JSON corrupt?)");
        Vec::new()
    })
}

/// 将可用模型列表序列化为 JSON 字符串
pub fn serialize_available_models(models: &[String]) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize available_models failed, persisting empty array");
        "[]".to_string()
    })
}

/// 从 JSON 字符串反序列化协议端点列表
pub fn parse_endpoints(json: &str) -> Vec<PlatformEndpoint> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse platform endpoints failed, using empty list (stored JSON corrupt?)");
        Vec::new()
    })
}

/// 将协议端点列表序列化为 JSON 字符串
pub fn serialize_endpoints(endpoints: &[PlatformEndpoint]) -> String {
    serde_json::to_string(endpoints).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize platform endpoints failed, persisting empty array");
        "[]".to_string()
    })
}

/// Codex home 目录（`CODEX_HOME` env 覆盖 → `~/.codex` 兜底）。
/// 原 aidog_core::gateway::codex::codex_home 逻辑下沉：aidog_mcp 跨 crate 复用同一真值。
pub fn codex_home() -> Result<std::path::PathBuf, String> {
    if let Ok(custom) = std::env::var("CODEX_HOME")
        && !custom.trim().is_empty() {
            return Ok(std::path::PathBuf::from(custom));
        }
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".codex"))
}
