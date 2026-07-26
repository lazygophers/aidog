use std::collections::HashMap;
use std::sync::RwLock;

use crate::gateway::models::{Group, GroupDetail};

/// setting 缓存键的借用探测接口：让 `(&str, &str)` 与拥有所有权的 `(String, String)`
/// 共享同一套 `Hash`/`Eq` 语义，从而命中路径用借用键查 map，零 String 分配。
///
/// 标准 `HashMap<(String,String), _>::get` 要求 `Q: Borrow<(String,String)>`，
/// 而 `(String,String)` 并不 `Borrow<(&str,&str)>`，无法直接借用查找；stable Rust
/// 也没有 `raw_entry`。用 trait 对象作为 `Borrow` 目标是该场景的惯用解：owned key 与
/// borrowed key 都实现本 trait，`HashMap<(String,String)>` 借用为 `dyn KeyPair`，
/// `Hash`/`Eq` 委托到 `(scope, key)` 二元组，二者必然一致。
pub(crate) trait KeyPair {
    fn scope(&self) -> &str;
    fn key(&self) -> &str;
}

impl KeyPair for (String, String) {
    fn scope(&self) -> &str {
        &self.0
    }
    fn key(&self) -> &str {
        &self.1
    }
}

impl KeyPair for (&str, &str) {
    fn scope(&self) -> &str {
        self.0
    }
    fn key(&self) -> &str {
        self.1
    }
}

impl std::hash::Hash for dyn KeyPair + '_ {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // 必须与 `(String, String)` 的派生 Hash 字节序一致：依次 hash 两个 str。
        self.scope().hash(state);
        self.key().hash(state);
    }
}

impl PartialEq for dyn KeyPair + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.scope() == other.scope() && self.key() == other.key()
    }
}

impl Eq for dyn KeyPair + '_ {}

impl<'a> std::borrow::Borrow<dyn KeyPair + 'a> for (String, String) {
    fn borrow(&self) -> &(dyn KeyPair + 'a) {
        self
    }
}

/// 进程内热路径缓存（随 Db 实例生命周期，clone 共享同一份）。
///
/// 为什么挂在 `Db` 内而非全局 static：cargo test 单进程多线程跑，每个 test 各开一个
/// `:memory:` Db；全局缓存会跨 test 串味（test A 写 proxy/logging，test B 读到脏值）。
/// 内嵌 `Arc<RwLock<..>>` 保证「每个 Db 实例独立缓存 + clone 共享」两个性质同时成立。
#[derive(Default)]
pub(crate) struct DbCache {
    /// setting 表 (scope,key)→JSON 值缓存。`None` 槽位表示「已查过且不存在」，
    /// 用 `Option<Option<Value>>`：外层 = 是否缓存，内层 = 行是否存在。
    pub(crate) settings: RwLock<HashMap<(String, String), Option<serde_json::Value>>>,
    /// list_groups() 结果缓存（resolve_group 热路径用），写 group 表时整体失效。
    pub(crate) groups: RwLock<Option<Vec<Group>>>,
    /// list_group_details() 结果缓存（Groups 页一次拉全量用）。
    ///
    /// 内嵌完整 GroupDetail（含 platform 易变字段：est_balance_remaining / status /
    /// auto_disabled_until / last_real_query_at 等），故须**写时全失效**：任何 group /
    /// group_platform 结构写、platform create/update/delete、以及 estimate/breaker 对
    /// platform 易变列的写都失效（宁全勿漏，见 invalidate_group_details_cache 调用点）。
    ///
    /// 关键：list_group_details **不在代理 resolve 热路径**（proxy/router 走
    /// get_group_platforms 直查单组），故 estimate.rs 每请求级写带来的频繁失效只代价
    /// 「下次 Groups 页打开重建一次」，不影响代理吞吐。
    pub(crate) group_details: RwLock<Option<Vec<GroupDetail>>>,
}
