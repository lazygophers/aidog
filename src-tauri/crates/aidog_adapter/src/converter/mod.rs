//! 协议转换聚合层：入站请求解析 / 请求转换（request）与响应解析 / 转换（response）。
//!
//! 拆分自原 `converter.rs`，对外 `adapter::converter::X` 路径保持不变。

mod request;
mod response;
pub mod traits;

pub use request::{convert_request, parse_incoming_request, passthrough_api_path};
pub use response::{convert_response, parse_sse, parse_upstream_sse, split_stream_inline_reasoning, to_client_sse, to_client_sse_stateful, AnthropicSseState, NonStreamResponse};
pub use traits::*;
