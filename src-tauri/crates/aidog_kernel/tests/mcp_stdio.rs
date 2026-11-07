//! 冒烟测试（magpie 票 10）：真 spawn `aidog-kernel mcp` 子进程，
//! 按 MCP stdio 传输（newline-delimited JSON-RPC）写请求行、断言响应行。纯本地，无外呼。

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[tokio::test]
async fn mcp_stdio_roundtrip() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aidog-kernel"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn aidog-kernel mcp");

    let mut stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");
    let mut reader = BufReader::new(stdout).lines();

    let requests = [
        // initialize 握手
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}"#,
        // 工具清单
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        // 票 11：web_search 真实现，未配置 env 时应回「未配置」的 isError 结果
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"web_search","arguments":{"query":"hi"}}}"#,
        // generate_image 仍是票 12 占位：应回「未实现」
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"generate_image","arguments":{"prompt":"p"}}}"#,
    ];
    for line in requests {
        stdin.write_all(line.as_bytes()).await.expect("write request");
        stdin.write_all(b"\n").await.expect("write newline");
    }
    stdin.flush().await.expect("flush stdin");
    // 请求发完即关写侧（shutdown 只 flush 不关 fd，EOF 必须靠 drop 送达）：
    // 服务端读到 EOF 正常退出，已写出的响应行仍能从 stdout 读到。
    drop(stdin);

    let mut responses = Vec::new();
    for _ in 0..4 {
        let line = tokio::time::timeout(Duration::from_secs(10), reader.next_line())
            .await
            .expect("timeout waiting for response line")
            .expect("stdout closed before all responses arrived")
            .expect("read response line");
        let v: serde_json::Value =
            serde_json::from_str(&line).expect("each stdout line must be a JSON-RPC message");
        assert_eq!(v["jsonrpc"], "2.0");
        responses.push(v);
    }

    let init = &responses[0];
    assert_eq!(init["id"], 1);
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert!(init["result"]["capabilities"]["tools"].is_object());

    let list = &responses[1];
    assert_eq!(list["id"], 2);
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        4,
        "骨架声明 4 个工具，实得 {}",
        serde_json::to_string(tools).unwrap_or_default()
    );

    let call = &responses[2];
    assert_eq!(call["id"], 3);
    let text = call["result"]["content"][0]["text"].as_str().expect("content text");
    assert!(
        text.contains("未配置"),
        "票 11 未配置 provider 的 web_search 应回「未配置」，实得 {text}"
    );
    assert_eq!(call["result"]["isError"], true);

    let image = &responses[3];
    assert_eq!(image["id"], 4);
    let text = image["result"]["content"][0]["text"].as_str().expect("content text");
    assert!(text.contains("未实现"), "generate_image 仍是票 12 占位，实得 {text}");
    assert_eq!(image["result"]["isError"], true);

    let status = child.wait().await.expect("wait child");
    assert!(status.success(), "stdin EOF 后子进程应正常退出，实得 {status}");
}
