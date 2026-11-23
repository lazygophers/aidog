//! 辅助模型请求（`/v1/embeddings`、`/v1/rerank`）的共享转发骨架，仿 `decision.rs`：
//! 解析 JSON → `select_candidates_gated(spec.kind)` → 重试编排 → OpenAI 系 endpoint →
//! `join_upstream_path(base_url, spec.api_path)` → 仅改写 `model` 为槽位值 → 转发 →
//! 2xx 回写响应 `model` 为客户端原名 → 记账。不支持流式。
//!
//! 种类差异全部收在 [`AuxSpec`]（路由种类 / 出站后缀 / 日志协议标签 / usage 提取器）；
//! 路由失败的 `blocked_reason` 取 `kind.no_platform_err()`，由 `decision::route_fail_response` 落库。

use super::attempt_plan::AttemptLoop;
use super::decision::route_fail_response;
use super::*;

/// 一种辅助模型请求的差异点（新增 rerank = 新增一个 const，不改骨架）。
pub(crate) struct AuxSpec {
    pub kind: RequestKind<'static>,
    /// 出站后缀，拼在 endpoint `base_url`（已含版本前缀）之后，如 `/embeddings`。
    pub api_path: &'static str,
    /// `source_protocol` / `target_protocol` 日志标签。
    pub label: &'static str,
    /// 2xx 响应体 → (input_tokens, output_tokens)。
    pub extract_usage: fn(&str) -> (i32, i32),
    /// 2xx 计量次数：(入站 JSON, 响应体) → 计数（rerank：Cohere `search_units`，缺省 1；
    /// speech：请求 `input` 字符数；transcriptions / translations：响应 `duration` 秒数）。
    /// None = 该种类不计量。
    pub extract_units: Option<fn(&Value, &str) -> i64>,
    /// 入站为 multipart/form-data（音频转写 / 翻译文件上传，body 非 JSON）。
    pub multipart: bool,
    /// 2xx 响应为二进制音频（speech）：日志落 `[binary N bytes]` 占位，响应原样回客户端。
    pub binary_response: bool,
    /// 客户端可要求流式（`stream:true` → SSE 中间图转发，票 05）；其余种类恒非流式。
    pub stream_capable: bool,
}

/// embedding：`usage.prompt_tokens` → input，output 恒 0（OpenAI 规范无输出 token）。
fn embedding_usage(body: &str) -> (i32, i32) {
    let (input, _output, ..) = extract_usage(body);
    (input, 0)
}

pub(crate) const EMBEDDING: AuxSpec = AuxSpec {
    kind: RequestKind::Embedding,
    api_path: "/embeddings",
    label: "embeddings",
    extract_usage: embedding_usage,
    extract_units: None,
    multipart: false,
    binary_response: false,
    stream_capable: false,
};

/// rerank：Jina `usage.total_tokens`（记 input）→ SiliconFlow `meta.tokens` →
/// Cohere `meta.billed_units` 的 token 字段；全缺 → (0, 0)（Cohere 按次计量见 [`rerank_search_units`]）。
fn rerank_usage(body: &str) -> (i32, i32) {
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    let n = |p: &Value, k: &str| p.get(k).and_then(Value::as_i64).unwrap_or(0) as i32;
    if let Some(t) = v.pointer("/usage/total_tokens").and_then(Value::as_i64) {
        return (t as i32, 0);
    }
    for path in ["/meta/tokens", "/meta/billed_units"] {
        if let Some(m) = v.pointer(path) {
            let (i, o) = (n(m, "input_tokens"), n(m, "output_tokens"));
            if i > 0 || o > 0 {
                return (i, o);
            }
        }
    }
    (0, 0)
}

/// rerank 计量次数：`meta.billed_units.search_units`，缺省按 1 次/请求。
fn rerank_search_units(_req: &Value, body: &str) -> i64 {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/meta/billed_units/search_units")
                .and_then(Value::as_i64)
        })
        .unwrap_or(1)
}

pub(crate) const RERANK: AuxSpec = AuxSpec {
    kind: RequestKind::Rerank,
    api_path: "/rerank",
    label: "rerank",
    extract_usage: rerank_usage,
    extract_units: Some(rerank_search_units),
    multipart: false,
    binary_response: false,
    stream_capable: false,
};

/// image：OpenAI Images usage（gpt-image 系 `usage.input_tokens` / `output_tokens`；旧形态
/// `prompt_tokens` 兼容）。计量张数：响应 `data` 长度 → 请求 `n`（推算）→ 官方缺省 1。
pub(crate) fn image_usage(body: &str) -> (i32, i32) {
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    let tok = |field: &str, legacy: &str| {
        v.pointer(&format!("/usage/{field}"))
            .or_else(|| v.pointer(&format!("/usage/{legacy}")))
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32
    };
    (tok("input_tokens", "prompt_tokens"), tok("output_tokens", "completion_tokens"))
}

/// image 计量张数：响应 `data` 数组长度；缺失按请求 `n`（OpenAI 官方缺省 1）推算（票 03 口径：
/// 能从请求或结果推算则记录）。
fn image_units(req: &Value, body: &str) -> i64 {
    if let Ok(v) = serde_json::from_str::<Value>(body)
        && let Some(data) = v.get("data").and_then(Value::as_array)
        && !data.is_empty()
    {
        return data.len() as i64;
    }
    req.get("n").and_then(Value::as_i64).unwrap_or(1).max(1)
}

// ══ 票 07：费用状态（known / estimated / unknown / incomplete，票 03 口径）══

/// 生图非流式 2xx 费用状态：响应 token 用量或 `data` 张数在场 → 已知；
/// 全缺（张数只能按请求 n / 官方缺省推算）→ 估算（票 03：可推算则记录并标估算）。
fn image_fee_state(input: i32, output: i32, body: &str) -> &'static str {
    if input > 0 || output > 0 {
        return "known";
    }
    let measured = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("data").and_then(Value::as_array).map(|a| !a.is_empty()))
        .unwrap_or(false);
    if measured {
        "known"
    } else {
        "estimated"
    }
}

/// 生图失败终态费用状态（票 03：有已收用量 → 不完整；无用量 → 未知）。
pub(super) fn failure_fee_state(input: i32, output: i32) -> &'static str {
    if input > 0 || output > 0 {
        "incomplete"
    } else {
        "unknown"
    }
}

/// 生图流式终态费用状态。partial 每张按官方附加费计 100 输出 token（billable，
/// 见 [`ImageStreamGuard`]），有已计费用 → 不完整；全无 → 未知；completed → 已知。
fn stream_fee_state(completed: bool, input: i32, output_with_surcharge: i32) -> &'static str {
    if completed {
        "known"
    } else if input > 0 || output_with_surcharge > 0 {
        "incomplete"
    } else {
        "unknown"
    }
}

