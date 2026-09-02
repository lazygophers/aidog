//! 行内思维链标签（`<thinking>` / `<think>`）分离。
//!
//! 部分上游模型不走结构化思维链通道（OpenAI `reasoning_content` / Anthropic thinking 块 /
//! Gemini `thought` part），而是把思考直接写进正文文本，用 `<thinking>…</thinking>` 或
//! `<think>…</think>` 包起来。这类响应经转换层落到 Anthropic 客户端时是普通 text 块，
//! Claude Code 按正文渲染 → 用户看到裸标签。
//!
//! 本模块把行内标签段按标准语义**判定为思维链**，交给转换层渲染成 Anthropic `thinking` 块
//! （非流式）/ `thinking_delta`（流式），与结构化通道走同一条出口，不做「留标签给客户端自己认」
//! 的兼容性处理。
//!
//! 两个入口：
//! - [`split_inline_reasoning`]：非流式整段文本一次性分离。
//! - [`InlineReasoningSplitter`]：流式增量分离，跨 chunk 缓冲被切断的标签。

/// 识别的标签对（开标签, 闭标签）。按开标签长度降序排列：`<thinking>` 必须排在
/// `<think>` 之前，否则 `<thinking>` 会被 `<think>` 前缀匹配吃掉前 7 字符。
const TAG_PAIRS: &[(&str, &str)] = &[("<thinking>", "</thinking>"), ("<think>", "</think>")];

/// 分离结果的一段：正文 or 思维链。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Reasoning(String),
}

/// 流式行内标签分离状态机。
///
/// `push` 逐个增量喂入，返回本次能**确定**归类的段；无法确定的尾部（可能是被 chunk 边界
/// 切断的标签，如 `"...<thin"`）留在内部缓冲，等下一个增量拼接后再判。流结束时必须调
/// [`finish`](Self::finish) 冲刷残留，否则未闭合标签内的内容会丢。
#[derive(Default)]
pub struct InlineReasoningSplitter {
    buf: String,
    /// Some(闭标签) = 当前在思维链段内
    open_close_tag: Option<&'static str>,
}

impl InlineReasoningSplitter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前是否处于未闭合的思维链段内（调用方判断流中断时残留归属用）。
    pub fn in_reasoning(&self) -> bool {
        self.open_close_tag.is_some()
    }

    /// 喂入一个文本增量，返回本次可确定归类的段（可能为空）。
    pub fn push(&mut self, delta: &str) -> Vec<Segment> {
        self.buf.push_str(delta);
        let mut out = Vec::new();
        loop {
            match self.open_close_tag {
                // ── 思维链段内：找闭标签 ──
                Some(close) => {
                    if let Some(i) = self.buf.find(close) {
                        push_seg(&mut out, Segment::Reasoning(self.buf[..i].to_string()));
                        self.buf.drain(..i + close.len());
                        self.open_close_tag = None;
                        continue;
                    }
                    // 无闭标签：尾部可能是被切断的闭标签，留缓冲，其余判定为思维链
                    let hold = partial_tail_len(&self.buf, &[close]);
                    let emit_to = self.buf.len() - hold;
                    if emit_to > 0 {
                        push_seg(
                            &mut out,
                            Segment::Reasoning(self.buf[..emit_to].to_string()),
                        );
                        self.buf.drain(..emit_to);
                    }
                    break;
                }
                // ── 正文段内：找开标签 ──
                None => {
                    let Some(i) = self.buf.find('<') else {
                        push_seg(&mut out, Segment::Text(std::mem::take(&mut self.buf)));
                        break;
                    };
                    if i > 0 {
                        push_seg(&mut out, Segment::Text(self.buf[..i].to_string()));
                        self.buf.drain(..i);
                    }
                    // buf 现以 '<' 开头
                    if let Some((open, close)) = TAG_PAIRS
                        .iter()
                        .find(|(open, _)| self.buf.starts_with(*open))
                    {
                        self.buf.drain(..open.len());
                        self.open_close_tag = Some(close);
                        continue;
                    }
                    // 可能是被切断的开标签（如 `"<thin"`）→ 留缓冲等下个增量
                    let opens: Vec<&str> = TAG_PAIRS.iter().map(|(o, _)| *o).collect();
                    if is_strict_prefix_of_any(&self.buf, &opens) {
                        break;
                    }
                    // 确定不是标签的 '<'：作为正文吐出，继续扫后面的内容
                    push_seg(&mut out, Segment::Text("<".to_string()));
                    self.buf.drain(..1);
                }
            }
        }
        out
    }

    /// 冲刷残留缓冲（流结束 / Stop 事件时必须调）。
    /// 未闭合的 `<thinking>` 段按思维链收尾（模型截断在思考中途），正文残留按正文收尾。
    pub fn finish(&mut self) -> Vec<Segment> {
        let rest = std::mem::take(&mut self.buf);
        let in_reasoning = self.open_close_tag.take().is_some();
        if rest.is_empty() {
            return Vec::new();
        }
        vec![if in_reasoning {
            Segment::Reasoning(rest)
        } else {
            Segment::Text(rest)
        }]
    }
}

/// 非流式整段分离：返回 (思维链, 正文)。两者都可能为空串。
///
/// 多段 `<thinking>` 按出现顺序用 `\n\n` 连接；标签本身不出现在任何一侧输出里。
pub fn split_inline_reasoning(text: &str) -> (String, String) {
    let mut sp = InlineReasoningSplitter::new();
    let mut segs = sp.push(text);
    segs.extend(sp.finish());
    let mut reasoning_parts: Vec<String> = Vec::new();
    let mut text_out = String::new();
    for seg in segs {
        match seg {
            Segment::Reasoning(r) => reasoning_parts.push(r),
            Segment::Text(t) => text_out.push_str(&t),
        }
    }
    let reasoning = reasoning_parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    // 标签摘除后正文常留下首尾空白（`<thinking>…</thinking>\n\n正文`）
    (reasoning, text_out.trim().to_string())
}

/// 文本是否含行内思维链标签（调用方零成本预判，避免无标签时白跑状态机）。
pub fn has_inline_reasoning_tag(text: &str) -> bool {
    TAG_PAIRS.iter().any(|(open, _)| text.contains(open))
}

/// 非空段才入列（避免下发空 delta 帧）。
fn push_seg(out: &mut Vec<Segment>, seg: Segment) {
    let empty = match &seg {
        Segment::Text(s) | Segment::Reasoning(s) => s.is_empty(),
    };
    if !empty {
        out.push(seg);
    }
}

/// `s` 是否为任一 tag 的**严格**前缀（即还没凑齐整个标签）。
fn is_strict_prefix_of_any(s: &str, tags: &[&str]) -> bool {
    tags.iter().any(|t| t.len() > s.len() && t.starts_with(s))
}

/// `s` 的最长后缀长度，使该后缀是某个 tag 的严格前缀（跨 chunk 被切断的标签头）。
/// 返回的字节数保证落在字符边界上。
fn partial_tail_len(s: &str, tags: &[&str]) -> usize {
    let max = tags.iter().map(|t| t.len()).max().unwrap_or(0).min(s.len());
    for len in (1..=max).rev() {
        let start = s.len() - len;
        if !s.is_char_boundary(start) {
            continue;
        }
        if is_strict_prefix_of_any(&s[start..], tags) {
            return len;
        }
    }
    0
}

#[cfg(test)]
#[path = "test_reasoning_tags.rs"]
mod test_reasoning_tags;
