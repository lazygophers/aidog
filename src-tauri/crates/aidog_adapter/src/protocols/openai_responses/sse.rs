//! OpenAI Responses 流式出站渲染器（issue 70）。
//!
//! Responses / Completions 客户端的流式输出此前一律走 Chat Completions 分块渲染，
//! 客户端按自身协议解析失败。本模块把中立 [`ChatStreamEvent`] 渲染为官方 Responses
//! 事件序列：`response.created` → `response.in_progress` → 各 output item 的
//! added / delta / done → `response.completed`（或 `response.incomplete`）。
//! 事件名与 Response 对象字段对齐 openai-python 官方类型定义。
//! 出处: <https://platform.openai.com/docs/api-reference/responses-streaming>

use serde_json::{Value, json};

use crate::types::{ChatStreamEvent, Usage};

/// Responses SSE 出站状态机：跨 chunk 维护 sequence_number、output item 开闭表、
/// 各 item 累计内容与 usage（终止帧需要全量回放）。
#[derive(Default)]
pub struct ResponsesSseState {
    seq: u64,
    response_id: String,
    model: String,
    created: bool,
    created_at: i64,
    finished: bool,
    /// 文本 message item：(item_id, output_index, 已累计全文)
    text_item: Option<(String, u64, String)>,
    /// reasoning item：(item_id, output_index, 已累计全文, summary_part_added)
    reasoning_item: Option<(String, u64, String, bool)>,
    /// 中立 tool index → (item_id, output_index, call_id, name, 已累计 arguments)
    tool_items: std::collections::HashMap<u32, (String, u64, String, String, String)>,
    /// item 输出顺序 = 创建顺序
    next_output: u64,
    /// 跨帧累积 usage（issue 71）：`response.completed` / `response.incomplete` 终帧回传，
    /// 缓存读走 `input_tokens_details.cached_tokens`。
    usage: Option<Usage>,
}

