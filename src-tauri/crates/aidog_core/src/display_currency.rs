//! 显示币种命令（币种表与存取在 `aidog_db::currency`）。
//! set 成功后经 `aidog_ctx` 广播 [`aidog_db::currency::DISPLAY_CURRENCY_EVENT`]，
//! 前端 store / popover 订阅刷新；同时 emit tray-refresh 重画托盘。

use aidog_db::currency::{self, DisplayCurrency};
use std::collections::BTreeMap;

crate::tauri_command! {
    /// 当前显示币种 + 8 币种表（含生效汇率）。
    pub async fn display_currency_get() -> DisplayCurrency {
        currency::get_display_currency(aidog_ctx::db()).await
    }
}

crate::tauri_command! {
    /// 设置显示币种与汇率覆盖项（整体替换；缺项 = 恢复默认）。
    pub async fn display_currency_set(
        code: String,
        rates: BTreeMap<String, f64>,
    ) -> Result<DisplayCurrency, String> {
        let r = currency::set_display_currency(aidog_ctx::db(), &code, rates).await?;
        aidog_ctx::emit_unit(currency::DISPLAY_CURRENCY_EVENT);
        // 托盘重画：复用 app_setup.rs:622 的 tray-refresh listener
        aidog_ctx::emit_unit("tray-refresh");
        Ok(r)
    }
}
