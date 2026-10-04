//! dsml.rs 单测。样例取自真实抓包（proxy_log 2026-10-04 书生·端砚 dsv4-flash-vision，
/// 上游把 Bash 工具调用以 DSML 文本标记吐进 anthropic text_delta）。
use super::*;

const REAL_SAMPLE: &str = concat!(
    "<｜DSML｜tool_calls>\n",
    "<｜DSML｜invoke name=\"Bash\">\n",
    "<｜DSML｜parameter name=\"arguments\" string=\"false\">",
    "{\"command\": \"cd /Users/luoxin/persons/sexy\\necho \\\"=== ch8 作者原版路径 ===\\\"\\ngit ls-tree 6e974a9^ 潮汐庄园/章节/ 2>/dev/null | head\", \"description\": \"Find ch8 original path\"}",
    "</｜DSML｜parameter>\n",
    "</｜DSML｜invoke>\n",
    "</｜DSML｜tool_calls>"
);

#[test]
fn parse_real_sample_single_arguments_object() {
    let calls = parse_dsml_tool_calls(REAL_SAMPLE);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "Bash");
    // 唯一 arguments 且为对象 → 直接作 input
    assert_eq!(calls[0].1.get("command").unwrap().as_str().unwrap().lines().count(), 3);
    assert_eq!(
        calls[0].1.get("description").unwrap().as_str().unwrap(),
        "Find ch8 original path"
    );
}

#[test]
fn parse_multiple_invokes_and_flat_params() {
    let group = concat!(
        "<｜DSML｜tool_calls>",
        "<｜DSML｜invoke name=\"Read\">",
        "<｜DSML｜parameter name=\"file_path\" string=\"true\">/tmp/a.rs</｜DSML｜parameter>",
        "</｜DSML｜invoke>",
        "<｜DSML｜invoke name=\"Write\">",
        "<｜DSML｜parameter name=\"arguments\" string=\"false\">{\"x\": 1}</｜DSML｜parameter>",
        "</｜DSML｜invoke>",
        "</｜DSML｜tool_calls>"
    );
    let calls = parse_dsml_tool_calls(group);
    assert_eq!(calls.len(), 2);
    // 非「唯一 arguments」→ 平铺
    assert_eq!(calls[0].1.get("file_path").unwrap().as_str().unwrap(), "/tmp/a.rs");
    // 唯一 arguments → 直接
    assert_eq!(calls[0].1.get("arguments"), None);
    assert_eq!(calls[1].1.get("x").unwrap().as_i64(), Some(1));
}

#[test]
fn rewriter_end_to_end_real_stream_shape() {
    let mut r = DsmlSseRewriter::new();
    let input = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"dsv4\",\"content\":[]}}\n\n",
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"我先查一下。\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"<｜DSML｜tool_calls><｜DSML｜invoke name=\\\"Bash\\\"><｜DSML｜parameter name=\\\"arguments\\\" string=\\\"false\\\">{\\\"command\\\":\\\"ls\\\"}</｜DSML｜parameter></｜DSML｜invoke></｜DSML｜tool_calls>\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":10}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    // 分三段喂（模拟网络 chunk 切断帧 / 切断标记）
    let n = input.len();
    let mut out = r.push(&input[..n / 3]);
    out.push_str(&r.push(&input[n / 3..2 * n / 3]));
    out.push_str(&r.push(&input[2 * n / 3..]));
    out.push_str(&r.finish());

    // 正文保留、标记剥净
    assert!(out.contains("我先查一下。"));
    assert!(!out.contains("DSML"));
    // 合成 tool_use 帧在上游 text 块 stop 之后、message_delta 之前
    let text_stop = out.find("\"type\":\"content_block_stop\",\"index\":0").unwrap();
    let tool_start = out.find("\"type\":\"tool_use\"").unwrap();
    let msg_delta = out.find("\"type\":\"message_delta\"").unwrap();
    assert!(text_stop < tool_start && tool_start < msg_delta);
    // index 取上游 max+1
    assert!(out.contains("\"index\":1,\"content_block\":{\"type\":\"tool_use\""));
    assert!(out.contains("\"partial_json\":\"{\\\"command\\\":\\\"ls\\\"}\""));
    // stop_reason 改写
    assert!(out.contains("\"stop_reason\":\"tool_use\""));
    // message_stop 帧保留
    assert!(out.contains("\"type\":\"message_stop\""));
}

#[test]
fn rewriter_fast_path_no_dsml_passthrough_untouched() {
    let mut r = DsmlSseRewriter::new();
    let frame = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"普通正文\"}}\n\n";
    assert_eq!(r.push(frame), frame);
}

#[test]
fn rewriter_split_marker_across_deltas() {
    let mut r = DsmlSseRewriter::new();
    let d = |t: &str| {
        format!(
            "event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":{}}}}}\n\n",
            serde_json::json!(t)
        )
    };
    let head = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
    let _ = r.push(head);
    // 标记被切成三段（d() 内部经 json! 编码，参数写裸引号）
    let out = r.push(&d("<｜DS"))
        + &r.push(&d("ML｜tool_calls><｜DSML｜invoke name=\"Grep\"><｜DSML｜parameter name=\"pattern\" string=\"true\">foo</｜DSML｜parameter></｜DSML｜invoke></｜DSML｜"))
        + &r.push(&d("tool_calls>"));
    // 中段整帧被吃（组内文本不外发）
    assert!(!out.contains("DSML"));
    let out = out
        + &r.push("event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n")
        + &r.push("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n");
    assert!(out.contains("\"type\":\"tool_use\""), "合成帧：{out}");
    assert!(out.contains("\"name\":\"Grep\""));
    // 平铺参数（非 arguments）：pattern 裸字符串
    assert!(out.contains("\"partial_json\":\"{\\\"pattern\\\":\\\"foo\\\"}\""));
}

#[test]
fn convert_body_strips_and_appends_tool_use() {
    let mut body = serde_json::json!({
        "type": "message",
        "role": "assistant",
        "content": [
            { "type": "text", "text": "查一下。".to_string() + REAL_SAMPLE }
        ],
        "stop_reason": "end_turn",
    });
    assert!(convert_dsml_in_body(&mut body));
    let content = body.get("content").unwrap().as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0].get("text").unwrap().as_str().unwrap(), "查一下。");
    assert_eq!(content[1].get("type").unwrap().as_str().unwrap(), "tool_use");
    assert_eq!(content[1].get("name").unwrap().as_str().unwrap(), "Bash");
    assert_eq!(body.get("stop_reason").unwrap().as_str().unwrap(), "tool_use");
}

#[test]
fn convert_body_pure_dsml_text_block_removed() {
    let mut body = serde_json::json!({
        "content": [ { "type": "text", "text": REAL_SAMPLE } ],
        "stop_reason": "end_turn",
    });
    assert!(convert_dsml_in_body(&mut body));
    let content = body.get("content").unwrap().as_array().unwrap();
    assert_eq!(content.len(), 1, "纯标记 text 块删除，只剩 tool_use");
    assert_eq!(content[0].get("type").unwrap().as_str().unwrap(), "tool_use");
}

#[test]
fn convert_body_no_marker_untouched() {
    let mut body = serde_json::json!({
        "content": [ { "type": "text", "text": "正常回答" } ],
        "stop_reason": "end_turn",
    });
    assert!(!convert_dsml_in_body(&mut body));
    assert_eq!(body.get("stop_reason").unwrap().as_str().unwrap(), "end_turn");
}