pub(crate) const IMAGE: AuxSpec = AuxSpec {
    kind: RequestKind::Image,
    api_path: "/images/generations",
    label: "images",
    extract_usage: image_usage,
    extract_units: Some(image_units),
    multipart: false,
    binary_response: false,
    stream_capable: true,
};

/// image edits：`/v1/images/edits` 图生图编辑（票 05）。multipart 入站原样透传
///（与音频转写同形），仅 `model` part 改写为槽位模型。
pub(crate) const IMAGE_EDITS: AuxSpec = AuxSpec {
    kind: RequestKind::Image,
    api_path: "/images/edits",
    label: "image_edits",
    extract_usage: image_usage,
    extract_units: Some(image_units),
    multipart: true,
    binary_response: false,
    stream_capable: true,
};

/// MCP 工具 `generate_image`（生图-模型类型 票 08）：调用方分组的 image 槽位链转发，
/// 复用 [`handle_aux_model`] 的票 04 链（路由 / 重试 / 限频 / 额度 / 计费 / proxy_log），
/// 旧工具级 provider 配置已移除（存量 `mcp_tools` 设置里的 `image` 键由 serde 忽略）。
/// 入参 prompt / size 组装 OpenAI images 请求体（model 由槽位链覆盖，n 固定 1）；
/// 出参 markdown 图片链接（条目 `url` 或 `b64_json`→data URL）。工具日志照旧写 tool_log。
pub(crate) async fn mcp_generate_image(
    state: &Arc<ProxyState>,
    group: &Group,
    args: &Value,
) -> Result<String, String> {
    let prompt = args.get("prompt").and_then(Value::as_str).unwrap_or_default();
    let mut body = serde_json::json!({ "model": "", "prompt": prompt, "n": 1 });
    if let Some(s) = args
        .get("size")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        body["size"] = serde_json::json!(s);
    }
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    let created = aidog_db::now();
    let mut log = ProxyLog {
        id: uuid::Uuid::new_v4().simple().to_string(),
        created_at: created,
        updated_at: created,
        group_key: group.group_key.clone(),
        ..Default::default()
    };
    let (lang, log_settings) = {
        let cache = state.settings_cache.read().await;
        (cache.lang, cache.log_settings.clone())
    };
    let ctx = super::call_scope::ReqCtx {
        log_settings: &log_settings,
        start: std::time::Instant::now(),
        lang,
        client: super::ClientGate::On(super::client_detect::client_probe(&Default::default())),
    };
    let resp = handle_aux_model(&IMAGE, state, &mut log, group, &bytes, None, ctx).await;
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024)
        .await
        .map_err(|e| format!("read response body error: {e}"))?;
    let outcome = if !status.is_success() {
        Err(String::from_utf8_lossy(&body_bytes).trim().to_string())
    } else {
        links_from_data(&body_bytes)
    };
    // 工具日志照旧（trigger=mcp，与 web_search / web_fetch 同一 tool_log 表）。
    let tctx = crate::tool_exec::ToolCtx {
        db: &state.db,
        trigger: crate::tool_exec::TRIGGER_MCP,
        group: &group.name,
        platform: "",
    };
    crate::tool_exec::write_log(
        &tctx,
        "generate_image",
        prompt.to_string(),
        0,
        &[],
        std::time::Instant::now(),
        &outcome,
    )
    .await;
    outcome
}

/// 上游响应 `data[]` → markdown 图片链接（条目 `url`，或 `b64_json` → data URL 内嵌）。
fn links_from_data(body: &[u8]) -> Result<String, String> {
    let v: Value = serde_json::from_slice(body).map_err(|e| format!("response json parse error: {e}"))?;
    let items = v["data"]
        .as_array()
        .ok_or_else(|| format!("响应缺少 data 数组: {}", String::from_utf8_lossy(&body[..body.len().min(200)])))?;
    let mut links = Vec::new();
    for item in items {
        if let Some(u) = item["url"].as_str().filter(|u| !u.is_empty()) {
            links.push(format!("![generated image]({u})"));
        } else if let Some(b64) = item["b64_json"].as_str().filter(|b| !b.is_empty()) {
            links.push(format!("![generated image](data:image/png;base64,{b64})"));
        }
    }
    if links.is_empty() {
        return Err("响应条目缺少 url / b64_json 字段，无法生成图片链接".into());
    }
    Ok(links.join("\n\n"))
}

/// audio 请求无 token usage（计费走 char / second 计数，issue #167）。
fn no_usage(_body: &str) -> (i32, i32) {
    (0, 0)
}

/// speech 计量：请求 `input` 字段字符数（char 条目，OpenAI TTS 按输入字符计价）。
fn speech_units(req: &Value, _resp: &str) -> i64 {
    req.get("input")
        .and_then(Value::as_str)
        .map(|s| s.chars().count() as i64)
        .unwrap_or(0)
}

/// transcriptions / translations 计量：响应 `duration` 秒数（second 条目；
/// verbose_json / 部分供应商返回；缺省 0 = 不可估算，不编造）。
fn asr_units(_req: &Value, resp: &str) -> i64 {
    serde_json::from_str::<Value>(resp)
        .ok()
        .and_then(|v| {
            v.get("duration")
                .and_then(Value::as_f64)
                .map(|d| d.round() as i64)
        })
        .unwrap_or(0)
}

pub(crate) const AUDIO_SPEECH: AuxSpec = AuxSpec {
    kind: RequestKind::Audio,
    api_path: "/audio/speech",
    label: "audio_speech",
    extract_usage: no_usage,
    extract_units: Some(speech_units),
    multipart: false,
    binary_response: true,
    stream_capable: false,
};

pub(crate) const AUDIO_TRANSCRIPTIONS: AuxSpec = AuxSpec {
    kind: RequestKind::Audio,
    api_path: "/audio/transcriptions",
    label: "audio_transcriptions",
    extract_usage: no_usage,
    extract_units: Some(asr_units),
    multipart: true,
    binary_response: false,
    stream_capable: false,
};

pub(crate) const AUDIO_TRANSLATIONS: AuxSpec = AuxSpec {
    kind: RequestKind::Audio,
    api_path: "/audio/translations",
    label: "audio_translations",
    extract_usage: no_usage,
    extract_units: Some(asr_units),
    multipart: true,
    binary_response: false,
    stream_capable: false,
};

