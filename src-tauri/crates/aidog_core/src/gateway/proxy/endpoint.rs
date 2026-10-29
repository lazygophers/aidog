use super::*;

/// 根据请求路径自动推断入站 AI 协议格式
/// - /v1/messages → anthropic
/// - /v1/responses → openai_responses（Codex，body 用 input）
/// - /v1/chat/completions, /v1/completions, /models, /images, /audio → openai
/// - /v1beta/models/... → gemini
///   回退到 anthropic
///
/// **`/v1` 可省略**：部分 OpenAI 兼容客户端（网关/SDK 把版本段算进 base_url）直发裸端点
/// `/proxy/chat/completions`，无 `/v1/` 段。此时按端点名后缀同样判 openai，
/// 否则会误落 anthropic 回退 → `parse_incoming_request` 解析 OpenAI body 失败返 400。
/// 例外：裸 `/models` 保持 anthropic 回退（`passthrough::handle_models_list` 依赖此语义
/// 输出 anthropic 列表格式，见 passthrough.rs:342）。
pub(crate) fn detect_source_protocol(path: &str) -> Protocol {
    if path.contains("/v1beta/") {
        return Protocol::Gemini;
    }
    // 定位到 /v1/ 起始（跳过代理根前缀如 /proxy）；分组路由已纯按 apikey，无 group path 前缀。
    // 无 /v1/ 段时退化为整段路径做端点名后缀匹配（裸端点客户端）。
    let (api_path, versioned) = match path.find("/v1/") {
        Some(idx) => (&path[idx + 3..], true), // 去掉 "/v1"，留 "/messages" 等
        None => (path, false),
    };

    // 端点名匹配：versioned 时必须是 api_path 开头（严格）；裸路径时允许任意前缀（/proxy 等）
    let hit = |ep: &str| {
        if versioned {
            api_path.starts_with(ep)
        } else {
            api_path.ends_with(ep) || api_path.contains(&format!("{ep}/"))
        }
    };

    if hit("/messages") {
        Protocol::Anthropic
    } else if hit("/responses") {
        // OpenAI Responses API（Codex 等）用 `input` 而非 `messages`，
        // 必须单独派发到 openai_responses 入站解析，不能与 chat/completions 同组。
        Protocol::OpenAIResponses
    } else if hit("/chat/completions")
        || hit("/completions")
        || hit("/embeddings")
        || hit("/images")
        || hit("/audio")
        || (versioned && api_path.starts_with("/models"))
    {
        Protocol::OpenAI
    } else {
        Protocol::Anthropic
    }
}

/// 从 gemini 入站 path 取模型名：`/v1beta/models/<model>:<method>` → `<model>`。
///
/// gemini 的模型名只在 URL 里，body 无此字段（票 09）。取不到返回空串，调用方按「无模型名」处理。
/// 代理根前缀（`/proxy/...`）与方法段（`:generateContent` / `:streamGenerateContent` /
/// `:countTokens`）都不进结果。
pub(crate) fn model_from_gemini_path(path: &str) -> String {
    let Some(idx) = path.find("/models/") else {
        return String::new();
    };
    let rest = &path[idx + "/models/".len()..];
    let seg = rest.split('/').next().unwrap_or("");
    seg.split(':').next().unwrap_or("").to_string()
}

pub(crate) fn select_endpoint_for_protocol<'a>(
    endpoints: &'a [super::models::PlatformEndpoint],
    source_protocol: &Protocol,
) -> Option<&'a super::models::PlatformEndpoint> {
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
            .find(|ep| ep.protocol == *source_protocol && key_usable(ep))
            .or_else(|| {
                endpoints
                    .iter()
                    .find(|ep| ep.coding_plan && ep.protocol == Protocol::OpenAI)
            })
    } else {
        // 普通平台：步骤 3 同协议直发；步骤 4 跨协议回退（释放 converter 5×5 互转）。
        // 优先 openai（最稳 converter 路径，平台最常见），若无 openai 取 endpoints 首个非 source 可用 endpoint。
        endpoints
            .iter()
            .find(|ep| ep.protocol == *source_protocol)
            .or_else(|| endpoints.iter().find(|ep| ep.protocol == Protocol::OpenAI))
            .or_else(|| endpoints.iter().find(|ep| ep.protocol != *source_protocol))
    }
}

