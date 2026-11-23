//! 票 06 单元测试：OpenAI Images ↔ Gemini Interactions 转换矩阵（纯函数，无外呼）。

use super::*;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

// ── generation：可等价转换 ──

/// prompt→input 文本块、1024x1024→image_size 1K + aspect_ratio 1:1、png→mime_type、
/// response_format(b64_json)/partial_images(0) 等价忽略、n=1 等价省略。
#[test]
fn generation_equivalent_params_convert() {
    let req = serde_json::json!({
        "model": "client-img",
        "prompt": "a cat",
        "n": 1,
        "size": "1024x1024",
        "output_format": "png",
        "response_format": "b64_json",
        "partial_images": 0,
    });
    let out = generation_to_gemini(&req, "gemini-img").unwrap();
    assert_eq!(out["model"], "gemini-img");
    assert_eq!(out["input"][0]["type"], "text");
    assert_eq!(out["input"][0]["text"], "a cat");
    assert_eq!(out["response_format"]["type"], "image");
    assert_eq!(out["response_format"]["image_size"], "1K");
    assert_eq!(out["response_format"]["aspect_ratio"], "1:1");
    assert_eq!(out["response_format"]["mime_type"], "image/png");
    assert!(out.get("n").is_none(), "n=1 等价省略");
    assert!(out.get("partial_images").is_none());
    // OpenAI response_format 是 legacy 参数（GPT image 恒返 base64）→ 不进上游体
    assert!(out.as_object().unwrap().len() == 3, "仅 model/input/response_format");
}

/// jpeg / webp / size 缺省（模型默认）均为等价路径。
#[test]
fn generation_default_and_mime_variants_convert() {
    let req = serde_json::json!({"prompt": "x", "output_format": "jpeg"});
    let out = generation_to_gemini(&req, "m").unwrap();
    assert_eq!(out["response_format"]["mime_type"], "image/jpeg");
    let req2 = serde_json::json!({"prompt": "x", "size": "auto"});
    let out2 = generation_to_gemini(&req2, "m").unwrap();
    assert!(
        !out2.as_object().unwrap().contains_key("response_format"),
        "size auto = 模型默认，省略 response_format"
    );
}

// ── generation：不可等价参数明确报错 ──

/// 官方两侧词表无对应的参数逐一断言报错并点名（票 00：不静默丢）。
#[test]
fn non_equivalent_params_error_named() {
    let cases: Vec<(Value, String)> = vec![
        (serde_json::json!({"prompt":"x","quality":"high"}), "quality".into()),
        (serde_json::json!({"prompt":"x","background":"transparent"}), "background".into()),
        (serde_json::json!({"prompt":"x","moderation":"low"}), "moderation".into()),
        (serde_json::json!({"prompt":"x","output_compression":50}), "output_compression".into()),
        (serde_json::json!({"prompt":"x","frame_count":9}), "frame_count".into()),
        (serde_json::json!({"prompt":"x","frame_duration_ms":200}), "frame_duration_ms".into()),
        (serde_json::json!({"prompt":"x","n":3}), "n=3".into()),
        (serde_json::json!({"prompt":"x","size":"1536x1024"}), "size=1536x1024".into()),
        (serde_json::json!({"prompt":"x","output_format":"gif"}), "output_format=gif".into()),
        (serde_json::json!({"prompt":"x","output_format":"svg"}), "output_format=svg".into()),
        (serde_json::json!({"prompt":"x","whatever":1}), "whatever".into()),
    ];
    for (req, needle) in cases {
        let err = generation_to_gemini(&req, "m").unwrap_err();
        assert!(err.contains("Gemini Interactions"), "{needle}: {err}");
        assert!(err.contains(&needle), "{needle}: {err}");
    }
}

/// prompt 必填（OpenAI Images 契约）：缺失 / 空 → 报错点名 prompt。
#[test]
fn prompt_required_error() {
    let err = generation_to_gemini(&serde_json::json!({"model":"m"}), "m").unwrap_err();
    assert!(err.contains("prompt"), "{err}");
    let err = generation_to_gemini(&serde_json::json!({"prompt":""}), "m").unwrap_err();
    assert!(err.contains("prompt"), "{err}");
}

// ── edits：multipart → Interactions input 块 ──

