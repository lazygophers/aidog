//! aidog_logs：proxy_log 表读写 crate（拆自 aidog_core::gateway::db::proxy_log）。
//! aidog_core 经 `gateway::db` re-export 保持 `crate::gateway::db::upsert_proxy_log` 等旧路径。

mod proxy_log;
#[cfg(test)]
mod test_proxy_log;

pub use proxy_log::*;