/// 音频端点 path → AuxSpec（handler.rs 分流用；后缀判定，容忍 `/proxy` 前缀与尾 `/`）。
pub(crate) fn audio_spec(path: &str) -> Option<&'static AuxSpec> {
    let p = path.trim_end_matches('/');
    if p.ends_with("/audio/speech") {
        Some(&AUDIO_SPEECH)
    } else if p.ends_with("/audio/transcriptions") {
        Some(&AUDIO_TRANSCRIPTIONS)
    } else if p.ends_with("/audio/translations") {
        Some(&AUDIO_TRANSLATIONS)
    } else {
        None
    }
}

/// multipart/form-data 取文本字段值：定位 `name="<field>"`，跳过首个空行读到下一边界。
/// 音频入站只取 model 名用于路由与日志，body 原样透传，不做完整 multipart 解析。
fn multipart_form_field(bytes: &[u8], field: &str) -> String {
    multipart_form_value_span(bytes, field)
        .map(|(s, e)| String::from_utf8_lossy(&bytes[s..e]).trim().to_string())
        .unwrap_or_default()
}

/// multipart part 值段定位（与 [`multipart_form_field`] 同判定）：返回值段 [start, end) 字节区间。
fn multipart_form_value_span(bytes: &[u8], field: &str) -> Option<(usize, usize)> {
    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }
    let marker = format!("name=\"{field}\"");
    let p = find(bytes, marker.as_bytes())?;
    let rest = &bytes[p + marker.len()..];
    let s = find(rest, b"\r\n\r\n")?;
    let body = &rest[s + 4..];
    let e = find(body, b"\r\n--").unwrap_or(body.len());
    Some((p + marker.len() + s + 4, p + marker.len() + s + 4 + e))
}

/// 从 Content-Type 解析 multipart boundary（容忍引号 / 尾随参数）。
fn boundary_from_content_type(content_type: &str) -> Option<String> {
    let p = content_type.to_ascii_lowercase().find("boundary=")?;
    let raw = &content_type[p + "boundary=".len()..];
    let end = raw.find(';').unwrap_or(raw.len());
    Some(raw[..end].trim().trim_matches('"').to_string())
}

// ══ 票 05：图片编辑与流式 ══

/// 编辑请求单文件上限：<50MB（官方 openai-python images.py edit() 文档串
/// "each image should be a png, webp, or jpg file less than 50MB"，2026-11-24 取证）。
const IMAGE_EDIT_MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
/// 参考图张数上限：官方 "You can provide up to 16 images"（同上文档串）。
/// Codex 客户端侧另有 ≤5 张引用上限（research/codex-生图.md §3），是客户端形态不是端点契约。
const IMAGE_EDIT_MAX_IMAGES: usize = 16;
/// mask / markup 上限：<4MB 且须为 PNG（官方 mask 文档串："less than 4MB" / markup "transparent PNG"）。
const IMAGE_EDIT_MAX_MASK_BYTES: usize = 4 * 1024 * 1024;

/// 编辑表单官方约束校验：image ≤16 张、png/webp/jpg、单文件 <50MB；mask/markup png <4MB。
/// 违规返回错误文案（400），合法 None。
fn validate_image_edit_form(bytes: &[u8], content_type: Option<&str>) -> Option<String> {
    let boundary = match content_type.and_then(boundary_from_content_type) {
        Some(b) if !b.is_empty() => b,
        _ => return Some("multipart boundary missing".to_string()),
    };
    let delim = format!("\r\n--{boundary}");
    let mut images = 0usize;
    // 迭代 part：定位每个 delim，part = 头段（到 \r\n\r\n）+ 值段（到下一 delim）。
    let mut cursor = 0usize;
    while let Some(off) = find_sub(&bytes[cursor..], delim.as_bytes()) {
        let start = cursor + off + delim.len();
        // part 头结束（跳过可选的尾随 \r\n——终结边界后无头段）
        let Some(hdr_end) = find_sub(&bytes[start..], b"\r\n\r\n") else {
            break;
        };
        let headers = String::from_utf8_lossy(&bytes[start..start + hdr_end]).to_string();
        let value_start = start + hdr_end + 4;
        let value_end = find_sub(&bytes[value_start..], delim.as_bytes())
            .map(|e| value_start + e)
            .unwrap_or(bytes.len());
        let (name, filename) = part_disposition(&headers);
        let size = value_end - value_start;
        match name.as_str() {
            "image" | "image[]" => {
                images += 1;
                if !image_file_ext_ok(filename.as_deref()) {
                    return Some(format!(
                        "image file must be png, webp, or jpg (OpenAI Images edits contract), got {filename:?}"
                    ));
                }
                if size >= IMAGE_EDIT_MAX_FILE_BYTES {
                    return Some(format!(
                        "image file must be less than 50MB (OpenAI Images edits contract), got {size} bytes"
                    ));
                }
            }
            "mask" | "markup" => {
                let is_png = filename
                    .as_deref()
                    .and_then(|f| f.rsplit('.').next())
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"));
                if !is_png {
                    return Some("mask/markup must be a PNG file (OpenAI Images edits contract)".to_string());
                }
                if size >= IMAGE_EDIT_MAX_MASK_BYTES {
                    return Some(format!(
                        "mask/markup must be less than 4MB (OpenAI Images edits contract), got {size} bytes"
                    ));
                }
            }
            _ => {}
        }
        cursor = value_end;
        if cursor >= bytes.len() {
            break;
        }
    }
    if images > IMAGE_EDIT_MAX_IMAGES {
        return Some(format!(
            "up to {IMAGE_EDIT_MAX_IMAGES} images per edit request (OpenAI Images edits contract), got {images}"
        ));
    }
    None
}

pub(crate) fn find_sub(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// 从 part 头段取 (name, filename)。
fn part_disposition(headers: &str) -> (String, Option<String>) {
    let mut name = String::new();
    let mut filename = None;
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(p) = lower.find("name=\"") {
            let rest = &line[p + 6..];
            if let Some(e) = rest.find('"') {
                name = rest[..e].to_string();
            }
        }
        if let Some(p) = lower.find("filename=\"") {
            let rest = &line[p + 10..];
            if let Some(e) = rest.find('"') {
                filename = Some(rest[..e].to_string());
            }
        }
    }
    (name, filename)
}

fn image_file_ext_ok(filename: Option<&str>) -> bool {
    filename
        .and_then(|f| f.rsplit('.').next())
        .map(|e| ["png", "jpg", "jpeg", "webp"].iter().any(|x| e.eq_ignore_ascii_case(x)))
        .unwrap_or(true) // 无文件名（部分客户端直传字节）不在端点可判范围，交上游校验
}

