//! 书生·端砚（intern_inkstone）DSML 工具调用文本标记 → 标准 tool_use。
//!
//! dsv4 系模型把工具调用以 DSML 文本标记写进正文（anthropic `text_delta` / text 块），
//! 而非结构化 tool_use 块。Claude Code 收到纯文本无法执行工具——用户视角「调用总是断开」。
//! 标记形如（`｜` 为全角 U+FF5C，非 ASCII 竖线）：
//!
//! ```text
//! <｜DSML｜tool_calls>
//! <｜DSML｜invoke name="Bash">
//! <｜DSML｜parameter name="arguments" string="false">{"command": "…"}</｜DSML｜parameter>
//! </｜DSML｜invoke>
//! </｜DSML｜tool_calls>
//! ```
//!
//! 三个入口：
//! - [`parse_dsml_tool_calls`]：完整组文本 → `(name, input)` 列表（组 = 含首尾 tool_calls 标签）。
//! - [`convert_dsml_in_body`]：非流式 anthropic 形状响应体改写（text 块剥标记 + 追加 tool_use 块）。
//! - [`DsmlSseRewriter`]：流式透传分支逐帧改写（text_delta 剥标记 + 合成 tool_use 帧），
//!   仿 `SseThinkingStripper` idiom（帧缓冲 + 完整帧过滤）。
//!
//! 廉价预筛：一切入口先 `str::contains("DSML")`（4 ASCII 字节，memchr 级），不含则零成本直通。

/// 组开标签。`｜` 全角 U+FF5C。
pub const DSML_OPEN: &str = "<｜DSML｜tool_calls>";
/// 组闭标签。
pub const DSML_CLOSE: &str = "</｜DSML｜tool_calls>";

/// 廉价预筛：文本含 DSML 痕迹。仅用于整帧 / 整 body 级；增量流不得用本函数预筛
/// （标签可能被 chunk 切断，此时本函数 false 但流后续会命中）。
pub fn maybe_dsml(text: &str) -> bool {
    text.contains("DSML")
}

/// 解析一个完整 DSML 组（含首尾 `<｜DSML｜tool_calls>` 标签）为工具调用列表。
///
/// invoke 的多 parameter 组装规则：唯一名为 `arguments` 且值为对象 → 直接作 input；
/// 否则按 `{参数名: 值}` 平铺。`string="true"` 的 parameter 体按裸字符串取值，其余按
/// JSON 解析、失败回落裸字符串。解析尽力而为：单条 invoke 坏了跳过，不毒化整组。
pub fn parse_dsml_tool_calls(group: &str) -> Vec<(String, serde_json::Value)> {
    const INVOKE_OPEN: &str = "<｜DSML｜invoke ";
    const INVOKE_CLOSE: &str = "</｜DSML｜invoke>";
    let mut out = Vec::new();
    let mut rest = group;
    while let Some(i) = rest.find(INVOKE_OPEN) {
        let after = &rest[i + INVOKE_OPEN.len()..];
        let Some((name, body_start)) = parse_name_attr(after) else {
            break;
        };
        let Some(end) = body_start.find(INVOKE_CLOSE) else {
            break;
        };
        out.push((name.to_string(), parse_parameters(&body_start[..end])));
        rest = &body_start[end + INVOKE_CLOSE.len()..];
    }
    out
}

/// `<｜DSML｜invoke name="X">` 的 name 属性：返回 (name, '>' 之后的剩余)。
fn parse_name_attr(s: &str) -> Option<(&str, &str)> {
    let s = s.strip_prefix("name=\"")?;
    let q = s.find('"')?;
    let after_quote = &s[q + 1..];
    let gt = after_quote.find('>')?;
    Some((&s[..q], &after_quote[gt + 1..]))
}

/// invoke 体 → input 对象。
fn parse_parameters(invoke_body: &str) -> serde_json::Value {
    const P_OPEN: &str = "<｜DSML｜parameter ";
    const P_CLOSE: &str = "</｜DSML｜parameter>";
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();
    let mut rest = invoke_body;
    while let Some(i) = rest.find(P_OPEN) {
        let after = &rest[i + P_OPEN.len()..];
        let Some(gt) = after.find('>') else { break };
        let header = &after[..gt];
        let body_start = &after[gt + 1..];
        let Some(end) = body_start.find(P_CLOSE) else { break };
        let raw_body = body_start[..end].trim();
        let name = header
            .split_whitespace()
            .find_map(|kv| kv.strip_prefix("name=\"").and_then(|v| v.strip_suffix('"')))
            .unwrap_or_default()
            .to_string();
        let is_string = header.contains("string=\"true\"");
        let value = if is_string {
            serde_json::Value::String(raw_body.to_string())
        } else {
            serde_json::from_str(raw_body)
                .unwrap_or_else(|_| serde_json::Value::String(raw_body.to_string()))
        };
        params.push((name, value));
        rest = &body_start[end + P_CLOSE.len()..];
    }
    // 组装：唯一 arguments 且为对象 → 直接用；否则平铺 {name: value}
    if params.len() == 1 && params[0].0 == "arguments" && params[0].1.is_object() {
        return params.pop().map(|(_, v)| v).unwrap_or(serde_json::Value::Null);
    }
    let mut map = serde_json::Map::new();
    for (k, v) in params {
        map.insert(k, v);
    }
    serde_json::Value::Object(map)
}

