use serde_json::Value;

use super::super::super::types::*;
use super::to_openai;

fn user_blocks(blocks: Vec<ContentBlock>) -> Message {
    Message { role: Role::User, content: MessageContent::Blocks(blocks) }
}
fn assistant_blocks(blocks: Vec<ContentBlock>) -> Message {
    Message { role: Role::Assistant, content: MessageContent::Blocks(blocks) }
}
fn base_req(messages: Vec<Message>) -> ChatRequest {
    ChatRequest {
        model: "kimi-k2".into(),
        messages,
        system: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        stream: None,
        tools: None,
        tool_choice: None,
        extra: None,
    }
}

// 缺陷 C: assistant 消息仅含 thinking(Unknown) 块,不得产出空 {"role":"assistant"} message
#[test]
fn thinking_only_assistant_message_is_skipped() {
    let thinking = ContentBlock::Unknown(serde_json::json!({
        "type": "thinking", "thinking": "let me think", "signature": "sig"
    }));
    let req = base_req(vec![
        Message { role: Role::User, content: MessageContent::Text("hi".into()) },
        assistant_blocks(vec![thinking]),
    ]);
    let out = to_openai(&req);
    // 只应保留 user 消息,thinking-only assistant 被跳过(否则 Kimi 400)
    assert_eq!(out.messages.len(), 1, "thinking-only assistant 不应产出空 message");
    assert_eq!(out.messages[0].role, "user");
}

// 缺陷 C 变体: thinking + tool_use 混排 → 保留 tool_calls,丢 thinking,非空
#[test]
fn thinking_plus_tool_use_keeps_tool_calls() {
    let req = base_req(vec![assistant_blocks(vec![
        ContentBlock::Unknown(serde_json::json!({"type":"thinking","thinking":"t"})),
        ContentBlock::ToolUse {
            id: "call_1".into(),
            name: "read_file".into(),
            input: serde_json::json!({"path":"/a"}),
        },
    ])]);
    let out = to_openai(&req);
    assert_eq!(out.messages.len(), 1);
    let m = &out.messages[0];
    assert_eq!(m.role, "assistant");
    let tcs = m.tool_calls.as_ref().expect("tool_calls present");
    assert_eq!(tcs.len(), 1);
    assert!(m.content.is_none(), "无 text 时 content 应省略而非空字符串");
}