impl ResponsesSseState {
    /// 渲染一个中立事件为 0..n 个 Responses SSE 帧（拼好的文本，`event:` + `data:` 行）。
    pub fn push(&mut self, event: &ChatStreamEvent, model: &str) -> Option<String> {
        match event {
            ChatStreamEvent::Start { id, model } => {
                if self.created {
                    return None;
                }
                self.response_id = id.clone();
                self.model = model.clone();
                Some(self.ensure_created(model))
            }
            ChatStreamEvent::Delta { text } => {
                let mut out = self.ensure_created(model);
                if self.text_item.is_none() {
                    let idx = self.next_output;
                    self.next_output += 1;
                    let item_id = format!("msg_{idx}");
                    out.push_str(&self.frame(
                        "response.output_item.added",
                        json!({
                            "type": "response.output_item.added",
                            "output_index": idx,
                            "item": {
                                "id": item_id, "type": "message", "status": "in_progress",
                                "role": "assistant", "content": [],
                            },
                        }),
                    ));
                    out.push_str(&self.frame(
                        "response.content_part.added",
                        json!({
                            "type": "response.content_part.added",
                            "item_id": item_id, "output_index": idx, "content_index": 0,
                            "part": { "type": "output_text", "text": "", "annotations": [] },
                        }),
                    ));
                    self.text_item = Some((item_id, idx, String::new()));
                }
                let (item_id, idx) = {
                    let (item_id, idx, acc) = self.text_item.as_mut().unwrap();
                    acc.push_str(text);
                    (item_id.clone(), *idx)
                };
                out.push_str(&self.frame(
                    "response.output_text.delta",
                    json!({
                        "type": "response.output_text.delta",
                        "item_id": item_id, "output_index": idx, "content_index": 0,
                        "delta": text,
                        "logprobs": [],
                    }),
                ));
                Some(out)
            }
            ChatStreamEvent::ReasoningDelta { text } => {
                let mut out = self.ensure_created(model);
                if self.reasoning_item.is_none() {
                    let idx = self.next_output;
                    self.next_output += 1;
                    let item_id = format!("rs_{idx}");
                    out.push_str(&self.frame(
                        "response.output_item.added",
                        json!({
                            "type": "response.output_item.added",
                            "output_index": idx,
                            "item": {
                                "id": item_id, "type": "reasoning",
                                "content": [], "summary": [],
                            },
                        }),
                    ));
                    self.reasoning_item = Some((item_id, idx, String::new(), false));
                }
                let (item_id, idx, part_added) = {
                    let (item_id, idx, acc, part_added) = self.reasoning_item.as_mut().unwrap();
                    acc.push_str(text);
                    (item_id.clone(), *idx, *part_added)
                };
                // 首个 summary delta 前须发 summary_part.added（官方序列）
                if !part_added {
                    out.push_str(&self.frame(
                        "response.reasoning_summary_part.added",
                        json!({
                            "type": "response.reasoning_summary_part.added",
                            "item_id": item_id, "output_index": idx, "summary_index": 0,
                            "part": { "type": "summary_text", "text": "" },
                        }),
                    ));
                    self.reasoning_item.as_mut().unwrap().3 = true;
                }
                // 思维链走官方 summary 流（summary_index 0）：Codex 等客户端按此事件渲染思考。
                out.push_str(&self.frame(
                    "response.reasoning_summary_text.delta",
                    json!({
                        "type": "response.reasoning_summary_text.delta",
                        "item_id": item_id, "output_index": idx, "summary_index": 0,
                        "delta": text,
                    }),
                ));
                Some(out)
            }
            ChatStreamEvent::ToolDelta {
                index,
                id,
                name,
                input,
            } => {
                let mut out = self.ensure_created(model);
                if let (Some(id), Some(name)) = (id, name)
                    && !self.tool_items.contains_key(index)
                {
                    let idx = self.next_output;
                    self.next_output += 1;
                    let item_id = format!("fc_{idx}");
                    out.push_str(&self.frame(
                        "response.output_item.added",
                        json!({
                            "type": "response.output_item.added",
                            "output_index": idx,
                            "item": {
                                "id": item_id, "type": "function_call", "status": "in_progress",
                                "call_id": id, "name": name, "arguments": "",
                            },
                        }),
                    ));
                    self.tool_items
                        .insert(*index, (item_id, idx, id.clone(), name.clone(), String::new()));
                }
                if let Some(input) = input {
                    if !self.tool_items.contains_key(index) {
                        // 参数增量先于 item 头（异常序列）：补一个占位 item 头，不丢帧
                        let idx = self.next_output;
                        self.next_output += 1;
                        let item_id = format!("fc_{idx}");
                        out.push_str(&self.frame(
                            "response.output_item.added",
                            json!({
                                "type": "response.output_item.added",
                                "output_index": idx,
                                "item": {
                                    "id": item_id, "type": "function_call", "status": "in_progress",
                                    "call_id": format!("call_{index}"), "name": "", "arguments": "",
                                },
                            }),
                        ));
                        self.tool_items.insert(
                            *index,
                            (item_id, idx, format!("call_{index}"), String::new(), String::new()),
                        );
                    }
                    let (item_id, idx) = {
                        let (item_id, idx, _, _, acc) = self.tool_items.get_mut(index).unwrap();
                        acc.push_str(input);
                        (item_id.clone(), *idx)
                    };
                    out.push_str(&self.frame(
                        "response.function_call_arguments.delta",
                        json!({
                            "type": "response.function_call_arguments.delta",
                            "item_id": item_id, "output_index": idx,
                            "delta": input,
                        }),
                    ));
                }
                if out.is_empty() { None } else { Some(out) }
            }
            ChatStreamEvent::Stop { finish_reason } => {
                // Chat 上游的 finish_reason 帧与 data: [DONE] 各产生一个 Stop；第二个是重复终态
                if self.finished {
                    return None;
                }
                self.finished = true;
                let mut out = self.ensure_created(model);
                let incomplete =
                    matches!(finish_reason.as_deref(), Some("length") | Some("max_tokens"));
                // 关闭各 item：每个 item 的 done 帧组按 output_index 升序回放全量内容
                // (output_index, done 帧组, item 全量回放)
                let mut closures: Vec<(u64, String, Value)> = Vec::new();
                if let Some((item_id, idx, acc)) = self.text_item.clone() {
                    let text = acc.clone();
                    let mut frames = String::new();
                    frames.push_str(&self.frame(
                        "response.output_text.done",
                        json!({
                            "type": "response.output_text.done",
                            "item_id": item_id, "output_index": idx, "content_index": 0,
                            "text": text,
                            "logprobs": [],
                        }),
                    ));
                    frames.push_str(&self.frame(
                        "response.content_part.done",
                        json!({
                            "type": "response.content_part.done",
                            "item_id": item_id, "output_index": idx, "content_index": 0,
                            "part": { "type": "output_text", "text": text, "annotations": [] },
                        }),
                    ));
                    let item = json!({
                        "id": item_id, "type": "message", "status": "completed",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": text, "annotations": [] }],
                    });
                    closures.push((idx, frames, item));
                }
                if let Some((item_id, idx, acc, _)) = self.reasoning_item.clone() {
                    let text = acc.clone();
                    let mut frames = String::new();
                    frames.push_str(&self.frame(
                        "response.reasoning_summary_text.done",
                        json!({
                            "type": "response.reasoning_summary_text.done",
                            "item_id": item_id, "output_index": idx, "summary_index": 0,
                            "text": text,
                        }),
                    ));
                    frames.push_str(&self.frame(
                        "response.reasoning_summary_part.done",
                        json!({
                            "type": "response.reasoning_summary_part.done",
                            "item_id": item_id, "output_index": idx, "summary_index": 0,
                            "part": { "type": "summary_text", "text": text },
                        }),
                    ));
                    let item = json!({
                        "id": item_id, "type": "reasoning",
                        "summary": [{ "type": "summary_text", "text": text }],
                    });
                    closures.push((idx, frames, item));
                }
                let tool_closures: Vec<(String, u64, String, String, String)> =
                    self.tool_items.values().cloned().collect();
                for (item_id, idx, call_id, name, acc) in tool_closures {
                    let arguments = acc.clone();
                    let mut frames = String::new();
                    frames.push_str(&self.frame(
                        "response.function_call_arguments.done",
                        json!({
                            "type": "response.function_call_arguments.done",
                            "item_id": item_id, "output_index": idx,
                            "arguments": arguments,
                        }),
                    ));
                    let item = json!({
                        "id": item_id, "type": "function_call", "status": "completed",
                        "call_id": call_id, "name": name, "arguments": arguments,
                    });
                    closures.push((idx, frames, item));
                }
                closures.sort_by_key(|(idx, _, _)| *idx);
                for (idx, frames, item) in &closures {
                    out.push_str(frames);
                    out.push_str(&self.frame(
                        "response.output_item.done",
                        json!({
                            "type": "response.output_item.done",
                            "output_index": idx,
                            "item": item,
                        }),
                    ));
                }
                let status = if incomplete { "incomplete" } else { "completed" };
                let usage = self.usage.as_ref().map(|u| {
                    let mut j = json!({
                        "input_tokens": u.prompt_tokens.unwrap_or(0),
                        "output_tokens": u.completion_tokens.unwrap_or(0),
                        "total_tokens": u.effective_total().unwrap_or(0),
                    });
                    if let Some(cr) = u.cache_read_tokens
                        && cr > 0
                    {
                        j["input_tokens_details"] = json!({ "cached_tokens": cr });
                    }
                    j
                });
                let mut response = json!({
                    "id": self.response_id,
                    "object": "response",
                    "status": status,
                    "created_at": self.created_at,
                    "model": self.model,
                    "output": closures.into_iter().map(|(_, _, i)| i).collect::<Vec<_>>(),
                    "usage": usage,
                });
                if incomplete {
                    response["incomplete_details"] =
                        json!({ "reason": "max_output_tokens" });
                }
                let event_name = if incomplete {
                    "response.incomplete"
                } else {
                    "response.completed"
                };
                out.push_str(&self.frame(event_name, json!({ "type": event_name, "response": response })));
                Some(out)
            }
            ChatStreamEvent::Usage { usage } => {
                match &mut self.usage {
                    Some(stored) => stored.merge_partial(usage),
                    None => self.usage = Some(usage.clone()),
                }
                None
            }
            // openai wire 没有 ping 事件，无处安放；keep-alive 由网关侧自发（aidog_core::gateway::proxy::stream::with_idle_ping）。
            ChatStreamEvent::Ping => None,
        }
    }