/// 非流式 anthropic 形状响应体改写：content[] 各 text 块剥 DSML 标记（剥后空的 text 块
/// 删除），解析出的工具调用追加为 tool_use 块，`stop_reason` 改 `tool_use`。
/// 返回是否改动（false = 无标记，原样）。openai 形状不在本入口覆盖（未观测到）。
pub fn convert_dsml_in_body(body: &mut serde_json::Value) -> bool {
    let Some(content) = body.get_mut("content").and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let mut tools: Vec<(String, serde_json::Value)> = Vec::new();
    let mut changed = false;
    for block in content.iter_mut() {
        if block.get("type").and_then(|t| t.as_str()) != Some("text") {
            continue;
        }
        let Some(text) = block.get("text").and_then(|t| t.as_str()) else {
            continue;
        };
        if !maybe_dsml(text) {
            continue;
        }
        let (clean, groups) = split_dsml_groups(text);
        tools.extend(groups.iter().flat_map(|g| parse_dsml_tool_calls(g)));
        changed = true;
        if let Some(obj) = block.as_object_mut() {
            obj.insert(
                "text".into(),
                serde_json::Value::String(clean.trim().to_string()),
            );
        }
    }
    if !changed {
        return false;
    }
    content.retain(|b| {
        b.get("type").and_then(|t| t.as_str()) != Some("text")
            || !b.get("text").and_then(|t| t.as_str()).is_some_and(|t| t.is_empty())
    });
    for (i, (name, input)) in tools.into_iter().enumerate() {
        content.push(serde_json::json!({
            "type": "tool_use",
            "id": format!("dsml-tool-{i:04x}"),
            "name": name,
            "input": input,
        }));
    }
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stop_reason".into(), serde_json::json!("tool_use"));
    }
    true
}

/// 流式文本分离段。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Text(String),
    /// 完整组（含首尾标签）。
    Dsml(String),
}

/// DSML 组的流式增量分离状态机（`InlineReasoningSplitter` 同 idiom）。
/// `push` 喂增量，返回本次可确定归类的段；可能是被切断标签的尾部留缓冲。
#[derive(Default)]
struct DsmlSplitter {
    buf: String,
    in_group: bool,
}

impl DsmlSplitter {
    /// 空闲：无在途组、无残留缓冲（可能的开标签前缀）。快路径判据。
    fn is_idle(&self) -> bool {
        !self.in_group && self.buf.is_empty()
    }

    fn push(&mut self, delta: &str) -> Vec<Segment> {
        self.buf.push_str(delta);
        let mut out = Vec::new();
        loop {
            if self.in_group {
                // 组内：找闭标签。未到 → 全部留缓冲（组内容不外发）。
                if let Some(i) = self.buf.find(DSML_CLOSE) {
                    let end = i + DSML_CLOSE.len();
                    out.push(Segment::Dsml(self.buf[..end].to_string()));
                    self.buf.drain(..end);
                    self.in_group = false;
                    continue;
                }
                break;
            }
            match self.buf.find('<') {
                Some(i) => {
                    if i > 0 {
                        out.push(Segment::Text(self.buf[..i].to_string()));
                        self.buf.drain(..i);
                    }
                    if self.buf.starts_with(DSML_OPEN) {
                        self.buf.drain(..DSML_OPEN.len());
                        self.in_group = true;
                        continue;
                    }
                    if is_strict_prefix(DSML_OPEN, &self.buf) {
                        break; // 被切断的开标签，留缓冲
                    }
                    out.push(Segment::Text("<".to_string()));
                    self.buf.drain(..1);
                }
                None => {
                    let rest = std::mem::take(&mut self.buf);
                    if !rest.is_empty() {
                        out.push(Segment::Text(rest));
                    }
                    break;
                }
            }
        }
        out
    }