/// TypeSafe 决策请求端点判定（jev-decision-proxy R1）：路径以 `/systemone` 结尾即为决策请求。
/// 覆盖 `/v1/systemone`、`/proxy/v1/systemone` 等变体（与 R1「尾段匹配」口径一致）。
pub(crate) fn is_decision_endpoint(path: &str) -> bool {
    path.trim_end_matches('/').ends_with("/systemone")
}

/// 独立搜索/tools 端点判定（spec-独立搜索转发 语义 2）：入站 path 与任一平台
/// `tools_endpoints` 声明 URL 的 path 精确相等（容忍 `/proxy` 前缀与尾 `/` 变体）。
/// 全库无声明 → false，请求落现有链路（语义 11 零回归）。数据经 effective_presets
/// （DB 同步值优先）现查不另建聚合缓存：tools 请求低频，一次 preset 文档遍历可忽略。
pub(crate) fn is_tools_endpoint(path: &str) -> bool {
    let doc = crate::gateway::registry::effective_presets();
    doc.get("protocols")
        .and_then(|p| p.as_object())
        .is_some_and(|protos| {
            protos.values().any(|po| {
                po.get("tools_endpoints")
                    .and_then(|t| t.as_object())
                    .is_some_and(|m| {
                        m.values().any(|u| {
                            u.as_str()
                                .is_some_and(|u| crate::gateway::billing::tools_url_matches(u, path))
                        })
                    })
            })
        })
}

/// 决策请求的 endpoint 选择（jev-decision-proxy R9）：
/// 平台有 `typesafe` 协议 endpoint 优先；否则 OpenAI 系 endpoint（chat / completions / responses）。
/// 都没有返回 None → 调用方回退平台主 `base_url` + `/systemone`（零端点平台）。
pub(crate) fn select_endpoint_for_decision(
    endpoints: &[super::models::PlatformEndpoint],
) -> Option<&super::models::PlatformEndpoint> {
    endpoints
        .iter()
        .find(|ep| ep.protocol == Protocol::TypeSafe)
        .or_else(|| endpoints.iter().find(|ep| ep.protocol == Protocol::OpenAI))
        .or_else(|| endpoints.iter().find(|ep| ep.protocol == Protocol::OpenAICompletions))
        .or_else(|| endpoints.iter().find(|ep| ep.protocol == Protocol::OpenAIResponses))
}

pub(crate) fn infer_passthrough_protocol_from_ua(ua: &str) -> Option<Protocol> {
    let lower = ua.to_lowercase();
    if lower.contains("claude-cli") {
        Some(Protocol::Anthropic)
    } else if lower.contains("codex") {
        Some(Protocol::OpenAIResponses)
    } else {
        None
    }
}

/// 密钥通道（ADR 0009）：客户端把分组密钥放进请求的一个位置。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum KeyChannel {
    /// header 名（小写）。
    Header(String),
    /// URL query 参数名。
    Query(&'static str),
}

/// 从某个密钥通道解出的一个候选。原值 = `prefix + key + suffix`；透传出站原位替换时只换 key 段。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KeyCandidate {
    pub key: String,
    pub channel: KeyChannel,
    pub prefix: String,
    pub suffix: String,
}

/// 默认 header 通道（除 Authorization，它另按方案词拆分）。
const DEFAULT_KEY_HEADERS: [&str; 3] = ["x-api-key", "api-key", "x-goog-api-key"];
/// 默认 query 通道。
pub(crate) const KEY_QUERY_PARAMS: [&str; 2] = ["api_key", "key"];

/// 头名是否属于默认密钥通道（不区分大小写）。
fn is_default_key_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("authorization")
        || DEFAULT_KEY_HEADERS.iter().any(|h| name.eq_ignore_ascii_case(h))
}

