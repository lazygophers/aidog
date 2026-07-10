//! 请求驱动 quota 预估增量更新（降频）
//!
//! 每次 proxy 请求完成（拿到 token）→ 本地预估增量更新平台余额 + coding plan，
//! 降低对上游 quota API 的查询频率，并在 5min / 100 次时触发真实校准覆盖。
//!
//! 关键约束（见 research）：
//!   - `Db.0` 是 `std::sync::Mutex<Connection>`，**禁持锁跨 .await**；
//!     校准里的 `query_quota` 是 async，须 锁外调用，结果回库时再短持锁。
//!   - 余额预估用**单条 SQL 原子自减**避免多请求并发丢更新。
//!   - coding plan 是 JSON 字段无法 SQL 内自增 → read-modify-write 必须在
//!     同一持锁临界区内完成（一次 lock 内 SELECT+UPDATE）。
//!
//! 子模块划分：
//!   - `model`   — 预估 JSON 模型（EstCodingPlan/EstTier）+ 校准阈值常量
//!   - `algo`    — 纯算法（增量预估 / pace 配色 / 真查拟合）
//!   - `db_ops`  — DB 集成（原子自减 / read-modify-write / 校准覆盖 / 请求后入口）

mod algo;
mod db_ops;
mod model;

// 对外路径保持 `gateway::estimate::X` 不变（部分项仅测试 / 备用，allow 保稳定 API）。
#[allow(unused_imports)]
pub use algo::{
    apply_tier_delta, balance_cost, calibrate_tier, should_calibrate, tier_pace, tier_usage_level,
    TierPace,
};
#[allow(unused_imports)]
pub use db_ops::{
    apply_balance_delta, apply_coding_plan_delta, build_calibrated_coding_plan, calibrate_from_quota,
    estimate_after_request, read_estimate_state, write_real_quota,
};
#[allow(unused_imports)]
pub use model::{EstCodingPlan, EstTier, CALIBRATE_COUNT, CALIBRATE_INTERVAL_MS};
