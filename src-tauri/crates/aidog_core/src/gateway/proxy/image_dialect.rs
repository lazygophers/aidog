//! Gemini 原生生图方言（Interactions API）跨协议转换（生图-模型类型 票 06）。
//!
//! 平台方言 = `platform.extra.image_dialect == "gemini"`（独立配置，不从聊天协议推断）。
//! 网关入口恒为 OpenAI Images 形状（`/v1/images/generations`、`/v1/images/edits`）；
//! gemini 方言平台出站前做 OpenAI→Interactions 转换，响应做 interaction→ImagesResponse 转换。
//! 只转换可等价表达的内容；无法等价表达的参数明确报错（票 00 拍板：不静默丢弃、不降级）。
//!
//! 参数矩阵出处（第 1 类官方原文，票 01 取证缓存 `.tmp/生图-协议核验/`）：
//! - 请求 / 响应形状：ai.google.dev/api-reference/interactions-api（`Interaction.steps[].content[]`、
//!   `ImageContent {type,data,mime_type}`、`usage.total_input_tokens` / `usage.total_output_tokens`、
//!   `ImageOutput {type,aspect_ratio,image_size,mime_type}`、image_size 词表 512/1K/2K/4K）
//! - 请求示例：ai.google.dev/gemini-api/docs/image-generation（REST：`input[]` 文本 + 参考图块、
//!   `response_format {type,mime_type,aspect_ratio,image_size}`、头 `Api-Revision: 2026-05-20`）
//! - 等价映射依据：1K = 1024²（docs/pricing NB2.1 脚注）；mime png/webp/jpeg 两协议同词表；
//!   Interactions 请求无 `n` 参数（API reference 全文无该字段）→ n>1 无等价表达。

use super::*;
use super::aux_model::find_sub;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// 出站路径（拼在平台 base_url 后；Gemini base_url 已含版本前缀 `/v1beta`）。
pub(crate) const GEMINI_API_PATH: &str = "/interactions";
/// 官方 REST 示例固定携带的修订头（image-generation 页 REST 示例原文）。
pub(crate) const GEMINI_API_REVISION: &str = "2026-05-20";

/// 参考图上限：Gemini 最多 14 张（NB2.1 / 3.1 Flash：≤10 高保真物体 + ≤4 角色；
/// 官方 docs/image-generation，票 01 报告）。OpenAI edits 允许 16 张，超限不可等价。
const GEMINI_MAX_REFERENCE_IMAGES: usize = 14;

/// 不可等价参数的统一报错文案（点名参数清单，票 00：明确报错不静默丢）。
fn unsupported_err(items: &[String]) -> String {
    format!(
        "parameters not expressible in Gemini Interactions API (explicit error, not silently dropped): {}",
        items.join(", ")
    )
}

/// OpenAI Images 请求 size 值 → response_format 分辨率 / 宽高比；None = 无需携带
///（缺省 / auto = 模型默认）。仅 `1024x1024` ↔ `1K` + `1:1` 为逐像素等价
///（官方 pricing 页 NB2.1 脚注 1K = 1024²）；其余尺寸在桶式分辨率词表（512/1K/2K/4K ×
/// 14 宽高比）中无逐像素等价 → 报错。
fn size_to_format(size: Option<&str>) -> Result<Option<Vec<(String, Value)>>, String> {
    Ok(match size {
        None | Some("auto") => None,
        Some("1024x1024") => Some(vec![
            ("image_size".to_string(), Value::from("1K")),
            ("aspect_ratio".to_string(), Value::from("1:1")),
        ]),
        Some(s) => return Err(format!("size={s}")),
    })
}