/// 透传平台 extra → 自定义密钥通道 `(头名小写, 模板)`（ADR 0009 决策 2）。
/// 只认显式配置了 `auth_header` 或 `auth_template` 的平台；缺省的 `Authorization: Bearer {key}`
/// 已在默认通道里。模板不含 `{key}` 的跳过（解不出 key）。
pub(crate) fn custom_key_channels(extras: &[String]) -> Vec<(String, String)> {
    extras
        .iter()
        .filter_map(|extra| {
            let cfg: Value = serde_json::from_str(extra).ok()?;
            let field = |k: &str| {
                cfg.get(k)
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            };
            let (header, template) = (field("auth_header"), field("auth_template"));
            if header.is_none() && template.is_none() {
                return None;
            }
            let template = template.unwrap_or_else(|| "Bearer {key}".to_string());
            template.contains("{key}").then(|| {
                (
                    header.unwrap_or_else(|| "authorization".to_string()).to_ascii_lowercase(),
                    template,
                )
            })
        })
        .collect()
}

/// 自定义通道里不属于默认通道的头名（去重）：转发前要额外剥掉的头。
pub(crate) fn custom_key_header_names(custom: &[(String, String)]) -> Vec<String> {
    let mut names: Vec<String> = custom
        .iter()
        .map(|(n, _)| n.clone())
        .filter(|n| !is_default_key_header(n))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// 按模板 `pre{key}post` 从头值解出 key（前后缀不区分大小写）。
fn match_template(value: &str, template: &str) -> Option<KeyCandidate> {
    let (pre, post) = template.split_once("{key}")?;
    if value.len() < pre.len() + post.len() {
        return None;
    }
    let head = value.get(..pre.len())?;
    let tail = value.get(value.len() - post.len()..)?;
    if !head.eq_ignore_ascii_case(pre) || !tail.eq_ignore_ascii_case(post) {
        return None;
    }
    let key = value.get(pre.len()..value.len() - post.len())?.trim();
    Some(KeyCandidate {
        key: key.to_string(),
        channel: KeyChannel::Header(String::new()),
        prefix: head.to_string(),
        suffix: tail.to_string(),
    })
}

/// 按 ADR 0009 顺序列出请求里全部密钥候选：自定义通道 → Authorization（整值、首个空白后部分）
/// → x-api-key → api-key → x-goog-api-key → ?api_key= → ?key=。空值不产出。纯函数。
pub(crate) fn key_candidates(
    headers: &axum::http::HeaderMap,
    query: Option<&str>,
    custom: &[(String, String)],
) -> Vec<KeyCandidate> {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let whole = |name: &str, v: &str| KeyCandidate {
        key: v.to_string(),
        channel: KeyChannel::Header(name.to_string()),
        prefix: String::new(),
        suffix: String::new(),
    };
    let mut out = Vec::new();
    for (name, template) in custom {
        if let Some(c) = header(name).and_then(|v| match_template(v, template)) {
            out.push(KeyCandidate {
                channel: KeyChannel::Header(name.clone()),
                ..c
            });
        }
    }
    if let Some(v) = header("authorization") {
        out.push(whole("authorization", v));
        if let Some((scheme, rest)) = v.split_once(char::is_whitespace)
            && !rest.trim().is_empty()
        {
            out.push(KeyCandidate {
                key: rest.trim().to_string(),
                channel: KeyChannel::Header("authorization".into()),
                prefix: format!("{scheme} "),
                suffix: String::new(),
            });
        }
    }
    for name in DEFAULT_KEY_HEADERS {
        if let Some(v) = header(name) {
            out.push(whole(name, v));
        }
    }
    for name in KEY_QUERY_PARAMS {
        if let Some(v) = query.and_then(|q| query_param(q, name)) {
            out.push(KeyCandidate {
                key: v,
                channel: KeyChannel::Query(name),
                prefix: String::new(),
                suffix: String::new(),
            });
        }
    }
    out.retain(|c| !c.key.is_empty());
    out
}

/// 从 URL query 串提取参数 `name` 的值（首个同名参数）。
/// 百分号解码 + `+`→空格（与浏览器表单编码兼容）；值空 / 参数缺席 → None。纯函数。
pub(crate) fn query_param(query: &str, name: &str) -> Option<String> {
    for pair in query.split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        if k != name {
            continue;
        }
        let decoded = percent_decode(v);
        return (!decoded.trim().is_empty()).then_some(decoded);
    }
    None
}

