//! CPA(CLIProxyAPI) 配置导入解析器。
//!
//! 解析 CLIProxyAPI 的 config.yaml/config.json，支持：
//! - 单文件 yaml/json
//! - 压缩包 zip/tgz/tar
//! - 文件夹递归扫描
//! - 可选 auth-dir OAuth 凭据扫描
//! - 多文件 name+base_url 去重合并

pub mod parser;

pub use parser::{parse_cpa_config, CpaOAuthType, CpaProvider, ParseResult, SkipReason};