// 缺陷 A: user 消息含 tool_result + text 混排,文本不得丢失
#[test]
fn tool_result_plus_text_preserves_text() {
    let req = base_req(vec![user_blocks(vec![
        ContentBlock::ToolResult { tool_use_id: "call_1".into(), content: "file content".into() },
        ContentBlock::Text { text: "now do X".into() },
    ])]);
    let out = to_openai(&req);
    // tool message + 残余 user 文本 message
    assert_eq!(out.messages.len(), 2);
    assert_eq!(out.messages[0].role, "tool");
    assert_eq!(out.messages[0].tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(out.messages[1].role, "user");
    assert_eq!(out.messages[1].content, Some(Value::String("now do X".into())));
}

// 缺陷 A 变体: 多个 tool_result(并行工具)各自成 tool message,tool_call_id 对应
#[test]
fn multiple_tool_results_each_become_tool_message() {
    let req = base_req(vec![user_blocks(vec![
        ContentBlock::ToolResult { tool_use_id: "c1".into(), content: "r1".into() },
        ContentBlock::ToolResult { tool_use_id: "c2".into(), content: "r2".into() },
    ])]);
    let out = to_openai(&req);
    assert_eq!(out.messages.len(), 2);
    assert_eq!(out.messages[0].tool_call_id.as_deref(), Some("c1"));
    assert_eq!(out.messages[1].tool_call_id.as_deref(), Some("c2"));
}

// 多段 text 块拼成单一字符串(非 array),避免 Kimi 拒多模态结构
#[test]
fn multiple_text_blocks_join_into_string() {
    let req = base_req(vec![user_blocks(vec![
        ContentBlock::Text { text: "part1".into() },
        ContentBlock::Text { text: "part2".into() },
    ])]);
    let out = to_openai(&req);
    assert_eq!(out.messages.len(), 1);
    assert_eq!(out.messages[0].content, Some(Value::String("part1\npart2".into())));
}

// system 字符串消息放在最前
#[test]
fn system_text_becomes_first_message() {
    let mut req = base_req(vec![
        Message { role: Role::User, content: MessageContent::Text("hello".into()) },
    ]);
    req.system = Some(SystemContent::Text("You are helpful".into()));
    let out = to_openai(&req);
    assert_eq!(out.messages.len(), 2);
    assert_eq!(out.messages[0].role, "system");
    assert_eq!(out.messages[0].content, Some(Value::String("You are helpful".into())));
    assert_eq!(out.messages[1].role, "user");
}

// system blocks: 提取 text 字段拼接
#[test]
fn system_blocks_extract_text() {
    let mut req = base_req(vec![
        Message { role: Role::User, content: MessageContent::Text("q".into()) },
    ]);
    req.system = Some(SystemContent::Blocks(vec![
        serde_json::json!({"type":"text","text":"block1"}),
        serde_json::json!({"type":"text","text":"block2"}),
    ]));
    let out = to_openai(&req);
    assert_eq!(out.messages[0].role, "system");
    assert_eq!(out.messages[0].content, Some(Value::String("block1\nblock2".into())));
}

// 普通 user/assistant 文本消息
#[test]
fn plain_text_user_and_assistant() {
    let req = base_req(vec![
        Message { role: Role::User, content: MessageContent::Text("question".into()) },
        Message { role: Role::Assistant, content: MessageContent::Text("answer".into()) },
    ]);
    let out = to_openai(&req);
    assert_eq!(out.messages.len(), 2);
    assert_eq!(out.messages[0].role, "user");
    assert_eq!(out.messages[1].role, "assistant");
}

// system/tool role messages
#[test]
fn system_role_message_in_messages() {
    let req = base_req(vec![
        Message { role: Role::System, content: MessageContent::Text("sys".into()) },
    ]);
    let out = to_openai(&req);
    assert_eq!(out.messages[0].role, "system");
}

// tools passthrough
#[test]
fn tools_are_converted() {
    let mut req = base_req(vec![
        Message { role: Role::User, content: MessageContent::Text("use tool".into()) },
    ]);
    req.tools = Some(vec![Tool {
        name: "my_tool".into(),
        description: Some("does stuff".into()),
        input_schema: serde_json::json!({"type":"object"}),
    }]);
    let out = to_openai(&req);
    let tools = out.tools.expect("tools present");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].function.name, "my_tool");
    assert_eq!(tools[0].r#type, "function");
}

// tool_choice variants
#[test]
fn tool_choice_auto() {
    let mut req = base_req(vec![Message { role: Role::User, content: MessageContent::Text("q".into()) }]);
    req.tool_choice = Some(ToolChoice::Auto);
    let out = to_openai(&req);
    assert_eq!(out.tool_choice, Some(serde_json::json!("auto")));
}

#[test]
fn tool_choice_any_maps_to_required() {
    let mut req = base_req(vec![Message { role: Role::User, content: MessageContent::Text("q".into()) }]);
    req.tool_choice = Some(ToolChoice::Any);
    let out = to_openai(&req);
    assert_eq!(out.tool_choice, Some(serde_json::json!("required")));
}

#[test]
fn tool_choice_none() {
    let mut req = base_req(vec![Message { role: Role::User, content: MessageContent::Text("q".into()) }]);
    req.tool_choice = Some(ToolChoice::None);
    let out = to_openai(&req);
    assert_eq!(out.tool_choice, Some(serde_json::json!("none")));
}

#[test]
fn tool_choice_named() {
    let mut req = base_req(vec![Message { role: Role::User, content: MessageContent::Text("q".into()) }]);
    req.tool_choice = Some(ToolChoice::Named { name: "my_tool".into() });
    let out = to_openai(&req);
    let tc = out.tool_choice.unwrap();
    assert_eq!(tc["type"], "function");
    assert_eq!(tc["function"]["name"], "my_tool");
}