    fn ensure_created(&mut self, model: &str) -> String {
        if self.created {
            return String::new();
        }
        if self.model.is_empty() {
            self.model = model.to_string();
        }
        self.emit_created()
    }

    /// created + in_progress 两帧（官方序列：created 之后紧跟 in_progress，同一 response 对象）。
    fn emit_created(&mut self) -> String {
        self.created = true;
        self.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let response = json!({
            "id": self.response_id,
            "object": "response",
            "status": "in_progress",
            "created_at": self.created_at,
            "model": self.model,
            "output": [],
            "usage": Value::Null,
        });
        let mut out = self.frame(
            "response.created",
            json!({ "type": "response.created", "response": response }),
        );
        out.push_str(&self.frame(
            "response.in_progress",
            json!({
                "type": "response.in_progress",
                "response": {
                    "id": self.response_id,
                    "object": "response",
                    "status": "in_progress",
                    "created_at": self.created_at,
                    "model": self.model,
                    "output": [],
                    "usage": Value::Null,
                },
            }),
        ));
        out
    }

    /// 拼一帧并递增 sequence_number。
    fn frame(&mut self, event_type: &str, mut data: Value) -> String {
        if let Some(obj) = data.as_object_mut() {
            obj.insert("sequence_number".into(), json!(self.seq));
        }
        self.seq += 1;
        format!("event: {}\ndata: {}\n\n", event_type, data)
    }
}

#[cfg(test)]
#[path = "test_sse.rs"]
mod test_sse;
