//! aidog_logs：proxy_log 表读写 crate（拆自 aidog_core::gateway::db::proxy_log）。
//! aidog_core 直接以 `aidog_logs::upsert_proxy_log` 等路径消费。

mod proxy_log;
#[cfg(test)]
mod test_proxy_log;

pub use proxy_log::*;
