//! aidog_stats：统计读写 crate（query_stats / stats_agg / stats_today / usage_stats，
//! 拆自 aidog_core::gateway::db）。aidog_core 经 `gateway::db` re-export 保持旧路径。

mod query_stats;
mod stats_agg;
mod stats_today;
mod usage_stats;
#[cfg(test)]
mod test_query_stats;
#[cfg(test)]
mod test_stats_agg;
#[cfg(test)]
mod test_stats_today;
#[cfg(test)]
mod test_usage_stats;

pub use query_stats::*;
pub use stats_agg::*;
pub use stats_today::*;
pub use usage_stats::*;

/// `db.init_tables()` 便捷封装。跨 crate 禁止给 `Db` 加 inherent impl（E0116），
/// 用 extension trait 复刻旧调用形态；backfill 注入 aidog_stats 自身的
/// `backfill_stats_agg_if_empty`（aidog_db 反向依赖会成环，故走函数注入）。
pub trait DbInitTables {
    fn init_tables(&self) -> impl std::future::Future<Output = Result<(), String>> + '_;
}

impl DbInitTables for aidog_db::Db {
    fn init_tables(&self) -> impl std::future::Future<Output = Result<(), String>> + '_ {
        aidog_db::schema::init_tables_raw(self, std::sync::Arc::new(backfill_stats_agg_if_empty))
    }
}