/// 极简 percent-decoding（`%XX` → 字节，`+` → 空格），仅用于 query 参数值。
/// 非法 `%` 序列（非两位 hex）原样保留字符，不报错——解码尽力而为，group key 匹配不上
/// 自然落 resolve_group 404，无需把编码错误上升为请求错误。
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 3 <= bytes.len() => {
                // bytes[i+1..i+3] 须是两位 hex
                let hex = &s[i + 1..i + 3];
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 按序拿密钥候选与分组 group_key 精确比对，第一个对上的胜（ADR 0009 决策 1）。
/// 返回命中的分组与定组用的那个候选（透传出站原位替换要用）。
pub(crate) async fn resolve_group(
    db: &Db,
    candidates: &[KeyCandidate],
) -> Option<(Group, KeyCandidate)> {
    let groups = match aidog_db::list_groups(db).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "resolve_group: list_groups failed");
            return None;
        }
    };
    let hit = candidates.iter().find_map(|c| {
        groups
            .iter()
            .find(|g| g.group_key == c.key)
            .map(|g| (g.clone(), c.clone()))
    });
    if hit.is_none() {
        tracing::warn!(
            candidate_count = candidates.len(),
            group_count = groups.len(),
            "resolve_group: no group matched any key candidate"
        );
    }
    hit
}

/// 加载全部透传平台声明的自定义密钥通道（DB 失败按无自定义通道处理并 warn，同 resolve_group 口径）。
pub(crate) async fn load_custom_key_channels(db: &Db) -> Vec<(String, String)> {
    match aidog_db::list_passthrough_extras(db).await {
        Ok(extras) => custom_key_channels(&extras),
        Err(e) => {
            tracing::warn!(error = %e, "load_custom_key_channels: list_passthrough_extras failed");
            Vec::new()
        }
    }
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
    let is_zen = is_opencode_zen(
        &platform.platform_type,
        &platform.base_url,
        Some(platform.endpoints.as_slice()),
    );
    opencode_zen_fallback(&platform.api_key, is_zen)
}

/// 「opencode_zen 平台」判定谓词（真值源）：Protocol::OpenCodeZen 或 base_url / 任一
/// endpoint base_url 含 `opencode.ai/zen`。proxy 与 fetch_models 共用，保证自定义
/// base_url 时兜底一致（model-test-proxy parity）。
pub fn is_opencode_zen(
    protocol: &Protocol,
    base_url: &str,
    endpoints: Option<&[super::models::PlatformEndpoint]>,
) -> bool {
    matches!(protocol, Protocol::OpenCodeZen)
        || base_url.to_lowercase().contains("opencode.ai/zen")
        || endpoints.is_some_and(|eps| {
            eps.iter()
                .any(|ep| ep.base_url.to_lowercase().contains("opencode.ai/zen"))
        })
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
pub(crate) async fn match_platform_by_host(
    db: &Db,
    connect_host: &str,
) -> Option<(u64, super::models::Platform)> {
    let target = connect_host.to_lowercase();
    let platforms = match aidog_db::list_platforms(db).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "match_platform_by_host: list_platforms failed");
            return None;
        }
    };
    platforms
        .iter()
        .filter(|p| p.status != super::models::PlatformStatus::Disabled)
        .find(|p| {
            endpoint_host(&p.base_url).as_deref() == Some(&target)
                || p.endpoints
                    .iter()
                    .any(|ep| endpoint_host(&ep.base_url).as_deref() == Some(&target))
        })
        .map(|p| (p.id, p.clone()))
}