fn build_edit_form(parts: &[(&str, &str, &[u8])], boundary: &str) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, extra, data) in parts {
        let disp = if extra.is_empty() {
            format!("Content-Disposition: form-data; name=\"{name}\"")
        } else {
            format!("Content-Disposition: form-data; name=\"{name}\"; {extra}")
        };
        body.extend_from_slice(format!("--{boundary}\r\n{disp}\r\n\r\n").as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// prompt→文本块、image[]→image 块（mime 取 part Content-Type、data base64）。
#[test]
fn edit_form_converts_to_input_blocks() {
    let b = "testboundary1";
    let bytes = build_edit_form(
        &[
            ("model", "", b"client-img"),
            ("prompt", "", b"a cat on a mat"),
            ("image[]", "filename=\"a.png\"\r\nContent-Type: image/png", b"PNGDATA-AAAA"),
            ("image[]", "filename=\"b.png\"\r\nContent-Type: image/png", b"PNGDATA-BBBB"),
        ],
        b,
    );
    let out = edit_to_gemini(&bytes, b, "gemini-img").unwrap();
    assert_eq!(out["model"], "gemini-img");
    let input = out["input"].as_array().unwrap();
    assert_eq!(input.len(), 3);
    assert_eq!(input[0]["type"], "text");
    assert_eq!(input[0]["text"], "a cat on a mat");
    assert_eq!(input[1]["type"], "image");
    assert_eq!(input[1]["mime_type"], "image/png");
    assert_eq!(
        B64.decode(input[1]["data"].as_str().unwrap()).unwrap(),
        b"PNGDATA-AAAA"
    );
    assert_eq!(
        B64.decode(input[2]["data"].as_str().unwrap()).unwrap(),
        b"PNGDATA-BBBB"
    );
}

/// 无 Content-Type 头时按文件名扩展映射 mime；jpg → image/jpeg。
#[test]
fn edit_mime_falls_back_to_filename_ext() {
    let b = "testboundary2";
    let bytes = build_edit_form(
        &[
            ("prompt", "", b"x"),
            ("image", "filename=\"c.jpg\"", b"JPGDATA"),
        ],
        b,
    );
    let out = edit_to_gemini(&bytes, b, "m").unwrap();
    assert_eq!(out["input"][1]["mime_type"], "image/jpeg");
}

/// mask / markup（Gemini 无对应）与未知 part 明确报错。
#[test]
fn edit_non_equivalent_parts_error() {
    let b = "testboundary3";
    let mask_form = build_edit_form(
        &[("prompt", "", b"x"), ("mask", "filename=\"m.png\"", b"MASKDATA")],
        b,
    );
    let err = edit_to_gemini(&mask_form, b, "m").unwrap_err();
    assert!(err.contains("mask"), "{err}");

    let unknown_form = build_edit_form(
        &[("prompt", "", b"x"), ("quality", "", b"high")],
        b,
    );
    let err = edit_to_gemini(&unknown_form, b, "m").unwrap_err();
    assert!(err.contains("quality"), "{err}");

    let n_form = build_edit_form(&[("prompt", "", b"x"), ("n", "", b"4")], b);
    let err = edit_to_gemini(&n_form, b, "m").unwrap_err();
    assert!(err.contains("n=4"), "{err}");

    let fmt_form = build_edit_form(&[("prompt", "", b"x"), ("output_format", "", b"gif")], b);
    let err = edit_to_gemini(&fmt_form, b, "m").unwrap_err();
    assert!(err.contains("output_format=gif"), "{err}");
}

/// 参考图 > 14 张（Gemini 上限，票 01 报告）→ 报错；14 张内正常。
#[test]
fn edit_reference_image_cap_boundary() {
    let b = "testboundary4";
    let mut parts: Vec<(String, String, Vec<u8>)> = vec![("prompt".into(), String::new(), b"x".to_vec())];
    for i in 0..14 {
        parts.push(("image[]".to_string(), format!("filename=\"p{i}.png\"\r\nContent-Type: image/png"), b"D".to_vec()));
    }
    let refs: Vec<(&str, &str, &[u8])> = parts.iter().map(|(n, e, d)| (n.as_str(), e.as_str(), d.as_slice())).collect();
    let bytes = build_edit_form(&refs, b);
    assert!(edit_to_gemini(&bytes, b, "m").is_ok(), "14 张在上限内");

    parts.push(("image[]".into(), "filename=\"x.png\"\r\nContent-Type: image/png".into(), b"D".to_vec()));
    let refs15: Vec<(&str, &str, &[u8])> = parts.iter().map(|(n, e, d)| (n.as_str(), e.as_str(), d.as_slice())).collect();
    let bytes15 = build_edit_form(&refs15, b);
    let err = edit_to_gemini(&bytes15, b, "m").unwrap_err();
    assert!(err.contains("14"), "{err}");
}

// ── 响应：interaction → ImagesResponse ──

const INTERACTION: &str = r#"{
  "id": "abc", "object": "interaction", "model": "gemini-img", "status": "completed",
  "steps": [{"type": "model_output", "content": [
    {"type": "text", "text": "here you go"},
    {"type": "image", "mime_type": "image/png", "data": "aGk="},
    {"type": "image", "mime_type": "image/png", "data": "aGk="}
  ]}],
  "usage": {"total_input_tokens": 10, "total_output_tokens": 50, "total_tokens": 60}
}"#;

/// steps[].content[] 的 image 块 → data[].b64_json；usage.total_* → usage.input/output_tokens。
#[test]
fn interaction_converts_to_images_response() {
    let v: Value = serde_json::from_str(INTERACTION).unwrap();
    let out = interaction_to_openai(&v, "img-slot").unwrap();
    assert_eq!(out["model"], "img-slot");
    assert_eq!(out["data"].as_array().map(Vec::len), Some(2));
    assert_eq!(out["data"][0]["b64_json"], "aGk=");
    assert_eq!(out["usage"]["input_tokens"], 10);
    assert_eq!(out["usage"]["output_tokens"], 50);
    let created = out["created"].as_i64().unwrap();
    let now_s = aidog_db::now() / 1000;
    assert!((created - now_s).abs() < 60, "created 为 Unix 秒");
}

/// 0 张图 / steps 缺失 → Err（上游契约不符，按服务类失败换平台）。
#[test]
fn interaction_without_image_errors() {
    let v: Value = serde_json::from_str(r#"{"steps":[{"type":"model_output","content":[{"type":"text","text":"hi"}]}]}"#).unwrap();
    assert!(interaction_to_openai(&v, "m").unwrap_err().contains("no image"));
    let v: Value = serde_json::from_str(r#"{"usage":{}}"#).unwrap();
    assert!(interaction_to_openai(&v, "m").is_err());
}