/// multipart 表单 `model` part 值改写为槽位模型，其余字节原样保留（票 05：
/// 编辑路由用 image 槽位，body 不可整体重序列化）。
fn rewrite_multipart_model(bytes: &[u8], boundary: &str, new_model: &str) -> Vec<u8> {
    let _ = boundary;
    match multipart_form_value_span(bytes, "model") {
        Some((s, e)) => {
            let mut out = Vec::with_capacity(bytes.len() - (e - s) + new_model.len());
            out.extend_from_slice(&bytes[..s]);
            out.extend_from_slice(new_model.as_bytes());
            out.extend_from_slice(&bytes[e..]);
            out
        }
        None => bytes.to_vec(),
    }
}

/// 客户端要求流式（JSON `stream:true`；multipart 表单 `stream` = true/1）。
fn image_stream_requested(spec: &AuxSpec, req: &AuxRequest<'_>) -> bool {
    if spec.multipart {
        matches!(
            multipart_form_field(req.bytes, "stream").to_ascii_lowercase().as_str(),
            "true" | "1"
        )
    } else {
        req.value.get("stream").and_then(Value::as_bool).unwrap_or(false)
    }
}

/// 官方契约：GIF/SVG 输出不支持流式与 partial（openai-python images.py：GIF "does not
/// support streaming, partial images"；SVG "not support streaming"，2026-11-24 取证）。
/// 用户拍板（票 00）：不能提供中间图时明确报不支持，不静默降级。
/// 冲突返回 output_format 值，无冲突（含非流式请求）None。
fn image_stream_format_conflict(spec: &AuxSpec, req: &AuxRequest<'_>) -> Option<String> {
    if !image_stream_requested(spec, req) {
        return None;
    }
    let fmt = if spec.multipart {
        multipart_form_field(req.bytes, "output_format")
    } else {
        req.value
            .get("output_format")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    matches!(fmt.as_str(), "gif" | "svg").then_some(fmt)
}

/// 辅助模型请求主入口（handler.rs 在 count_tokens / 决策分流旁调用）。
/// `content_type`：入站 Content-Type（multipart 音频请求原样透传给上游，含 boundary）。
pub(crate) async fn handle_aux_model(
    spec: &AuxSpec,
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    group: &Group,
    bytes: &[u8],
    content_type: Option<&str>,
    ctx: super::call_scope::ReqCtx<'_>,
) -> Response {
    let super::call_scope::ReqCtx {
        log_settings,
        start,
        lang,
        client,
    } = ctx;
    log.source_protocol = spec.label.to_string();
    log.target_protocol = spec.label.to_string();

    let req_value: Value = if spec.multipart {
        // multipart（音频转写 / 翻译）body 非 JSON：只取 model 字段做路由，body 原样透传。
        Value::Null
    } else {
        match serde_json::from_slice(bytes) {
            Ok(v) => v,
            Err(e) => {
                log.response_body = format!("parse request json error: {e}");
                log.status_code = 400;
                log.done = true;
                log.duration_ms = start.elapsed().as_millis() as i32;
                upsert_log(state, log, log_settings).await;
                let mut r = (
                    StatusCode::BAD_REQUEST,
                    format!("{}: {e}", i18n::t(lang, ErrorKey::ParseJson)),
                )
                    .into_response();
                inject_trace_header(&mut r);
                return r;
            }
        }
    };
    let requested_model = if spec.multipart {
        multipart_form_field(bytes, "model")
    } else {
        req_value
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string()
    };
    log.model = requested_model.clone();
    let incoming = AuxRequest {
        value: &req_value,
        bytes,
        model: &requested_model,
        content_type,
    };
    if spec.label == "image_edits"
        && let Some(err) = validate_image_edit_form(bytes, content_type)
    {
        log.response_body = err.clone();
        log.status_code = 400;
        log.done = true;
        log.duration_ms = start.elapsed().as_millis() as i32;
        upsert_log(state, log, log_settings).await;
        let mut r = (StatusCode::BAD_REQUEST, err).into_response();
        inject_trace_header(&mut r);
        return r;
    }
    if matches!(spec.label, "images" | "image_edits")
        && let Some(fmt) = image_stream_format_conflict(spec, &incoming)
    {
        let err = format!("streaming is unsupported for output_format={fmt}");
        log.response_body = err.clone();
        log.status_code = 400;
        log.done = true;
        log.duration_ms = start.elapsed().as_millis() as i32;
        upsert_log(state, log, log_settings).await;
        let mut r = (StatusCode::BAD_REQUEST, err).into_response();
        inject_trace_header(&mut r);
        return r;
    }

    let sched_settings = aidog_db::get_scheduling_settings(&state.db).await;
    let sched_ctx = ScheduleCtx {
        scheduler: &state.scheduler,
        sticky: &state.sticky,
        settings: &sched_settings,
        sticky_key: Some(format!("{}|{}", group.group_key, spec.label)),
        // multipart 无 JSON 形态 body，不进限频估算（决策 / 搜索 rate_body: None 同口径）。
        rate_body: (!spec.multipart).then_some(&req_value),
    };
    let candidate_set = match select_candidates_gated(
        &state.db,
        group,
        &requested_model,
        Some(&sched_ctx),
        spec.kind,
        client.clone(),
    )
    .await
    {
        Ok(c) => {
            log.routing = Some(c.routing.clone());
            c
        }
        Err(e) => {
            log.routing = Some(e.routing.clone());
            tracing::warn!(group = %group.name, kind = spec.label, error = %e, "aux route failed");
            return route_fail_response(state, log, log_settings, &e, start, lang).await;
        }
    };
    let candidates = candidate_set.candidates;

    let mut attempt_loop = AttemptLoop::new(
        group.max_retries,
        spec.kind,
        &requested_model,
        client,
        candidates,
    );
    let mut attempts: Vec<ProxyAttempt> = Vec::new();
    // 平台限频占位跨调用保存：同平台连续调用复用（平台级只占一次），换平台时释放上一个。
    let mut rate_slot = RateSlot::default();

    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let req_timeout = if system_timeout.request_timeout_secs > 0 {
        system_timeout.request_timeout_secs
    } else {
        60
    };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 {
        system_timeout.connect_timeout_secs
    } else {
        10
    };

    while let Some(pick) = attempt_loop
        .next(&state.db, group, &sched_ctx, log, attempts.len())
        .await
    {
        let route = pick.route;
        let attempt_start = std::time::Instant::now();
        let attempt_ts = aidog_db::now();

        // 平台限频占位（与 forward_attempt 同口径）：begin 占平台级，enter_model 占模型级；
        // 占位失败不发请求（不计次数）：平台级排除整个平台，模型级只排除该组合。
        let _access = match rate_slot.enter(
            state,
            log,
            &mut attempts,
            RateSlotCtx {
                route: &route,
                requested_model: &requested_model,
                req_value: &req_value,
                model_alt: pick.model_alt,
                label: spec.label,
            },
        ) {
            Ok(access) => access,
            Err(()) => {
                attempt_loop.failed(&mut attempts, false);
                continue;
            }
        };
        // 额度窗口闸门（extra.quota_windows 执行）：超限不发上游请求（不计调用次数），排除整个平台。
        if !crate::gateway::quota_window::gate_attempt(
            &state.db,
            log,
            &mut attempts,
            &route.platform,
            &route.target_model,
            pick.model_alt,
        )
        .await
        {
            attempt_loop.failed(&mut attempts, false);
            continue;
        }
        let capped = attempt_loop.begin_call(log, &attempts, route.platform.id);
        let scope = super::call_scope::CallScope {
            state,
            group,
            route: &route,
            log_settings,
            lang,
            start,
            attempt_start,
            attempt_ts,
            is_last_candidate: capped,
            candidate_total: pick.candidate_total,
            has_next_model: pick.model_alt && !capped,
            breaker_th: super::scheduling::BreakerThresholds::resolve(
                &sched_settings,
                &route.platform,
                group.routing_mode,
            ),
            proxy_client: &proxy_client,
            req_timeout,
            conn_timeout,
        };
        let out = aux_attempt(
            &scope,
            spec,
            log,
            &mut attempts,
            &AuxRequest {
                value: &req_value,
                bytes,
                model: &requested_model,
                content_type,
            },
            &mut false,
        )
        .await;
        match out {
            AttemptOutcome::Respond(r) => return r,
            AttemptOutcome::Next { connect_failed } => {
                attempt_loop.failed(&mut attempts, connect_failed)
            }
        }
    }

    AttemptLoop::exhausted(state, log, &mut attempts, start, log_settings, lang).await
}

/// 一次辅助请求的入站快照（JSON 值 / 原始字节 / 客户端模型名 / Content-Type）。
struct AuxRequest<'a> {
    value: &'a Value,
    bytes: &'a [u8],
    model: &'a str,
    content_type: Option<&'a str>,
}

