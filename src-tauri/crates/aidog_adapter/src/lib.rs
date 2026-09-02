//! 协议适配器模块

pub mod converter;
pub mod protocols;
pub mod quota;
pub mod reasoning_tags;
pub mod thinking;
pub mod thinking_strip;
pub mod types;

// 平台转换器模块（与 Protocol enum 平台变体一一对齐，按 enum 声明顺序排列）
pub mod aicodemirror;
pub mod aigocode;
pub mod aihubmix;
pub mod apikeyfun;
pub mod atlascloud;
pub mod bailian;
pub mod bailian_coding;
pub mod bailian_en;
pub mod byteplus;
pub mod ccsub;
pub mod cherryin;
pub mod claude_code;
pub mod claudeapi;
pub mod claudecn;
pub mod cli_proxy;
pub mod codex;
pub mod compshare;
pub mod compshare_coding;
pub mod crazyrouter;
pub mod ctok;
pub mod cubence;
pub mod deepseek;
pub mod devin;
pub mod dmxapi;
pub mod doubao;
pub mod eflowcode;
pub mod glm;
pub mod glm_coding;
pub mod glm_coding_en;
pub mod glm_en;
pub mod kimi;
pub mod kimi_coding;
pub mod kimi_en;
pub mod lemondata;
pub mod longcat;
pub mod micu;
pub mod minimax;
pub mod minimax_en;
pub mod mock;
pub mod modelscope;
pub mod newapi;
pub mod novita;
pub mod nvidia;
pub mod opencode;
pub mod opencode_zen;
pub mod openrouter;
pub mod packycode;
pub mod pateway;
pub mod pipellm;
pub mod qianfan;
pub mod qianfan_coding;
pub mod relaxycode;
pub mod rightcode;
pub mod runapi;
pub mod sensenova;
pub mod sensenova_en;
pub mod shengsuanyun;
pub mod siliconflow;
pub mod siliconflow_en;
pub mod sssaicode;
pub mod stepfun;
pub mod stepfun_en;
pub mod sudocode;
pub mod therouter;
pub mod xiaomi_mimo;
pub mod xiaomi_mimo_coding;
pub mod xiaomi_mimo_coding_en;

pub use converter::{
    AnthropicSseState, convert_request, convert_response, parse_incoming_request, parse_sse,
    parse_upstream_sse, passthrough_api_path, split_stream_inline_reasoning, to_client_sse,
    to_client_sse_stateful,
};
pub use protocols::*;
pub use reasoning_tags::InlineReasoningSplitter;
pub use thinking_strip::{SseThinkingStripper, strip_thinking_in_body};
pub use types::*;
