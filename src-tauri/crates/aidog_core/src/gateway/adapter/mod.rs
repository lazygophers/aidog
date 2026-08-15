//! 协议适配器模块

pub mod converter;
pub mod protocols;
pub mod types;

// 平台转换器模块（按平台维度组织）
pub mod glm;
pub mod kimi;
pub mod kimi_coding;
pub mod minimax;
pub mod minimax_en;
pub mod deepseek;
pub mod stepfun;
pub mod stepfun_en;
pub mod doubao;
pub mod byteplus;
pub mod qianfan;
pub mod xiaomi_mimo;
pub mod bailian;
pub mod longcat;
pub mod sensenova;
pub mod siliconflow;
pub mod siliconflow_en;
pub mod aihubmix;

pub use converter::{convert_request, convert_response, parse_sse, parse_upstream_sse, parse_incoming_request, passthrough_api_path, to_client_sse};
pub use protocols::*;
pub use types::*;