/// 单候选尝试（endpoint 选择 → 建请求 → 发送 → 非 2xx 沿用 handle_non_success → 2xx 记账终态）。
async fn aux_attempt(
    scope: &super::call_scope::CallScope<'_>,
    spec: &AuxSpec,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    req: &AuxRequest<'_>,
    switched: &mut bool,
) -> AttemptOutcome {
    let (route, log_settings, group) = (scope.route, scope.log_settings, scope.group);
    let (is_last_candidate, has_next_model) = (scope.is_last_candidate, scope.has_next_model);
    let stream_requested = spec.stream_capable && image_stream_requested(spec, req);
    // 票 06：gemini 方言平台（extra.image_dialect=gemini）走 Interactions 出站。
    let gemini =
        matches!(spec.kind, RequestKind::Image)
            && aidog_db::image_dialect_is_gemini(&route.platform.extra);
    // 客户端要求流式中间图 + gemini 方言（Interactions 流为 step 事件，无 partial 图语义，
    // 官方 streaming 页无 image_generation.partial_image 对应物）→ 明确报不支持
    //（票 00：不伪装不降级）：按平台排除换下一候选，末位 501。确定性配置能力缺失，
    // 不记熔断 / 降权（非平台故障）。
    if gemini && stream_requested {
        attempts.push(ProxyAttempt {
            kind: Some("stream_unsupported".into()),
            exclusion: Some(ExcludeScope::Platform),
            chain_stop: has_next_model.then(|| "service".to_string()),
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: 0,
            error: "gemini image dialect does not support streaming partial images (Interactions API has no partial-image events)".into(),
            duration_ms: scope.attempt_start.elapsed().as_millis() as i64,
            ts: scope.attempt_ts,
            model: Some(route.target_model.clone()),
            ..Default::default()
        });
        tracing::warn!(platform = %route.platform.name, "gemini image stream unsupported: failover");
        if !is_last_candidate {
            return AttemptOutcome::Next { connect_failed: false };
        }
        return image_terminal(
            scope,
            log,
            attempts,
            route,
            StatusCode::NOT_IMPLEMENTED,
            "image streaming not supported by gemini dialect (no intermediate images available)"
                .to_string(),
        )
        .await;
    }
    // 票 06：gemini 方言平台跳过 OpenAI 系 endpoint 选择，走平台主 base_url + /interactions
    //（Gemini base_url 已含版本前缀 /v1beta，官方端点 POST {base}/v1beta/interactions）。
    let (base_url, api_path) = if gemini {
        (
            route.platform.base_url.clone(),
            super::image_dialect::GEMINI_API_PATH,
        )
    } else {
        // endpoint：OpenAI 系优先，否则回退平台主 base_url（零端点平台）。
        // supported_protocols 约束（4dbbc7c4 同族）：模型支持子集内硬序，无数据（None→空集）全回落。
        let supported = aidog_db::model_supported_protocols(
            &route.platform.platform_type.wire_str(),
            &route.target_model,
        );
        let ep = select_endpoint_for_openai_family_model(
            &route.platform.endpoints,
            supported.unwrap_or(&[]),
        );
        (
            ep.map(|e| e.base_url.clone())
                .unwrap_or_else(|| route.platform.base_url.clone()),
            spec.api_path,
        )
    };
    // URL = base_url（含版本前缀）+ 出站后缀，禁额外拼接。
    let url = join_upstream_path(&base_url, api_path);

    // 出站 Content-Type：gemini 方言恒 JSON（multipart 入站已转换）；multipart 原样透传
    //（含 boundary），其余 JSON。
    let out_ct = if gemini || !spec.multipart {
        "application/json".to_string()
    } else {
        req.content_type.unwrap_or("multipart/form-data").to_string()
    };

    // 出站 body：gemini 方言先跨协议转换（OpenAI→Interactions，不可等价参数明确报错，
    // 票 00）；JSON 请求仅改写 model（槽位值），其余字段原样；multipart 原样透传
    //（模型名由路由层 target_model 决定，body 不可重序列化）。
    let actual_model = route.target_model.clone();
    let upstream_body: Vec<u8> = if gemini {
        let converted = if spec.multipart {
            let boundary = req
                .content_type
                .and_then(boundary_from_content_type)
                .unwrap_or_default();
            super::image_dialect::edit_to_gemini(req.bytes, &boundary, &actual_model)
        } else {
            super::image_dialect::generation_to_gemini(req.value, &actual_model)
        };
        match converted {
            Ok(v) => serde_json::to_vec(&v).unwrap_or_default(),
            // 不可等价表达（票 00 明确报错）：按平台排除试其他候选，末位直返 400 点名参数。
            Err(msg) => {
                attempts.push(ProxyAttempt {
                    kind: Some("param_incompatible".into()),
                    exclusion: Some(ExcludeScope::Platform),
                    chain_stop: has_next_model.then(|| "service".to_string()),
                    platform_id: route.platform.id,
                    platform_name: route.platform.name.clone(),
                    status_code: 0,
                    error: msg.clone(),
                    duration_ms: scope.attempt_start.elapsed().as_millis() as i64,
                    ts: scope.attempt_ts,
                    model: Some(actual_model.clone()),
                    ..Default::default()
                });
                tracing::warn!(platform = %route.platform.name, error = %msg, "image params not expressible in gemini dialect: failover");
                if !is_last_candidate {
                    return AttemptOutcome::Next { connect_failed: false };
                }
                return image_terminal(scope, log, attempts, route, StatusCode::BAD_REQUEST, msg)
                    .await;
            }
        }
    } else if spec.multipart {
        if matches!(spec.kind, RequestKind::Image) {
            // 编辑（票 05）：model part 改写为槽位模型，其余字节原样保留。
            let boundary = req
                .content_type
                .and_then(boundary_from_content_type)
                .unwrap_or_default();
            rewrite_multipart_model(req.bytes, &boundary, &actual_model)
        } else {
            // 音频转写 / 翻译：无专用槽，模型名随请求直转，body 原样透传。
            req.bytes.to_vec()
        }
    } else {
        let mut upstream_body = req.value.clone();
        if let Some(obj) = upstream_body.as_object_mut() {
            obj.insert("model".to_string(), Value::String(actual_model.clone()));
        }
        serde_json::to_vec(&upstream_body).unwrap_or_default()
    };

    log.platform_id = route.platform.id;
    log.actual_model = actual_model.clone();
    log.upstream_request_url = url.clone();
    log.upstream_request_headers = if gemini {
        // Gemini API key 走 x-goog-api-key 头（官方 REST 示例）；Bearer 为 OAuth 专用不发。
        format!(
            r#"{{"x-goog-api-key":"[REDACTED]","content-type":"{out_ct}","api-revision":"{}"}}"#,
            super::image_dialect::GEMINI_API_REVISION
        )
    } else {
        format!(
            r#"{{"authorization":"[REDACTED]","content-type":"{out_ct}"}}"#
        )
    };
    log.upstream_request_body = if log_settings.log_upstream_request && (!spec.multipart || gemini)
    {
        String::from_utf8_lossy(&upstream_body).to_string()
    } else {
        String::new()
    };

    let client = scope.build_client().await;
    let mut rb = client.post(&url).header("Content-Type", out_ct);
    if gemini {
        rb = rb
            .header("x-goog-api-key", &route.platform.api_key)
            .header(
                "Api-Revision",
                super::image_dialect::GEMINI_API_REVISION,
            );
    } else {
        rb = rb.header(
            "Authorization",
            format!("Bearer {}", route.platform.api_key),
        );
    }
    let rb = rb.body(upstream_body.clone());
    tracing::info!(group = %group.name, platform = %route.platform.name, model = %actual_model, url = %url, kind = spec.label, "aux upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        // transport 错误：judge(Transport) + apply 记账（connect 计熔断 + 降权，其余仅降权）。
        Err(e) => return scope.transport_failed(log, attempts, &e, &url).await,
    };

    let status = resp.status();
    let resp_ct = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    log.upstream_status_code = status.as_u16() as i32;
    log.upstream_response_headers =
        upstream_headers_json_for_log(resp.headers(), log_settings.log_upstream_request);

    // 非 2xx 沿用 handle_non_success（429 冷却 / 5xx·529 换平台计熔断 / 401 auth 冷却 / 400·422 直返）。
    if !status.is_success() {
        return match handle_non_success(
            resp,
            status,
            scope.state,
            log,
            attempts,
            route,
            group,
            &scope.breaker_th,
            NonSuccessCtx {
                url: &url,
                start: scope.start,
                attempt_start: scope.attempt_start,
                attempt_ts: scope.attempt_ts,
                is_last_candidate,
                log_settings,
                requested_model: req.model,
                has_next_model,
                switched,
                ttfb_ms: None,
                candidate_total: scope.candidate_total,
                // 票 07：生图审核拒绝按模型类裁决（judge 生图特例分岔）。
                image: matches!(spec.kind, RequestKind::Image),
            },
        )
        .await
        {
            AttemptOutcome::Respond(r) => AttemptOutcome::Respond(r),
            AttemptOutcome::Next { .. } => AttemptOutcome::Next {
                connect_failed: false,
            },
        };
    }

    // ── 2xx 流式分支（票 05）：客户端要求流式时上游必须回 SSE；回 JSON = 上游不能提供
    // 中间图 → 不静默降级，按服务类失败换平台，末位 501 明确报不支持。──
    if stream_requested {
        if !resp_ct.contains("text/event-stream") {
            attempts.push(ProxyAttempt {
                kind: Some("stream_unsupported".into()),
                error_class: Some("service".into()),
                chain_stop: has_next_model.then(|| "service".to_string()),
                effects: vec!["penalty_server".into()],
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: status.as_u16() as i32,
                error: format!("upstream returned non-stream response for stream=true ({resp_ct})"),
                duration_ms: scope.attempt_start.elapsed().as_millis() as i64,
                ts: scope.attempt_ts,
                model: Some(actual_model.clone()),
                ..Default::default()
            });
            scope.state.scheduler.record_penalty(
                route.platform.id,
                super::scheduling::PenaltyTier::Server,
                aidog_db::now(),
            );
            tracing::warn!(platform = %route.platform.name, "image stream unsupported: non-SSE 2xx, failover");
            if !is_last_candidate {
                return AttemptOutcome::Next { connect_failed: false };
            }
            return image_terminal(
                scope,
                log,
                attempts,
                route,
                StatusCode::NOT_IMPLEMENTED,
                "image streaming not supported by upstream (no intermediate images available)"
                    .to_string(),
            )
            .await;
        }
        return image_stream_respond(scope, log, attempts, resp, resp_ct).await;
    }

    // ── 2xx：非流式。gemini 方言先把 interaction 转 ImagesResponse（票 06）；
    // JSON 响应 model 回写客户端原名；二进制音频原样回客户端。──
    let body = resp.bytes().await.unwrap_or_default();
    // gemini 方言：0 图 / 坏 JSON = 上游契约不符 → 服务类失败换平台，末位 502。
    let body: Vec<u8> = if gemini {
        match serde_json::from_slice::<Value>(&body)
            .map_err(|e| e.to_string())
            .and_then(|v| super::image_dialect::interaction_to_openai(&v, &actual_model))
            .and_then(|v| serde_json::to_vec(&v).map_err(|e| e.to_string()))
        {
            Ok(b) => b,
            Err(e) => {
                let msg = format!("gemini interaction response not convertible: {e}");
                attempts.push(ProxyAttempt {
                    kind: Some("response_unconvertible".into()),
                    error_class: Some("service".into()),
                    exclusion: Some(ExcludeScope::Platform),
                    chain_stop: has_next_model.then(|| "service".to_string()),
                    effects: vec!["penalty_server".into()],
                    platform_id: route.platform.id,
                    platform_name: route.platform.name.clone(),
                    status_code: status.as_u16() as i32,
                    error: msg.clone(),
                    duration_ms: scope.attempt_start.elapsed().as_millis() as i64,
                    ts: scope.attempt_ts,
                    model: Some(actual_model.clone()),
                    ..Default::default()
                });
                scope.state.scheduler.record_penalty(
                    route.platform.id,
                    super::scheduling::PenaltyTier::Server,
                    aidog_db::now(),
                );
                tracing::warn!(platform = %route.platform.name, error = %e, "gemini interaction response unconvertible: failover");
                if !is_last_candidate {
                    return AttemptOutcome::Next { connect_failed: false };
                }
                return image_terminal(
                    scope,
                    log,
                    attempts,
                    route,
                    StatusCode::BAD_GATEWAY,
                    msg,
                )
                .await;
            }
        }
    } else {
        body.to_vec()
    };
    let body_str = String::from_utf8_lossy(&body).to_string();
    let client_body = if spec.binary_response || req.model.is_empty() {
        body.to_vec()
    } else {
        replace_model_in_json(&body, req.model)
    };

    // 成功记账（CallScope 统一：延迟 EMA + last_error 清空 + auto_disabled 恢复 + attempt 留痕，
    // 对齐 decision.rs / forward.rs commit_2xx_success）。
    scope.succeeded(attempts, status.as_u16()).await;

    // token 记账；est_cost 留 0，由 upsert_log 按 actual_model 的 registry 价 × tokens 计算
    // （全 0 价未标 free → 兜底价；free:true → 0，见 log.rs est_cost 回落）。
    let (input, output) = (spec.extract_usage)(&body_str);
    log.input_tokens = input;
    log.output_tokens = output;
    // 费用状态（票 07）：生图 2xx 按响应用量/张数定已知或估算。
    if matches!(spec.kind, RequestKind::Image) {
        log.fee_state = Some(image_fee_state(input, output, &body_str).to_string());
    }
    // 计量次数（rerank：search_units；speech：input 字符数；ASR：duration 秒数；embedding None → 0）。
    let request_units = match spec.extract_units {
        Some(units) => {
            let n = units(req.value, &body_str);
            // 计数经内存 ProxyLog 透传，calc_est_cost 按条目 `price.unit` 消费
            // （request/image/mp 按次；char/second 按字符数/时长换算，issue 157 / #167）。
            log.tool_call_counts
                .push((crate::gateway::billing::SEARCH_UNITS_KEY.to_string(), n));
            n
        }
        None => 0,
    };

    // issue #61：embedding / rerank 请求入余额与手动预算预估（此前完全不触发）。
    // spawn_estimate 后台执行，不阻塞响应；base_url / api_key 仅供校准真查用。
    spawn_estimate(
        scope.state,
        route.platform.id,
        &route.platform.platform_type,
        route.platform.base_url.clone(),
        route.platform.api_key.clone(),
        actual_model.clone(),
        route.platform.extra.clone(),
        input,
        output,
        0, // embedding / rerank 无缓存 token
        0,
        0,
        route.platform.endpoints.iter().any(|ep| ep.coding_plan),
        log.created_at,
        0.0, // tools 按次费与 aux 请求无关
        request_units,
        true, // 额度窗口执行范围：向量化 / 重排序计入
        tracing::Span::current(),
    );

    log.status_code = status.as_u16() as i32;
    log.done = true;
    // 二进制音频（speech）不落原文，落字节数占位；JSON 落原文。
    let raw_body_log = if spec.binary_response {
        format!("[binary {} bytes]", body.len())
    } else {
        body_str
    };
    log.response_body = raw_body_log.clone();
    log.user_response_body = raw_body_log;
    log.user_response_headers = format!(r#"{{"content-type":"{resp_ct}"}}"#);
    log.duration_ms = scope.start.elapsed().as_millis() as i32;
    log.retry_count = aidog_db::models::retry_count_of(attempts);
    log.attempts = std::mem::take(attempts);
    upsert_log(scope.state, log, log_settings).await;
    let mut response = (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, resp_ct)],
        client_body,
    )
        .into_response();
    inject_trace_header(&mut response);
    AttemptOutcome::Respond(response)
}

