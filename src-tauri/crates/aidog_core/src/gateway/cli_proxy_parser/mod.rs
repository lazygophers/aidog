//! CLI 代理配置导入解析器（cpa-standalone-module s3 迁出 cpa_import）。
//!
//! 解析 CLIProxyAPI 的 config.yaml/config.json/auth-dir/zip/dir 为中间表示 `CpaProvider`。
//! 新 `commands_cli_proxy` crate 消费；旧 `cpa_import` mapper 经 re-export 保持工作（s4 删旧模块）。
//! parser 本身段无关，映射（segment → wire_protocol / Protocol）在 caller 侧（见
//! commands_cli_proxy::import 与 cpa_import::mapper）。

pub mod parser;
pub use parser::{parse_cpa_config, CpaOAuthType, CpaProvider, CpaSourceSegment, ParseResult, SkipReason};
