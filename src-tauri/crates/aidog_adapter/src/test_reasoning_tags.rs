use super::*;

// ── 非流式整段分离 ──

#[test]
fn split_thinking_tag_separates_reasoning_and_text() {
    let (r, t) = split_inline_reasoning("<thinking>先查文件再改</thinking>\n\n已完成修改。");
    assert_eq!(r, "先查文件再改");
    assert_eq!(t, "已完成修改。");
}

#[test]
fn split_short_think_tag_also_recognized() {
    let (r, t) = split_inline_reasoning("<think>abc</think>answer");
    assert_eq!(r, "abc");
    assert_eq!(t, "answer");
}

#[test]
fn split_multiple_segments_joined() {
    let (r, t) = split_inline_reasoning("<thinking>一</thinking>正文A<thinking>二</thinking>正文B");
    assert_eq!(r, "一\n\n二");
    assert_eq!(t, "正文A正文B");
}

#[test]
fn split_unclosed_tag_is_all_reasoning() {
    let (r, t) = split_inline_reasoning("正文\n<thinking>被截断的思考");
    assert_eq!(r, "被截断的思考");
    assert_eq!(t, "正文");
}

#[test]
fn split_plain_text_untouched() {
    let (r, t) = split_inline_reasoning("a < b 且 c <d> e");
    assert_eq!(r, "");
    assert_eq!(t, "a < b 且 c <d> e", "非思维链标签的尖括号不动");
}

#[test]
fn split_no_tag_returns_empty_reasoning() {
    let (r, t) = split_inline_reasoning("hello world");
    assert_eq!(r, "");
    assert_eq!(t, "hello world");
    assert!(!has_inline_reasoning_tag("hello world"));
    assert!(has_inline_reasoning_tag("x<think>y"));
}

// ── 流式增量分离 ──

/// 逐 chunk 喂入，收集全部段（含 finish 冲刷）。
fn feed(chunks: &[&str]) -> Vec<Segment> {
    let mut sp = InlineReasoningSplitter::new();
    let mut out = Vec::new();
    for c in chunks {
        out.extend(sp.push(c));
    }
    out.extend(sp.finish());
    out
}

#[test]
fn stream_tag_split_across_chunks() {
    // 标签被 chunk 边界切成 `<thin` + `king>`：不得当正文吐出
    let segs = feed(&["hello <thin", "king>思考中", "</think", "ing>结论"]);
    assert_eq!(
        segs,
        vec![
            Segment::Text("hello ".into()),
            Segment::Reasoning("思考中".into()),
            Segment::Text("结论".into()),
        ]
    );
}

#[test]
fn stream_reasoning_emitted_incrementally() {
    // 思维链段内无闭标签时，正文部分立即下发（不攒到闭标签才吐，避免首 token 时延退化）
    let mut sp = InlineReasoningSplitter::new();
    assert_eq!(sp.push("<thinking>第一段"), vec![Segment::Reasoning("第一段".into())]);
    assert!(sp.in_reasoning());
    assert_eq!(sp.push("第二段</thinking>正文"), vec![
        Segment::Reasoning("第二段".into()),
        Segment::Text("正文".into()),
    ]);
    assert!(!sp.in_reasoning());
    assert!(sp.finish().is_empty());
}

#[test]
fn stream_unclosed_tag_flushed_as_reasoning_on_finish() {
    let segs = feed(&["<thinking>半截"]);
    assert_eq!(segs, vec![Segment::Reasoning("半截".into())]);
}

#[test]
fn stream_multibyte_char_split_across_chunks_not_corrupted() {
    // `<` 后跟中文：partial_tail_len 必须落在字符边界（否则切片 panic）
    let segs = feed(&["a<", "中文</b>"]);
    assert!(segs.iter().all(|s| matches!(s, Segment::Text(_))), "全是正文: {segs:?}");
    let joined: String = segs
        .iter()
        .map(|s| match s {
            Segment::Text(t) | Segment::Reasoning(t) => t.as_str(),
        })
        .collect();
    assert_eq!(joined, "a<中文</b>", "分段可碎，拼回必须一字不差");
}

#[test]
fn stream_plain_text_passthrough() {
    let segs = feed(&["普通", "文本"]);
    assert_eq!(segs, vec![Segment::Text("普通".into()), Segment::Text("文本".into())]);
}
