//! 搜索设置页的管理命令：手动刷新 searx.space 公共实例列表（用户点按钮才拉，不自动、不定时）。
use aidog_search::searx_space::{SEARX_INSTANCES_URL, SearxInstance, fetch_searx_instances};

crate::tauri_command! {
    /// 拉一次 searx.space 实例列表（健康实例，按耗时升序），供设置页多选。
    pub async fn search_searx_instances() -> Result<Vec<SearxInstance>, String> {
        fetch_searx_instances(SEARX_INSTANCES_URL).await
    }
}
