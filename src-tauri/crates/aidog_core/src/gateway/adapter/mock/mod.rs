//! Mock 平台类型：本地生成可控假响应，不转发真实上游。
//!
//! 配置三层覆盖（逐字段，优先级高 → 低）：
//! 1. 请求 body 顶层 `mock` 对象
//! 2. 请求 messages 的 role 映射（role 当 key，content 当 value）
//! 3. platform.extra JSON 的 `mock` 对象（兜底默认）

mod config;
mod response;
mod stream;

pub use config::resolve_mock_config;
#[allow(unused_imports)]
pub use config::MockConfig;
pub use response::{build_error_body, build_response};
pub use stream::build_sse_chunks;
