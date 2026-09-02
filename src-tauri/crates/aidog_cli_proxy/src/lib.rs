//! aidog_cli_proxy：CLIProxyAPI（CPA）域——配置解析（parser/archive）+ provider/平台
pub mod archive;
pub mod batch;
pub mod import;
/// 命令（batch/import/platform/provider，拆自 aidog_core::gateway::cli_proxy_parser 与
/// aidog_core::cli_proxy_cmd，2026-08-16）。
pub mod parser;
pub mod platform;
pub mod provider;

pub mod test_cmd;
