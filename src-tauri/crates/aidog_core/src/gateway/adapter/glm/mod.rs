//! GLM 平台转换器
//!
//! 处理各种协议格式 ↔ GLM 平台格式的转换

pub mod openai_chat;
pub mod openai_responses;
pub mod openai_completions;

pub use openai_chat::*;
pub use openai_responses::*;
pub use openai_completions::*;