// ══ 票 05/06：image 确定性终态与流式转发 ══

/// image 链路的确定性终态（票 05 上游不回 SSE → 501；票 06 不可等价参数 → 400、
/// 响应不可转换 → 502、gemini 方言流式 → 501）：落库 + 直返文本错误体。
async fn image_terminal(
    scope: &super::call_scope::CallScope<'_>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    route: &RouteResult,
    status: StatusCode,
    msg: String,
) -> AttemptOutcome {
    log.platform_id = route.platform.id;
    log.status_code = status.as_u16() as i32;
    log.done = true;
    log.response_body = msg.clone();
    log.user_response_body = msg.clone();
    log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
    log.duration_ms = scope.start.elapsed().as_millis() as i32;
    log.retry_count = aidog_db::models::retry_count_of(attempts);
    log.attempts = std::mem::take(attempts);
    upsert_log(scope.state, log, scope.log_settings).await;
    let mut r = (status, msg).into_response();
    inject_trace_header(&mut r);
    AttemptOutcome::Respond(r)
}

// ══ 票 05：image 流式转发 ══

/// SSE 流内记账累计器：事件文本（≤16MB 尾窗，对齐 STREAM_BODY_MAX_BYTES）+ completed usage。
#[derive(Clone, Default)]
struct ImageStreamAcc {
    text: String,
    scanned: usize,
    input: i32,
    output: i32,
    /// 收到的 `image_generation.partial_image` 事件数（票 07 附加费计量）。
    partials: i32,
    completed: bool,
}

