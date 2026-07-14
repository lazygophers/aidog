//! commands-cli-proxy crate —— CLI 代理 provider + platform + 导入命令（cpa-standalone-module s3）。
//!
//! 域：
//! - `provider`: cli_proxy_provider 表 CRUD（薄壳，转 `aidog_core::gateway::db::cli_proxy_*`）
//! - `test_cmd`: 临时用 provider 配置探测余额（复用 `gateway::quota::query_quota`，不落库）
//! - `platform`: 建 platform 表 cli-proxy 行（extra 存 cli_proxy_provider_id）
//! - `import`: 解析 CPA config.yaml/auth-dir/zip/dir → 批量 create provider（迁旧 cpa_import parser）
//!
//! 旧 `commands_platform::cpa_import` 保持工作（re-export shim），s4 整模块删除。

pub mod import;
pub mod platform;
pub mod provider;
pub mod test_cmd;