    /// 流结束冲刷。未闭合的组按正文吐回（截断流解析不出工具，标记原样保留更可诊断）。
    fn finish(&mut self) -> Vec<Segment> {
        let rest = std::mem::take(&mut self.buf);
        if rest.is_empty() {
            return Vec::new();
        }
        if self.in_group {
            vec![Segment::Text(format!("{DSML_OPEN}{rest}"))]
        } else {
            vec![Segment::Text(rest)]
        }
    }
}

/// 整段文本一次性分离：返回 (干净正文, 完整 DSML 组列表)。
fn split_dsml_groups(text: &str) -> (String, Vec<String>) {
    let mut sp = DsmlSplitter::default();
    let mut segs = sp.push(text);
    segs.extend(sp.finish());
    let mut clean = String::new();
    let mut groups = Vec::new();
    for s in segs {
        match s {
            Segment::Text(t) => clean.push_str(&t),
            Segment::Dsml(g) => groups.push(g),
        }
    }
    (clean, groups)
}

/// s 是否是 marker 的严格前缀（s 短于 marker 且逐字节相同）。
fn is_strict_prefix(marker: &str, s: &str) -> bool {
    s.len() < marker.len() && marker.as_bytes().starts_with(s.as_bytes())
}

/// 流式 SSE 逐帧改写器（透传分支，anthropic wire）：text_delta 剥 DSML 标记，
/// 完整组在块收束点（`content_block_stop` / `message_delta` / `message_stop` 帧）前
/// 合成 tool_use 帧序列（start / input_json_delta / stop），`message_delta` 的
/// `stop_reason` 改 `tool_use`。非 data 帧与非 JSON data 帧原样过。
///
/// 快路径：分离器空闲、无待 flush 工具、帧不含 '<' 也不是块 start 帧 → 原样返回，零解析成本。
pub struct DsmlSseRewriter {
    /// 帧缓冲（push 喂入的文本按 "\n\n" 切帧）。
    frame_buf: String,
    splitter: DsmlSplitter,
    /// 干净正文累积（一个 delta 帧内的文本段 + 留缓冲的残留合并出下一个有效 delta）。
    clean_text: String,
    /// 合成 tool_use 帧的下一个 index（首个 content_block_start 时初始化）。
    next_index: u32,
    /// 待 flush 的工具调用（组完成即解析入列，块收束点统一合成帧）。
    pending: Vec<(String, serde_json::Value)>,
    /// 已产出 tool_use 帧 → message_delta 的 stop_reason 改写为 tool_use。
    saw_tool: bool,
}

impl Default for DsmlSseRewriter {
    fn default() -> Self {
        Self::new()
    }
}

impl DsmlSseRewriter {
    pub fn new() -> Self {
        Self {
            frame_buf: String::new(),
            splitter: DsmlSplitter::default(),
            clean_text: String::new(),
            next_index: 0,
            pending: Vec::new(),
            saw_tool: false,
        }
    }

    /// 喂入一段上游 SSE 文本，返回可下发的改写后文本（不完整尾帧留内部缓冲）。
    pub fn push(&mut self, text: &str) -> String {
        self.frame_buf.push_str(text);
        let mut out = String::new();
        while let Some(pos) = self.frame_buf.find("\n\n") {
            let frame: String = self.frame_buf.drain(..pos + 2).collect();
            if let Some(kept) = self.filter_frame(&frame) {
                out.push_str(&kept);
            }
        }
        out
    }

    /// 冲刷残留（上游流结束 / 终止符到达时调）。
    pub fn finish(&mut self) -> String {
        let rest = std::mem::take(&mut self.frame_buf);
        if rest.is_empty() {
            return String::new();
        }
        self.filter_frame(&rest).unwrap_or_default()
    }