/// OpenAI `output_format` → Gemini `mime_type`：png/webp/jpeg 同词表等价；
/// gif/svg（OpenAI 专有动画 / 矢量输出）无对应 → 报错。
fn output_format_to_mime(fmt: Option<&str>) -> Result<Option<(String, Value)>, String> {
    Ok(match fmt {
        None => None,
        Some("png") => Some(("mime_type".to_string(), Value::from("image/png"))),
        Some("jpeg") => Some(("mime_type".to_string(), Value::from("image/jpeg"))),
        Some("webp") => Some(("mime_type".to_string(), Value::from("image/webp"))),
        Some(f) => return Err(format!("output_format={f}")),
    })
}

/// 组装 response_format（ImageOutput 形状：`{type:"image", image_size, aspect_ratio, mime_type}`）。
fn build_response_format(
    size: Option<&str>,
    output_format: Option<&str>,
    unsupported: &mut Vec<String>,
) -> Option<Value> {
    let mut fields: Vec<(String, Value)> = vec![("type".to_string(), Value::from("image"))];
    match size_to_format(size) {
        Ok(Some(kv)) => fields.extend(kv),
        Ok(None) => {}
        Err(e) => unsupported.push(e),
    }
    match output_format_to_mime(output_format) {
        Ok(Some(kv)) => fields.extend([kv]),
        Ok(None) => {}
        Err(e) => unsupported.push(e),
    }
    (fields.len() > 1).then(|| Value::Object(fields.into_iter().collect()))
}

/// `/v1/images/generations` JSON → Interactions 请求体。
/// 等价：prompt→input 文本块、model、n=1（缺省同义，省略）、size 1024x1024→1K+1:1、
/// output_format png/webp/jpeg→mime_type；OpenAI `response_format`（legacy，GPT image
/// 恒返 base64，SDK 文档串原文）与未流式的 `partial_images`（官方缺省 0 = 只收最终图）
/// 等价忽略；`stream` 由调用方先行判 501。其余参数（quality / background / moderation /
/// output_compression / frame_* / 未知键 / n>1 / 其余 size）无 Gemini 对应 → Err 点名。
pub(crate) fn generation_to_gemini(req: &Value, model: &str) -> Result<Value, String> {
    let Some(obj) = req.as_object() else {
        return Err("request must be a JSON object".into());
    };
    let mut unsupported: Vec<String> = Vec::new();
    for k in ["quality", "background", "moderation", "output_compression", "frame_count", "frame_duration_ms"] {
        if obj.contains_key(k) {
            unsupported.push(k.to_string());
        }
    }
    for k in obj.keys() {
        if !matches!(
            k.as_str(),
            "model" | "prompt" | "n" | "size" | "response_format"
                | "output_format" | "stream" | "partial_images"
        ) {
            unsupported.push(k.to_string());
        }
    }
    let prompt = obj.get("prompt").and_then(Value::as_str).unwrap_or("");
    if prompt.is_empty() {
        unsupported.push("prompt (required by OpenAI Images contract)".to_string());
    }
    let n = obj.get("n").and_then(Value::as_i64).unwrap_or(1);
    if n != 1 {
        unsupported.push(format!("n={n} (Interactions API has no batch parameter)"));
    }
    let size = obj.get("size").and_then(Value::as_str);
    let output_format = obj.get("output_format").and_then(Value::as_str);
    let response_format = build_response_format(size, output_format, &mut unsupported);
    if !unsupported.is_empty() {
        return Err(unsupported_err(&unsupported));
    }
    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), Value::from(model));
    body.insert(
        "input".to_string(),
        serde_json::json!([{ "type": "text", "text": prompt }]),
    );
    if let Some(rf) = response_format {
        body.insert("response_format".to_string(), rf);
    }
    Ok(Value::Object(body))
}

/// 一个 multipart part 的解析结果（头段文本 + 值段字节）。
struct FormPart<'a> {
    headers: &'a str,
    value: &'a [u8],
}