impl ImageStreamAcc {
    const TEXT_CAP: usize = 16 * 1024 * 1024;

    fn feed(&mut self, chunk: &[u8]) {
        self.text.push_str(&String::from_utf8_lossy(chunk));
        // 尾窗截断：丢弃头部一半，重扫（absorb 幂等，不会双计 usage）。
        if self.text.len() > Self::TEXT_CAP {
            let keep = self.text.split_off(self.text.len() / 2);
            self.text = keep;
            self.scanned = 0;
        }
        while let Some(idx) = self.text[self.scanned..].find('\n') {
            let end = self.scanned + idx + 1;
            let line = self.text[self.scanned..end].to_string();
            self.scanned = end;
            self.absorb_line(&line);
        }
    }

    fn absorb_line(&mut self, line: &str) {
        let Some(data) = line.strip_prefix("data:") else {
            return;
        };
        let Ok(v) = serde_json::from_str::<Value>(data.trim()) else {
            return;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("image_generation.partial_image") => {
                self.partials += 1;
                return;
            }
            Some("image_generation.completed") => {}
            _ => return,
        }
        let tok = |field: &str, legacy: &str| {
            v.pointer(&format!("/usage/{field}"))
                .or_else(|| v.pointer(&format!("/usage/{legacy}")))
                .and_then(Value::as_i64)
                .unwrap_or(0) as i32
        };
        self.input = tok("input_tokens", "prompt_tokens");
        self.output = tok("output_tokens", "completion_tokens");
        self.completed = true;
    }
}