/// 判定 path 是否为 AI API 端点（未匹配 token 时应 404，不走 fallback 直通）。
/// 复用既有 is_models_endpoint / is_responses_subendpoint / is_count_tokens_endpoint，
/// 补 chat/completions / messages / embeddings 等主路径。
/// path 已含可能的 /proxy 前缀；按「尾段匹配 / 包含子串」宽松判定，宁误判为 API（保留 404）
/// 也不漏判（漏判会把配错 token 的 API 流量旁路直通到原 host，违反 PRD 非目标）。
///
/// ponytail: Bug B 修复后 should_fallback_passthrough 不再调此函数（host 判定前置，path 不参与）。
/// 保留供 serve_plaintext 归属注入门（票 08：CONNECT 绑定 group 仅注入 AI API 端点请求）复用 +
/// 单测覆盖锁定 AI 端点识别语义（is_api_endpoint_covers_main_paths）。
pub(crate) fn is_api_endpoint(path: &str) -> bool {
    if super::is_models_endpoint(path) {
        return true;
    }
    if super::is_responses_subendpoint(path) {
        return true;
    }
    if super::is_count_tokens_endpoint(path) {
        return true;
    }
    // 主路径前缀匹配（find 跳过 /proxy 等前缀，与 detect_source_protocol 同款定位逻辑）。
    let api_path = if let Some(idx) = path.find("/v1/") {
        &path[idx..]
    } else {
        return false;
    };
    api_path.starts_with("/v1/messages")
        || api_path.starts_with("/v1/chat/completions")
        || api_path.starts_with("/v1/completions")
        || api_path.starts_with("/v1/responses")
        || api_path.starts_with("/v1/embeddings")
        || api_path.starts_with("/v1/images")
        || api_path.starts_with("/v1/audio")
        || api_path.starts_with("/v1/systemone")
}

/// fallback 直通判定：Host ≠ 代理自身监听 host → MITM 解密灌入 / forward proxy
/// absolute-form → 透明直通原 host。
/// Host = 代理自身直连（含错 token 探测代理自身）→ false（保留 resolve_group → 404 语义）。
///
/// 关键设计（Bug B 修法）：原顺序把 `is_api_endpoint(path)` early-return 置顶于 host 判定之前，
/// 致 MITM 解密灌入的 `/api/anthropic/v1/messages`（含 `/v1/messages` → is_api_endpoint=true）
/// 被拦死不直通 → 上游真实 key 落 resolve_group 返 None → 404。修后 host 判定**前置**：
/// host 非自身 → 直接 true（MITM 灌入直通原 host，Authorization 上游真实 key 由原 host 验证）；
/// host 自身 → false（保留 404 语义）。path 不再参与判定（已删除原入参）。
///
/// - `host`：请求 Host header（含端口，如 `www.baidu.com` 或 `127.0.0.1:9892`）。
/// - `listen_addr`：代理实际监听 (ip, port)（state.listen_addr）；None 走保守分支不直通。
pub(crate) fn should_fallback_passthrough(
    host: &str,
    listen_addr: Option<(std::net::IpAddr, u16)>,
) -> bool {
    let Some((ip, port)) = listen_addr else {
        // 测试 state 或未启动：保守不直通（保留原 404 语义，避免误旁路）。
        return false;
    };
    // 拆 Host header 的 host:port（host 不带端口时整个当作 host）。
    let (req_host, req_port) = match host.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok()),
        None => (host, None),
    };
    let req_host = req_host.trim().to_lowercase();
    // 代理自身监听 host 候选名（loopback 各形态）。
    let is_self_host = matches!(req_host.as_str(), "localhost" | "127.0.0.1" | "0.0.0.0");
    // 判定 Host 是否为「代理自身」：
    // - loopback 名 + 监听端口匹配 → 自身直连（端口不同 = 本机其他服务，视为非自身允许直通）。
    // - 非 loopback 名 → 字面量 IP 与 listen ip 比较（含非 loopback bind，如 LAN 192.168.x.x）。
    //   port 不匹配仍判自身（避免代理换端口探测被旁路；非 API 流量直连代理本就异常）。
    let is_self = if is_self_host {
        req_port.is_some_and(|rp| rp == port)
    } else if let Ok(req_ip) = req_host.parse::<std::net::IpAddr>() {
        req_ip == ip
    } else {
        false // 既非 loopback 名也非 IP 字面量 → MITM 解密灌入 / forward proxy absolute-form host
    };
    // host 自身 → false（保留 404）；非自身 → true（透明直通原 host）。
    !is_self
}

#[cfg(test)]
#[path = "test_endpoint.rs"]
mod test_endpoint;

pub(crate) use aidog_db::endpoint_host;
