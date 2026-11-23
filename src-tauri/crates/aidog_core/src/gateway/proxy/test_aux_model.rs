//! 辅助模型请求单测：入站识别、endpoint 选择、usage 提取。端到端见 test_integration.rs。

use super::*;

// audio spec 的 multipart / binary_response 是编译期常量，const 块断言（运行期断言被 clippy 拒）。
const _: () = {
    assert!(AUDIO_SPEECH.binary_response && !AUDIO_SPEECH.multipart);
    assert!(!AUDIO_TRANSCRIPTIONS.binary_response && AUDIO_TRANSCRIPTIONS.multipart);
    assert!(!AUDIO_TRANSLATIONS.binary_response && AUDIO_TRANSLATIONS.multipart);
};

#[test]
fn embedding_endpoint_suffix_match() {
    for p in ["/v1/embeddings", "/embeddings", "/proxy/v1/embeddings", "/v1/embeddings/"] {
        assert!(is_embedding_endpoint(p), "{p}");
    }
    for p in ["/v1/embeddingsfoo", "/v1/embeddings/x", "/v1/chat/completions", "/v1/myembeddings"] {
        assert!(!is_embedding_endpoint(p), "{p}");
    }
}

#[test]
fn embedding_usage_only_input() {
    let (i, o) = (EMBEDDING.extract_usage)(r#"{"usage":{"prompt_tokens":8,"total_tokens":8}}"#);
    assert_eq!((i, o), (8, 0));
    assert_eq!((EMBEDDING.extract_usage)("not json"), (0, 0));
    assert_eq!((EMBEDDING.extract_usage)(r#"{"data":[]}"#), (0, 0));
}

#[test]
fn openai_family_endpoint_priority() {
    let ep = |protocol, url: &str| super::models::PlatformEndpoint {
        protocol,
        base_url: url.into(),
        client_type: "default".into(),
        coding_plan: false,
    };
    let eps = vec![
        ep(Protocol::Anthropic, "a"),
        ep(Protocol::OpenAIResponses, "r"),
        ep(Protocol::OpenAI, "o"),
    ];
    assert_eq!(
        super::endpoint::select_endpoint_for_openai_family(&eps).unwrap().base_url,
        "o"
    );
    assert!(super::endpoint::select_endpoint_for_openai_family(&eps[..1]).is_none());
}

#[test]
fn rerank_endpoint_suffix_match() {
    for p in ["/v1/rerank", "/v2/rerank", "/rerank", "/proxy/v1/rerank", "/v1/rerank/"] {
        assert!(is_rerank_endpoint(p), "{p}");
    }
    for p in ["/v1/rerankx", "/v1/rerank/x", "/v1/embeddings", "/v1/myrerank"] {
        assert!(!is_rerank_endpoint(p), "{p}");
    }
    assert!(!is_embedding_endpoint("/v1/rerank"));
}

/// 样例取自一手文档（spec Further Notes）：Jina / SiliconFlow / Cohere / 空。
#[test]
fn rerank_usage_three_shapes() {
    let u = RERANK.extract_usage;
    assert_eq!(u(r#"{"model":"jina-reranker-v2","usage":{"total_tokens":815},"results":[]}"#), (815, 0));
    assert_eq!(u(r#"{"id":"x","results":[],"meta":{"tokens":{"input_tokens":12,"output_tokens":3}}}"#), (12, 3));
    assert_eq!(u(r#"{"id":"x","results":[],"meta":{"billed_units":{"input_tokens":7,"output_tokens":2}}}"#), (7, 2));
    // Cohere 按次计量：token 记 0
    assert_eq!(u(r#"{"id":"x","results":[],"meta":{"billed_units":{"search_units":1}}}"#), (0, 0));
    assert_eq!(u(""), (0, 0));
    assert_eq!(u("not json"), (0, 0));
    assert_eq!(u(r#"{"results":[]}"#), (0, 0));
}

#[test]
fn rerank_search_units_defaults_to_one() {
    let f = RERANK.extract_units.expect("rerank has units");
    let req = Value::Null;
    assert_eq!(f(&req, r#"{"meta":{"billed_units":{"search_units":3}}}"#), 3);
    assert_eq!(f(&req, r#"{"usage":{"total_tokens":9}}"#), 1);
    assert_eq!(f(&req, "not json"), 1);
    assert!(EMBEDDING.extract_units.is_none());
}

/// audio spec 形状与计量提取器（issue #167）。
#[test]
fn audio_spec_shape_and_extractors() {
    for p in ["/v1/audio/speech", "/proxy/v1/audio/speech", "/v1/audio/speech/"] {
        assert!(audio_spec(p).is_some(), "{p}");
    }
    for p in ["/v1/audio/speechx", "/v1/audio", "/v1/chat/completions", "/v1/audio/transcribe"] {
        assert!(audio_spec(p).is_none(), "{p}");
    }
    assert_eq!(audio_spec("/v1/audio/speech").map(|s| s.label), Some("audio_speech"));
    assert_eq!(audio_spec("/v1/audio/transcriptions").map(|s| s.label), Some("audio_transcriptions"));
    assert_eq!(audio_spec("/v1/audio/translations").map(|s| s.label), Some("audio_translations"));
    assert_eq!((AUDIO_SPEECH.extract_usage)(""), (0, 0));

    let sf = AUDIO_SPEECH.extract_units.expect("speech has units");
    assert_eq!(sf(&serde_json::json!({"input":"你好世界hello"}), ""), 9);
    assert_eq!(sf(&serde_json::json!({"input":""}), ""), 0);
    assert_eq!(sf(&Value::Null, ""), 0);

    let af = AUDIO_TRANSCRIPTIONS.extract_units.expect("asr has units");
    let req = Value::Null;
    assert_eq!(af(&req, r#"{"text":"hi","duration":12.4}"#), 12);
    assert_eq!(af(&req, r#"{"text":"hi"}"#), 0);
    assert_eq!(af(&req, "not json"), 0);
}

/// image spec 形状与计量提取器（生图-模型类型 票 04）。
const _: () = {
    assert!(!IMAGE.multipart && !IMAGE.binary_response);
};

#[test]
fn image_spec_shape_and_extractors() {
    for p in [
        "/v1/images/generations",
        "/images/generations",
        "/proxy/v1/images/generations",
        "/v1/images/generations/",
    ] {
        assert!(is_image_endpoint(p), "{p}");
    }
    for p in [
        "/v1/images/edits",
        "/v1/images",
        "/v1/chat/completions",
        "/v1/images/generationsx",
    ] {
        assert!(!is_image_endpoint(p), "{p}");
    }
    assert_eq!(IMAGE.label, "images");
    assert_eq!(IMAGE.api_path, "/images/generations");

    let (i, o) = (IMAGE.extract_usage)(
        r#"{"usage":{"input_tokens":10,"output_tokens":50,"total_tokens":60}}"#,
    );
    assert_eq!((i, o), (10, 50));
    assert_eq!((IMAGE.extract_usage)(r#"{"usage":{"prompt_tokens":8}}"#), (8, 0));
    assert_eq!((IMAGE.extract_usage)("not json"), (0, 0));

    let uf = IMAGE.extract_units.expect("image has units");
    let req = serde_json::json!({"model":"m","prompt":"x","n":2});
    assert_eq!(
        uf(&req, r#"{"created":1,"data":[{"b64_json":"a"},{"b64_json":"b"}]}"#),
        2,
        "张数取响应 data 长度"
    );
    assert_eq!(uf(&req, r#"{"created":1}"#), 2, "响应无 data → 按请求 n 推算");
    let req_no_n = serde_json::json!({"model":"m","prompt":"x"});
    assert_eq!(
        uf(&req_no_n, r#"{"created":1}"#),
        1,
        "无 data 无 n → 官方缺省 1 张"
    );
}

// ══ 票 05：图片编辑与流式 ══

const _: () = {
    assert!(IMAGE_EDITS.multipart && !IMAGE_EDITS.binary_response);
};

#[test]
fn image_edit_spec_shape() {
    assert_eq!(IMAGE_EDITS.label, "image_edits");
    assert_eq!(IMAGE_EDITS.api_path, "/images/edits");
}

#[test]
fn image_edit_endpoint_suffix_match() {
    for p in ["/v1/images/edits", "/images/edits", "/proxy/v1/images/edits", "/v1/images/edits/"] {
        assert!(is_image_edit_endpoint(p), "{p}");
    }
    for p in [
        "/v1/images/editsx",
        "/v1/images/edits/x",
        "/v1/images/generations",
        "/v1/chat/completions",
    ] {
        assert!(!is_image_edit_endpoint(p), "{p}");
    }
    assert!(!is_image_endpoint("/v1/images/edits"), "edits 不命中 generations 判定");
}

/// 构造 multipart 表单：parts = (name, filename: Option<&str>, body)。
fn edit_form(parts: &[(&str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
    const B: &str = "aidogboundaries";
    let mut body = Vec::new();
    for (name, filename, data) in parts {
        body.extend_from_slice(format!("--{B}\r\n").as_bytes());
        match filename {
            Some(f) => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\n\r\n")
                    .as_bytes(),
            ),
            None => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            ),
        }
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{B}--\r\n").as_bytes());
    (
        format!("multipart/form-data; boundary={B}"),
        body,
    )
}

#[test]
fn image_edit_form_validation_official_limits() {
    let ok = edit_form(&[
        ("model", None, b"client-img"),
        ("prompt", None, b"a cat"),
        ("image[]", Some("a.png"), b"PNGDATA"),
        ("image[]", Some("b.webp"), b"WEBPDATA"),
        ("mask", Some("m.png"), b"MASK"),
    ]);
    assert_eq!(validate_image_edit_form(&ok.1, Some(ok.0.as_str())), None, "合法多参考图 + mask");

    // 17 张 → 超官方 16 张上限
    let mut parts: Vec<(&str, Option<&str>, &[u8])> = vec![("model", None, b"m"), ("prompt", None, b"p")];
    for _ in 0..17 {
        parts.push(("image[]", Some("a.png"), b"PNGDATA"));
    }
    let too_many = edit_form(&parts);
    assert!(
        validate_image_edit_form(&too_many.1, Some(too_many.0.as_str())).is_some(),
        "17 张须被拒"
    );

    // 单文件 ≥50MB → 超官方上限
    let big = edit_form(&[
        ("model", None, b"m"),
        ("prompt", None, b"p"),
        ("image", Some("a.png"), &vec![0u8; 50 * 1024 * 1024]),
    ]);
    assert!(validate_image_edit_form(&big.1, Some(big.0.as_str())).is_some(), "50MB 须被拒");

    // mask >4MB → 超官方上限；mask 非 png → 类型违规
    let big_mask = edit_form(&[
        ("model", None, b"m"),
        ("prompt", None, b"p"),
        ("image", Some("a.png"), b"PNG"),
        ("mask", Some("m.png"), &vec![0u8; 5 * 1024 * 1024]),
    ]);
    assert!(validate_image_edit_form(&big_mask.1, Some(big_mask.0.as_str())).is_some(), "5MB mask 须被拒");
    let bad_mask = edit_form(&[
        ("model", None, b"m"),
        ("prompt", None, b"p"),
        ("image", Some("a.png"), b"PNG"),
        ("mask", Some("m.jpg"), b"JPG"),
    ]);
    assert!(
        validate_image_edit_form(&bad_mask.1, Some(bad_mask.0.as_str())).is_some(),
        "mask 须为 png"
    );

    // image 非 png/webp/jpg → 类型违规
    let bad_type = edit_form(&[
        ("model", None, b"m"),
        ("prompt", None, b"p"),
        ("image", Some("a.gif"), b"GIF"),
    ]);
    assert!(validate_image_edit_form(&bad_type.1, Some(bad_type.0.as_str())).is_some(), "gif 参考图须被拒");

    // 缺 boundary → 违规
    assert!(validate_image_edit_form(&ok.1, Some("multipart/form-data")).is_some());
    assert!(validate_image_edit_form(&ok.1, None).is_some());
}

#[test]
fn image_edit_model_part_rewritten_in_place() {
    let (ct, body) = edit_form(&[
        ("model", None, b"client-img"),
        ("prompt", None, b"a cat"),
        ("image[]", Some("a.png"), b"PNGDATA-A"),
        ("image[]", Some("b.png"), b"PNGDATA-B"),
    ]);
    let boundary = boundary_from_content_type(&ct).expect("boundary 解析");
    let out = rewrite_multipart_model(&body, &boundary, "img-slot");
    assert_eq!(multipart_form_field(&out, "model"), "img-slot");
    assert_eq!(multipart_form_field(&out, "prompt"), "a cat", "其余 part 不动");
    assert!(
        out.windows(b"PNGDATA-A".len()).any(|w| w == b"PNGDATA-A")
            && out.windows(b"PNGDATA-B".len()).any(|w| w == b"PNGDATA-B"),
        "图片 part 字节原样保留"
    );
    assert!(
        out.windows(4).any(|w| w == b"--\r\n") || out.ends_with(b"--\r\n"),
        "终结边界保留"
    );
    // 客户端 model 值已被替换（不再是 client-img）
    assert!(!out.windows(b"client-img".len()).any(|w| w == b"client-img"));
}

fn aux_req<'a>(value: &'a Value, bytes: &'a [u8], content_type: Option<&'a str>) -> AuxRequest<'a> {
    AuxRequest { value, bytes, model: "m", content_type }
}

#[test]
fn image_stream_request_detection() {
    let s = serde_json::json!({"stream": true});
    assert!(image_stream_requested(&IMAGE, &aux_req(&s, b"", None)), "JSON stream:true");
    let ns = serde_json::json!({"stream": false});
    assert!(!image_stream_requested(&IMAGE, &aux_req(&ns, b"", None)));
    assert!(!image_stream_requested(&IMAGE, &aux_req(&Value::Null, b"", None)));

    let (ct, body) = edit_form(&[("model", None, b"m"), ("stream", None, b"true")]);
    assert!(
        image_stream_requested(&IMAGE_EDITS, &aux_req(&Value::Null, &body, Some(ct.as_str()))),
        "multipart stream=true"
    );
    let (ct2, body2) = edit_form(&[("model", None, b"m"), ("stream", None, b"false")]);
    assert!(!image_stream_requested(&IMAGE_EDITS, &aux_req(&Value::Null, &body2, Some(ct2.as_str()))));
}

#[test]
fn image_stream_format_conflict_gif_svg() {
    let req = serde_json::json!({"stream": true, "output_format": "gif"});
    assert_eq!(
        image_stream_format_conflict(&IMAGE, &aux_req(&req, b"", None)).as_deref(),
        Some("gif"),
        "官方：GIF 不支持流式与 partial"
    );
    let req_svg = serde_json::json!({"stream": true, "output_format": "svg"});
    assert_eq!(image_stream_format_conflict(&IMAGE, &aux_req(&req_svg, b"", None)).as_deref(), Some("svg"));
    let req_png = serde_json::json!({"stream": true, "output_format": "png"});
    assert_eq!(image_stream_format_conflict(&IMAGE, &aux_req(&req_png, b"", None)), None);
    let no_stream = serde_json::json!({"output_format": "gif"});
    assert_eq!(
        image_stream_format_conflict(&IMAGE, &aux_req(&no_stream, b"", None)),
        None,
        "非流式请求不拦"
    );

    let (ct, body) = edit_form(&[
        ("model", None, b"m"),
        ("stream", None, b"true"),
        ("output_format", None, b"svg"),
    ]);
    assert_eq!(
        image_stream_format_conflict(&IMAGE_EDITS, &aux_req(&Value::Null, &body, Some(ct.as_str()))).as_deref(),
        Some("svg"),
        "multipart 表单同判"
    );
}

#[test]
fn image_stream_usage_extracts_completed_event() {
    // 经 ImageStreamAcc::feed（流内逐行累计）验证 completed 事件 usage 提取。
    let feed = |chunks: &[&[u8]]| {
        let mut acc = ImageStreamAcc::default();
        for c in chunks {
            acc.feed(c);
        }
        (acc.input, acc.output, acc.completed)
    };
    let sse = concat!(
        "event: image_generation.partial_image\r\n",
        "data: {\"type\":\"image_generation.partial_image\",\"b64_json\":\"AA==\",\"partial_image_index\":0}\r\n\r\n",
        "event: image_generation.completed\r\n",
        "data: {\"type\":\"image_generation.completed\",\"b64_json\":\"QQ==\",\"usage\":{\"input_tokens\":10,\"output_tokens\":50,\"total_tokens\":60}}\r\n\r\n",
    );
    assert_eq!(feed(&[sse.as_bytes()]), (10, 50, true));
    // completed 事件跨 chunk 截断（usage 字段被切在两个 feed 之间）
    let mid = sse.find("\"output_tokens\"").unwrap();
    assert_eq!(
        feed(&[&sse.as_bytes()[..mid], &sse.as_bytes()[mid..]]),
        (10, 50, true),
        "跨 chunk 截断的 completed 行仍可解析"
    );
    // 中断：只有 partial，无 completed → 用量 0、未完成
    assert_eq!(
        feed(&[b"data: {\"type\":\"image_generation.partial_image\",\"b64_json\":\"AA==\"}"]),
        (0, 0, false)
    );
    assert_eq!(feed(&[b""]), (0, 0, false));
    assert_eq!(feed(&[b"data: [DONE]"]), (0, 0, false));
}

// ══ 票 07：费用状态（known / estimated / unknown / incomplete）══

/// 四类费用状态各有映射（票 03 口径）：
/// - 非流式 2xx：token 用量或 data 张数在场 → known；全缺（张数按请求 n 推算）→ estimated。
/// - 失败终态：有已收用量 → incomplete；无用量 → unknown。
/// - 流式终态：completed → known；中断有已计费用（含 partial 附加费）→ incomplete；全无 → unknown。
#[test]
fn image_fee_states_covered() {
    use super::{failure_fee_state, image_fee_state, stream_fee_state};
    // known：token 用量在场 / data 张数在场
    assert_eq!(image_fee_state(10, 0, "{}"), "known");
    assert_eq!(image_fee_state(0, 0, r#"{"data":[{"b64_json":"aGk="}]}"#), "known");
    // estimated：全缺，张数只能按请求 n / 官方缺省推算
    assert_eq!(image_fee_state(0, 0, r#"{"created":1}"#), "estimated");
    assert_eq!(image_fee_state(0, 0, r#"{"data":[]}"#), "estimated", "空 data 不算计量到场");
    // 失败终态
    assert_eq!(failure_fee_state(0, 0), "unknown");
    assert_eq!(failure_fee_state(7, 0), "incomplete");
    // 流式终态
    assert_eq!(stream_fee_state(true, 10, 150), "known");
    assert_eq!(stream_fee_state(false, 0, 100), "incomplete", "partial 附加费是已计费用");
    assert_eq!(stream_fee_state(false, 3, 0), "incomplete");
    assert_eq!(stream_fee_state(false, 0, 0), "unknown");
}

/// partial_image 事件计数 + 官方附加费（每张 100 图像输出 token）计入 output。
#[test]
fn image_stream_acc_counts_partials_for_surcharge() {
    let mut acc = ImageStreamAcc::default();
    acc.feed(b"data: {\"type\":\"image_generation.partial_image\",\"b64_json\":\"AA==\"}\r\n\r\n");
    acc.feed(b"data: {\"type\":\"image_generation.partial_image\",\"b64_json\":\"BA==\"}\r\n\r\n");
    assert_eq!(acc.partials, 2, "两张 partial 事件计数");
    acc.feed(b"data: {\"type\":\"image_generation.completed\",\"b64_json\":\"QQ==\",\"usage\":{\"input_tokens\":10,\"output_tokens\":50}}\r\n\r\n");
    assert_eq!((acc.input, acc.output, acc.partials), (10, 50, 2));
    // guard 计费口径：output = completed usage + 100 × partials = 250
    let output_with_surcharge = acc.output + 100 * acc.partials;
    assert_eq!(output_with_surcharge, 250);
    assert_eq!(stream_fee_state(acc.completed, acc.input, output_with_surcharge), "known");
    // 只有两张 partial 中断：output 附加费 200 → incomplete（非 unknown）
    let mut acc2 = ImageStreamAcc::default();
    acc2.feed(b"data: {\"type\":\"image_generation.partial_image\",\"b64_json\":\"AA==\"}\r\n\r\n");
    acc2.feed(b"data: {\"type\":\"image_generation.partial_image\",\"b64_json\":\"BA==\"}\r\n\r\n");
    let out2 = acc2.output + 100 * acc2.partials;
    assert_eq!(out2, 200);
    assert_eq!(stream_fee_state(acc2.completed, acc2.input, out2), "incomplete");
}