/// 流式终态落库 guard：随响应 stream Drop（流耗尽 / 客户端断连）触发，spawn 异步收尾
///——mitm_bypass MitmStreamGuard 同 idiom（Drop 不能 await）。
struct ImageStreamGuard {
    state: Arc<ProxyState>,
    log: ProxyLog,
    log_settings: ProxyLogSettings,
    acc: Arc<std::sync::Mutex<ImageStreamAcc>>,
    start: std::time::Instant,
    actual_model: String,
    platform_id: u64,
    platform_type: Protocol,
    quota_base_url: String,
    quota_api_key: String,
    quota_extra: String,
    coding_plan: bool,
}

impl Drop for ImageStreamGuard {
    fn drop(&mut self) {
        let state = self.state.clone();
        let mut log = std::mem::take(&mut self.log);
        let acc = self.acc.lock().map(|a| a.clone()).unwrap_or_default();
        log.platform_id = self.platform_id;
        log.is_stream = true;
        log.status_code = 200;
        log.input_tokens = acc.input;
        // 官方附加费（guides/image-generation "Partial images cost"，2026-11-24 取证）：
        // 流式每张 partial 额外收 100 图像输出 token，completed usage 不含该笔。
        let output_with_surcharge = acc.output + 100 * acc.partials;
        log.output_tokens = output_with_surcharge;
        // 费用状态（票 03 口径）：completed → 已知；中断有已计费用（含 partial 附加费）→
        // 不完整；全无 → 未知。
        log.fee_state = Some(
            stream_fee_state(acc.completed, acc.input, output_with_surcharge).to_string(),
        );
        // 中断且无 completed usage → 0 units，避免把请求 n 当成已完成图数；费用未知按票 03 口径。
        let units = i64::from(acc.completed);
        log.tool_call_counts
            .push((crate::gateway::billing::SEARCH_UNITS_KEY.to_string(), units));
        log.response_body = acc.text.clone();
        log.user_response_body = acc.text;
        log.done = true;
        log.duration_ms = self.start.elapsed().as_millis() as i32;
        log.retry_count = aidog_db::models::retry_count_of(&log.attempts);
        if !acc.completed {
            // 流式中断（票 03 口径）：按已收用量记账（partial 事件不带 usage → 0 = 未知），
            // 标记落 attempts（元数据，不受原始信息开关控）；可见费用标记由票 07 接。
            log.attempts.push(ProxyAttempt {
                kind: Some("stream_interrupted".into()),
                error_class: None,
                platform_id: self.platform_id,
                status_code: 200,
                error: "image stream interrupted before completed event".into(),
                duration_ms: log.duration_ms as i64,
                ts: aidog_db::now(),
                model: Some(self.actual_model.clone()),
                ..Default::default()
            });
        }
        let input = log.input_tokens;
        let output = log.output_tokens;
        spawn_estimate(
            &state,
            self.platform_id,
            &self.platform_type,
            self.quota_base_url.clone(),
            self.quota_api_key.clone(),
            self.actual_model.clone(),
            self.quota_extra.clone(),
            input,
            output,
            0,
            0,
            0,
            self.coding_plan,
            log.created_at,
            0.0,
            units,
            true,
            tracing::Span::current(),
        );
        // 终态落库走同步入队（queue_upsert_log 非阻塞；est_cost 在队列写库时按 registry 价计算）。
        let settings = self.log_settings.clone();
        super::log::queue_upsert_log(&state, &log, &settings);
    }
}

/// 流式 2xx：SSE 事件原文透传（官方事件形态，不改写），流内累计 completed usage，
/// 终态由 [`ImageStreamGuard`] 落库。
async fn image_stream_respond(
    scope: &super::call_scope::CallScope<'_>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    resp: reqwest::Response,
    resp_ct: String,
) -> AttemptOutcome {
    // 成功记账在响应头到达（对齐聊天 peek 后 commit）；usage 计费在流终态 guard 补写。
    scope.succeeded(attempts, 200).await;
    log.attempts = std::mem::take(attempts);
    log.user_response_headers = format!(r#"{{"content-type":"{resp_ct}"}}"#);

    let acc = Arc::new(std::sync::Mutex::new(ImageStreamAcc::default()));
    let acc_stream = acc.clone();
    let stream = resp.bytes_stream().map(move |chunk| {
        let chunk = chunk.unwrap_or_default();
        if let Ok(mut a) = acc_stream.lock() {
            a.feed(&chunk);
        }
        Ok::<Bytes, std::io::Error>(chunk)
    });
    let guard = ImageStreamGuard {
        state: scope.state.clone(),
        log: std::mem::take(log),
        log_settings: scope.log_settings.clone(),
        acc,
        start: scope.start,
        actual_model: scope.route.target_model.clone(),
        platform_id: scope.route.platform.id,
        platform_type: scope.route.platform.platform_type.clone(),
        quota_base_url: scope.route.platform.base_url.clone(),
        quota_api_key: scope.route.platform.api_key.clone(),
        quota_extra: scope.route.platform.extra.clone(),
        coding_plan: scope.route.platform.endpoints.iter().any(|ep| ep.coding_plan),
    };
    // guard 随 stream Drop 触发终态落库（客户端断连 / 流耗尽都覆盖）。
    let stream = stream.chain(futures::stream::poll_fn(move |_| {
        let _ = &guard;
        std::task::Poll::Ready(None)
    }));
    let mut response = (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, resp_ct)],
        Body::from_stream(stream),
    )
        .into_response();
    inject_trace_header(&mut response);
    AttemptOutcome::Respond(response)
}

#[cfg(test)]
#[path = "test_aux_model.rs"]
mod test_aux_model;