/// 遍历 multipart part（与 aux_model::validate_image_edit_form 同边界判定）。
/// RFC 2045 首边界行在 body 开头、无前置 CRLF → 单独收首段（validate 只从首个
/// CRLF+boundary 起扫，首段被跳过——对它只校验 image/mask 无影响，这里 prompt 在首段必须收到）。
fn multipart_parts<'a>(bytes: &'a [u8], boundary: &str) -> Vec<FormPart<'a>> {
    let delim = format!("\r\n--{boundary}");
    let mut out = Vec::new();
    let mut cursor = 0usize;
    if bytes.starts_with(&delim.as_bytes()[2..]) {
        let hdr_off = delim.len() - 2;
        let Some(hdr_end) = find_sub(&bytes[hdr_off..], b"\r\n\r\n") else {
            return out;
        };
        let headers = std::str::from_utf8(&bytes[hdr_off..hdr_off + hdr_end]).unwrap_or("");
        let value_start = hdr_off + hdr_end + 4;
        let value_end = find_sub(&bytes[value_start..], delim.as_bytes())
            .map(|e| value_start + e)
            .unwrap_or(bytes.len());
        out.push(FormPart {
            headers,
            value: &bytes[value_start..value_end],
        });
        cursor = value_end;
    }
    while let Some(off) = find_sub(&bytes[cursor..], delim.as_bytes()) {
        let start = cursor + off + delim.len();
        let Some(hdr_end) = find_sub(&bytes[start..], b"\r\n\r\n") else {
            break;
        };
        let headers = std::str::from_utf8(&bytes[start..start + hdr_end]).unwrap_or("");
        let value_start = start + hdr_end + 4;
        let value_end = find_sub(&bytes[value_start..], delim.as_bytes())
            .map(|e| value_start + e)
            .unwrap_or(bytes.len());
        out.push(FormPart {
            headers,
            value: &bytes[value_start..value_end],
        });
        cursor = value_end;
        if cursor >= bytes.len() {
            break;
        }
    }
    out
}

/// part 头里的 Content-Disposition 属性值（name= / filename=）。
fn disposition_attr<'a>(headers: &'a str, attr: &str) -> Option<&'a str> {
    let marker = format!("{attr}=\"");
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(p) = lower.find(&marker) {
            let start = p + marker.len();
            let rest = &line[start..];
            if let Some(e) = rest.find('"') {
                return Some(&line[start..start + e]);
            }
        }
    }
    None
}

/// part 显式 Content-Type 头（如 `image/png`）。
fn part_content_type(headers: &str) -> Option<&str> {
    headers.lines().find_map(|l| {
        let lower = l.to_ascii_lowercase();
        lower
            .starts_with("content-type:")
            .then(|| l[lower.len()..].trim())
    })
}

/// 文件名扩展 → mime（Gemini ImageContent 词表内的三种编辑上传格式）。
fn mime_from_filename(filename: Option<&str>) -> Option<String> {
    let ext = filename?.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png".into()),
        "jpg" | "jpeg" => Some("image/jpeg".into()),
        "webp" => Some("image/webp".into()),
        _ => None,
    }
}

