//! 协议转换器模块

pub mod anthropic;
pub mod gemini;
pub mod mock;
pub mod openai;
pub mod openai_completions;
pub mod openai_responses;

pub use anthropic::strip_unsigned_thinking_blocks;
