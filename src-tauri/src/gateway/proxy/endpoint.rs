use super::*;

/// 根据请求路径自动推断入站 AI 协议格式
/// - /v1/messages → anthropic
/// - /v1/responses → openai_responses（Codex，body 用 input）
/// - /v1/chat/completions, /v1/completions, /models, /images, /audio → openai
/// - /v1beta/models/... → gemini
///   回退到 anthropic
pub(crate) fn detect_source_protocol(path: &str) -> String {
    // 定位到 /v1/ 起始（跳过代理根前缀如 /proxy）；分组路由已纯按 apikey，无 group path 前缀
    let api_path = if let Some(idx) = path.find("/v1/") {
        &path[idx..]
    } else if path.contains("/v1beta/") {
        return "gemini".to_string();
    } else {
        return "anthropic".to_string();
    };

    if api_path.starts_with("/v1/messages") {
        "anthropic".to_string()
    } else if api_path.starts_with("/v1/responses") {
        // OpenAI Responses API（Codex 等）用 `input` 而非 `messages`，
        // 必须单独派发到 openai_responses 入站解析，不能与 chat/completions 同组。
        "openai_responses".to_string()
    } else if api_path.starts_with("/v1/chat/completions")
        || api_path.starts_with("/v1/completions")
        || api_path.starts_with("/v1/embeddings")
        || api_path.starts_with("/v1/images")
        || api_path.starts_with("/v1/audio")
        || api_path.starts_with("/v1/models")
    {
        "openai".to_string()
    } else if path.contains("/v1beta/") {
        "gemini".to_string()
    } else {
        "anthropic".to_string()
    }
}

/// 按入站 User-Agent 推断客户端"原生" wire 协议（仅用于 UA 透传分支，见 [protocol-same-proto-passthrough] 扩展）。
///
/// 复用现有出站合成 UA 的子串特征规则（`claude_code_ua` / `codex_ua`）应用到入站匹配：
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
pub(crate) fn endpoint_host(base_url: &str) -> Option<String> {
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

pub(crate) fn select_endpoint_for_protocol<'a>(
    endpoints: &'a [super::models::PlatformEndpoint],
    source_protocol: &str,
) -> Option<&'a super::models::PlatformEndpoint> {
    let ep_proto = |ep: &super::models::PlatformEndpoint| format!("{:?}", ep.protocol).to_lowercase();
    let has_coding_ep = endpoints.iter().any(|ep| ep.coding_plan);
    if has_coding_ep {
        // 步骤 1（加固）：同协议端点直发原协议。采纳条件放宽为 `coding_plan ||
        // 与某 coding 端点同 host`——后者覆盖 GLM 形态（anthropic 端点 base_url 与
        // openai coding 端点同 host `open.bigmodel.cn`，同一把 coding key 通用，DB 中
        // anthropic 端点 coding_plan=false 仍应原协议直发，无需 migration 改数据）。
        // 跨 host 的同协议端点（Kimi anthropic 端点 host=moonshot.cn ≠ coding host
        // kimi.com，需另一把常规 key，coding key 打过去 401）不采纳，落步骤 2 转换。
        // 步骤 2：openai coding 兜底（转换出站）。两步均不落「跨 host 非 coding」端点（防 401）。
        let key_usable = |ep: &super::models::PlatformEndpoint| {
            ep.coding_plan
                || endpoint_host(&ep.base_url).is_some_and(|h| {
                    endpoints
                        .iter()
                        .any(|c| c.coding_plan && endpoint_host(&c.base_url).as_deref() == Some(&h))
                })
        };
        endpoints
            .iter()
            .find(|ep| ep_proto(ep) == source_protocol && key_usable(ep))
            .or_else(|| endpoints.iter().find(|ep| ep.coding_plan && ep_proto(ep) == "openai"))
    } else {
        // 普通平台：步骤 3 同协议直发；步骤 4 openai_responses 回退 openai。
        endpoints
            .iter()
            .find(|ep| ep_proto(ep) == source_protocol)
            .or_else(|| {
                if source_protocol == "openai_responses" {
                    endpoints.iter().find(|ep| ep_proto(ep) == "openai")
                } else {
                    None
                }
            })
    }
}