    /// 单帧决策：`None` = 整帧丢弃；`Some(s)` = 下发（可能改写）。
    fn filter_frame(&mut self, frame: &str) -> Option<String> {
        // 快路径：分离器空闲、无待 flush 工具、帧无 '<'（开标记首字符）、无块 start 帧
        // （要记 index）→ 原样过。注意 "DSML" 整串检测不够——半个开标签（"<｜DS"）不含
        // 整串但必须过分离器缓冲（跨帧切断的标记）。
        if self.splitter.is_idle()
            && self.pending.is_empty()
            && !self.saw_tool
            && !frame.contains('<')
            && !frame.contains("content_block_start")
        {
            return Some(frame.to_string());
        }
        let (prefix, data_json) = split_data_line(frame)?;
        let Ok(mut json) = serde_json::from_str::<serde_json::Value>(data_json) else {
            return Some(frame.to_string());
        };
        let ty = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "content_block_start" => {
                // 记录 max index + 1，供合成 tool 帧取号（text/tool 块都经过这里）。
                if let Some(i) = json.get("index").and_then(|v| v.as_u64()) {
                    self.next_index = self.next_index.max(i as u32 + 1);
                }
                Some(frame.to_string())
            }
            "content_block_delta" => {
                let is_text = json
                    .pointer("/delta/type")
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| t == "text_delta");
                if !is_text {
                    return Some(frame.to_string());
                }
                let Some(text) = json
                    .pointer("/delta/text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
                else {
                    return Some(frame.to_string());
                };
                // 文本级快筛：空闲且不可能含（半个）开标记 → 原样过。
                if self.splitter.is_idle() && !text.contains('<') && !maybe_dsml(&text) {
                    return Some(frame.to_string());
                }
                let segs = self.splitter.push(&text);
                self.collect(segs);
                let clean = std::mem::take(&mut self.clean_text);
                if clean.is_empty() {
                    // 本帧文本全被标记吃掉：不能在此处插 tool 帧（text 块未收束，
                    // Anthropic 块不可交错），留到收束点 flush。整帧丢弃。
                    None
                } else {
                    if let Some(delta) = json
                        .as_object_mut()
                        .and_then(|o| o.get_mut("delta"))
                        .and_then(|d| d.as_object_mut())
                    {
                        delta.insert("text".into(), serde_json::Value::String(clean));
                    }
                    Some(format!("{prefix}data: {json}\n\n"))
                }
            }
            // 收束点。content_block_stop：先发本帧（text 块收尾）再 flush 合成 tool 帧
            //（Anthropic 块按序，text 不 stop 不能开 tool）。message_delta / message_stop：
            // 兜底 flush（正常路径已在块 stop 处 flush 完）。
            "content_block_stop" => {
                let mut out = format!("{prefix}data: {json}\n\n");
                out.push_str(&self.flush_pending());
                Some(out)
            }
            "message_delta" | "message_stop" => {
                let mut out = self.flush_pending();
                if ty == "message_delta"
                    && self.saw_tool
                    && let Some(delta) = json
                        .as_object_mut()
                        .and_then(|o| o.get_mut("delta"))
                        .and_then(|d| d.as_object_mut())
                    {
                        delta.insert(
                            "stop_reason".into(),
                            serde_json::Value::String("tool_use".into()),
                        );
                    }
                out.push_str(&format!("{prefix}data: {json}\n\n"));
                Some(out)
            }
            _ => Some(frame.to_string()),
        }
    }

    /// 分离段收集：干净文本并入 clean_text（等下一个 delta 帧或收束点一并下发），
    /// 完整组解析入 pending。
    fn collect(&mut self, segs: Vec<Segment>) {
        for s in segs {
            match s {
                Segment::Text(t) => self.clean_text.push_str(&t),
                Segment::Dsml(g) => self.pending.extend(parse_dsml_tool_calls(&g)),
            }
        }
    }

    /// 合成 pending 工具的 tool_use 帧序列（start + input_json_delta + stop × n）。
    fn flush_pending(&mut self) -> String {
        let mut out = String::new();
        let mut idx = self.next_index;
        for (i, (name, input)) in std::mem::take(&mut self.pending).into_iter().enumerate() {
            let id = format!("dsml-tool-stream-{i:04x}");
            out.push_str(&format!(
                "event: content_block_start\ndata: {}\n\n",
                serde_json::json!({
                    "type": "content_block_start", "index": idx,
                    "content_block": { "type": "tool_use", "id": id, "name": name, "input": {} }
                })
            ));
            out.push_str(&format!(
                "event: content_block_delta\ndata: {}\n\n",
                serde_json::json!({
                    "type": "content_block_delta", "index": idx,
                    "delta": { "type": "input_json_delta", "partial_json": input.to_string() }
                })
            ));
            out.push_str(&format!(
                "event: content_block_stop\ndata: {}\n\n",
                serde_json::json!({ "type": "content_block_stop", "index": idx })
            ));
            self.saw_tool = true;
            idx += 1;
        }
        self.next_index = idx;
        out
    }
}

/// 拆一帧为 (data 行之前的部分, data 行 JSON)。无 data 行返回 None。（thinking_strip 同款。）
fn split_data_line(frame: &str) -> Option<(String, &str)> {
    let mut prefix = String::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("data: ") {
            return Some((prefix, rest));
        }
        if !line.is_empty() {
            prefix.push_str(line);
            prefix.push('\n');
        }
    }
    None
}

#[cfg(test)]
#[path = "test_dsml.rs"]
mod test_dsml;