/// multipart 编辑表单 → Interactions 请求体：prompt→文本块、image/image[]→image 块
///（mime 取 part Content-Type，缺省按文件名扩展；data base64）。mask / markup /
/// 未知 part / n>1 / 其余 size / gif·svg output_format / 参考图 >14 张无等价表达 → Err 点名。
pub(crate) fn edit_to_gemini(bytes: &[u8], boundary: &str, model: &str) -> Result<Value, String> {
    let mut unsupported: Vec<String> = Vec::new();
    let mut prompt: Option<String> = None;
    let mut n: i64 = 1;
    let mut size: Option<&str> = None;
    let mut output_format: Option<&str> = None;
    let mut images: Vec<(String, &[u8])> = Vec::new();
    for part in multipart_parts(bytes, boundary) {
        let Some(name) = disposition_attr(part.headers, "name") else {
            continue;
        };
        match name {
            "prompt" => prompt = Some(String::from_utf8_lossy(part.value).trim().to_string()),
            "model" | "response_format" | "stream" | "partial_images" => {
                // model 由路由层改写；response_format legacy 等价忽略；stream 调用方先行判 501。
            }
            "image" | "image[]" => {
                let filename = disposition_attr(part.headers, "filename");
                let mime = part_content_type(part.headers)
                    .filter(|c| c.to_ascii_lowercase().starts_with("image/"))
                    .map(|c| c.to_string())
                    .or_else(|| mime_from_filename(filename));
                match mime {
                    Some(m) => images.push((m, part.value)),
                    None => unsupported.push(format!(
                        "image mime for {filename:?} (png/webp/jpg only in OpenAI edits contract)"
                    )),
                }
            }
            "n" => {
                n = String::from_utf8_lossy(part.value)
                    .trim()
                    .parse()
                    .unwrap_or(1);
            }
            "size" => {
                size = Some(std::str::from_utf8(part.value).unwrap_or_default().trim());
            }
            "output_format" => {
                output_format =
                    Some(std::str::from_utf8(part.value).unwrap_or_default().trim());
            }
            "mask" | "markup" => unsupported.push(name.to_string()),
            other => unsupported.push(other.to_string()),
        }
    }
    if n != 1 {
        unsupported.push(format!("n={n} (Interactions API has no batch parameter)"));
    }
    if images.len() > GEMINI_MAX_REFERENCE_IMAGES {
        unsupported.push(format!(
            "{} reference images (Gemini supports up to {GEMINI_MAX_REFERENCE_IMAGES})",
            images.len()
        ));
    }
    let prompt_text = prompt.unwrap_or_default();
    if prompt_text.is_empty() {
        unsupported.push("prompt (required by OpenAI Images contract)".to_string());
    }
    let response_format = build_response_format(size, output_format, &mut unsupported);
    if !unsupported.is_empty() {
        return Err(unsupported_err(&unsupported));
    }
    let mut input = vec![serde_json::json!({ "type": "text", "text": prompt_text })];
    for (mime, data) in &images {
        input.push(serde_json::json!({
            "type": "image",
            "mime_type": mime,
            "data": B64.encode(data),
        }));
    }
    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), Value::from(model));
    body.insert("input".to_string(), Value::from(input));
    if let Some(rf) = response_format {
        body.insert("response_format".to_string(), rf);
    }
    Ok(Value::Object(body))
}

/// interaction 响应 → OpenAI ImagesResponse：遍历 `steps[].content[]` 的 image 块
/// → `data[].b64_json`；`usage.total_input_tokens` / `usage.total_output_tokens`
/// → `usage.input_tokens` / `usage.output_tokens`。0 张图 / steps 缺失 → Err
///（上游契约不符，调用方按服务类失败换平台）。
pub(crate) fn interaction_to_openai(body: &Value, model: &str) -> Result<Value, String> {
    let mut data = Vec::new();
    for step in body
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        for block in step.get("content").and_then(Value::as_array).into_iter().flatten() {
            if block.get("type").and_then(Value::as_str) == Some("image")
                && let Some(d) = block.get("data").and_then(Value::as_str)
            {
                data.push(serde_json::json!({ "b64_json": d }));
            }
        }
    }
    if data.is_empty() {
        return Err("interaction response contains no image block".into());
    }
    let usage = body.get("usage");
    let tok = |k: &str| {
        usage
            .and_then(|u| u.get(k))
            .and_then(Value::as_i64)
            .unwrap_or(0)
    };
    Ok(serde_json::json!({
        "created": aidog_db::now() / 1000,
        "model": model,
        "data": data,
        "usage": {
            "input_tokens": tok("total_input_tokens"),
            "output_tokens": tok("total_output_tokens"),
        }
    }))
}

#[cfg(test)]
#[path = "test_image_dialect.rs"]
mod test_image_dialect;