pub(crate) fn infer_passthrough_protocol_from_ua(ua: &str) -> Option<&'static str> {
    let lower = ua.to_lowercase();
    if lower.contains("claude-cli") {
        Some("anthropic")
    } else if lower.contains("codex") {
        Some("openai_responses")
    } else {
        None
    }
}

/// 在已取出的分组列表中按 group_key（= Authorization Bearer apikey）精确匹配。
/// 分组路由纯按 apikey(group_key)，不再支持 URL path 前缀匹配。
pub(crate) async fn resolve_group(db: &Db, token: Option<&str>) -> Option<Group> {
    let groups = match super::db::list_groups(db).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "resolve_group: list_groups failed");
            return None;
        }
    };
    if let Some(token) = token {
        if let Some(idx) = groups.iter().position(|g| g.group_key == token) {
            return groups.into_iter().nth(idx);
        }
        tracing::warn!(token = %token, "resolve_group: token did not match any group_key");
    }
    tracing::warn!(group_count = groups.len(), "resolve_group: no group matched token");
    None
}

// ─── 客户端模拟 Header ────────────────────────────────────────

/// 根据客户端类型和目标协议，构建模拟的 HTTP 请求头。
/// 数据来源：GitHub 逆向分析 + claude-code-hub 参考实现
/// OpenCode Zen 平台 api_key 解析：用户填了用用户的；留空时注入匿名免费 key `$opencode`
/// （实测被服务端接受，与 `public` 等价走免费共享限频；裸随机串/$ 大写变体均 401）。
/// 对 `Protocol::OpenCodeZen` 平台或 base_url/endpoint 含 `opencode.ai/zen` 的平台生效，
/// 其余平台原样返回（空即空）。枚举判定与 lib.rs(fetch_models/model_test) 对齐，
/// 保证自定义 base_url 时 proxy 与 fetch_models 兜底一致（model-test-proxy parity）。
pub fn resolve_opencode_zen_key(platform: &super::models::Platform) -> String {
    let is_zen = matches!(platform.platform_type, Protocol::OpenCodeZen)
        || platform.base_url.to_lowercase().contains("opencode.ai/zen")
        || platform
            .endpoints
            .iter()
            .any(|ep| ep.base_url.to_lowercase().contains("opencode.ai/zen"));
    opencode_zen_fallback(&platform.api_key, is_zen)
}

/// `resolve_opencode_zen_key` 的纯决策核（便于单测，免构造 Platform）。
pub fn opencode_zen_fallback(api_key: &str, is_zen: bool) -> String {
    if !api_key.trim().is_empty() || !is_zen {
        api_key.to_string()
    } else {
        "$opencode".to_string()
    }
}

/// P1 CONNECT 隧道：仅按 CONNECT target host 段比对平台 base_url host。
///
/// 复用 `endpoint_host()`（剥 scheme/userinfo/port，小写化）。命中任一启用态平台
/// （enabled/auto_disabled）的主 base_url 或 endpoints[].base_url host 即返回 `(platform_id, Platform)`
/// 元组（P2 CONNECT 熔断需 Platform 解析 per-platform breaker 阈值，单次扫描一并返回避免二次 DB
/// 查询）；未命中返回 None（调用方写 platform_id=0）。P1 隧道无 apikey（HTTPS 未解密），无法做
/// group 路由——不计费、不入候选选择，仅 host 标记 proxy_log.platform_id。平台数量小，O(n) 全表
/// 扫描可接受（CONNECT 每连接一次）。
pub(crate) async fn match_platform_by_host(db: &Db, connect_host: &str) -> Option<(u64, super::models::Platform)> {
    let target = connect_host.to_lowercase();
    let platforms = match super::db::list_platforms(db).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "match_platform_by_host: list_platforms failed");
            return None;
        }
    };
    platforms.iter()
        .filter(|p| p.status != super::models::PlatformStatus::Disabled)
        .find(|p| {
            endpoint_host(&p.base_url).as_deref() == Some(&target)
                || p.endpoints.iter().any(|ep| endpoint_host(&ep.base_url).as_deref() == Some(&target))
        })
        .map(|p| (p.id, p.clone()))
}

#[cfg(test)]
#[path = "test_endpoint.rs"]
mod test_endpoint;
