//! 显示币种：币种表（唯一真值源）+ 设置存取 + USD→显示字符串换算。
//!
//! 存储（est_cost 等）全程 USD，本模块只管显示层。前端 `utils/formatters.ts::formatMoney`
//! 与托盘共用同一公式，对拍数据见 `currency_golden.json`。
//!
//! 设置：setting 表（主库）scope `display` key `currency`，值 `{ code, rates }`，
//! `rates` 只存用户改过的覆盖项，缺省回落内置默认。

use crate::models::SetSettingInput;
use crate::{Db, CNY_PER_USD};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// setting 表 scope / key。
pub const DISPLAY_SCOPE: &str = "display";
pub const DISPLAY_KEY: &str = "currency";
/// `display_currency_set` 成功后广播的事件名（前端 store / popover / 托盘订阅）。
pub const DISPLAY_CURRENCY_EVENT: &str = "display-currency-updated";

/// 币种定义。
pub struct CurrencyDef {
    pub code: &'static str,
    pub symbol: &'static str,
    /// 换算后 ≥ 1 时的小数位。
    pub decimals: usize,
    /// 默认汇率：1 USD = x 该币种。
    pub default_rate: f64,
}

/// 8 币种表。默认汇率快照来源（到期不自动更新）：
/// - EUR/JPY/GBP/HKD/CHF：ECB eurofxref-daily.xml 2026-10-29 参考汇率换算为 1 USD 兑；
/// - CNY：沿用计费常量 [`CNY_PER_USD`]；
/// - RUB：俄央行 XML_daily 2026-10-30 生效价（取数日 2026-10-29）；ECB 自 2022-03-01 起停发 RUB。
pub const CURRENCIES: [CurrencyDef; 8] = [
    CurrencyDef { code: "USD", symbol: "$", decimals: 2, default_rate: 1.0 },
    CurrencyDef { code: "CNY", symbol: "¥", decimals: 2, default_rate: CNY_PER_USD },
    CurrencyDef { code: "EUR", symbol: "€", decimals: 2, default_rate: 0.8985 },
    CurrencyDef { code: "JPY", symbol: "JP¥", decimals: 0, default_rate: 158.06 },
    CurrencyDef { code: "GBP", symbol: "£", decimals: 2, default_rate: 0.7509 },
    CurrencyDef { code: "HKD", symbol: "HK$", decimals: 2, default_rate: 7.8464 },
    CurrencyDef { code: "CHF", symbol: "CHF ", decimals: 2, default_rate: 0.8286 },
    CurrencyDef { code: "RUB", symbol: "₽", decimals: 2, default_rate: 80.6761 },
];

pub fn currency_def(code: &str) -> Option<&'static CurrencyDef> {
    CURRENCIES.iter().find(|c| c.code == code)
}

/// 持久化形状。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DisplayCurrencySetting {
    #[serde(default)]
    pub code: String,
    /// 用户覆盖汇率（仅改过的项）。
    #[serde(default)]
    pub rates: BTreeMap<String, f64>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CurrencyRow {
    pub code: String,
    pub symbol: String,
    pub decimals: usize,
    pub default_rate: f64,
    /// 生效汇率（覆盖项优先，缺省 = default_rate）。
    pub rate: f64,
}

/// `display_currency_get` 返回：当前币种 + 全表。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct DisplayCurrency {
    pub code: String,
    pub symbol: String,
    pub decimals: usize,
    pub rate: f64,
    pub table: Vec<CurrencyRow>,
}

/// 平台余额币种（真查 `BalanceInfo.currency`）的 setting scope，key = 平台 id。
pub const BALANCE_CCY_SCOPE: &str = "balance_currency";

impl DisplayCurrency {
    /// 平台余额（原币种，`est_balance_remaining` 口径）→ USD：CNY 按币种表生效 CNY 汇率回除
    /// （默认即 `CNY_PER_USD`，用户改了表则 CNY 余额显示 CNY 时仍等于原值）；其余 / 未记录一律视 USD。
    /// shortcut: 仅识别 CNY（与 `balance_fx` 扣减口径一致），其它币种按 USD 处理；出现新币种时两处同加。
    pub fn balance_to_usd(&self, amount: f64, ccy: Option<&str>) -> f64 {
        match ccy {
            Some(c) if c.trim().eq_ignore_ascii_case("CNY") => {
                let rate = self.table.iter().find(|r| r.code == "CNY").map_or(1.0, |r| r.rate);
                amount / rate
            }
            _ => amount,
        }
    }
}

/// 全部平台的余额币种（平台 id → 大写币种码）。读失败 / 非字符串值跳过。
pub async fn get_balance_currencies(db: &Db) -> HashMap<u64, String> {
    let mut out = HashMap::new();
    for key in crate::list_setting_keys(db, BALANCE_CCY_SCOPE).await.unwrap_or_default() {
        let Ok(id) = key.parse::<u64>() else { continue };
        if let Ok(Some(serde_json::Value::String(c))) =
            crate::get_setting(db, BALANCE_CCY_SCOPE, &key).await
        {
            out.insert(id, c);
        }
    }
    out
}

fn effective_rate(def: &CurrencyDef, rates: &BTreeMap<String, f64>) -> f64 {
    if def.code == "USD" {
        return 1.0;
    }
    rates
        .get(def.code)
        .copied()
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(def.default_rate)
}

/// 由持久化设置组装对外视图。未知 code 回落 USD。
pub fn build_display_currency(s: &DisplayCurrencySetting) -> DisplayCurrency {
    let cur = currency_def(&s.code).unwrap_or(&CURRENCIES[0]);
    DisplayCurrency {
        code: cur.code.to_string(),
        symbol: cur.symbol.to_string(),
        decimals: cur.decimals,
        rate: effective_rate(cur, &s.rates),
        table: CURRENCIES
            .iter()
            .map(|d| CurrencyRow {
                code: d.code.to_string(),
                symbol: d.symbol.to_string(),
                decimals: d.decimals,
                default_rate: d.default_rate,
                rate: effective_rate(d, &s.rates),
            })
            .collect(),
    }
}

/// 读显示币种。未设置 / 解析失败 → USD。
pub async fn get_display_currency(db: &Db) -> DisplayCurrency {
    let s = match crate::get_setting(db, DISPLAY_SCOPE, DISPLAY_KEY).await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => DisplayCurrencySetting::default(),
    };
    build_display_currency(&s)
}

/// 校验并写入。`rates` 整体替换覆盖项（缺项 = 恢复默认）。
pub async fn set_display_currency(
    db: &Db,
    code: &str,
    rates: BTreeMap<String, f64>,
) -> Result<DisplayCurrency, String> {
    if currency_def(code).is_none() {
        return Err(format!("unsupported currency code: {code}"));
    }
    for (k, r) in &rates {
        if currency_def(k).is_none() {
            return Err(format!("unsupported currency code in rates: {k}"));
        }
        if k == "USD" {
            return Err("USD rate is fixed at 1".to_string());
        }
        if !r.is_finite() || *r <= 0.0 {
            return Err(format!("invalid rate for {k}: must be finite and > 0"));
        }
    }
    let setting = DisplayCurrencySetting { code: code.to_string(), rates };
    let value = serde_json::to_value(&setting).map_err(|e| format!("serialize currency: {e}"))?;
    crate::set_setting(
        db,
        SetSettingInput {
            scope: DISPLAY_SCOPE.to_string(),
            key: DISPLAY_KEY.to_string(),
            value,
        },
    )
    .await?;
    Ok(build_display_currency(&setting))
}

/// 整数部分加千分位逗号（仅 0 位小数币种使用，保持其余币种与旧 formatCost 逐字一致）。
fn group_thousands(s: &str) -> String {
    let (int, frac) = s.split_once('.').map_or((s, None), |(i, f)| (i, Some(f)));
    let mut out = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

/// USD 金额 → 显示字符串（符号前置）。`shown = usd * rate`，量级规则在换算后判定：
/// 非正/NaN → "0"；≥1 → `decimals` 位（0 位币种加千分位）；≥0.01 → 3 位；
/// 更小 → 2 位有效数字定点（小数位下限 5 上限 12，不用科学计数）。
pub fn format_money(usd: f64, symbol: &str, decimals: usize, rate: f64) -> String {
    let n = usd * rate;
    let body = if n.is_nan() || n <= 0.0 {
        "0".to_string()
    } else if n >= 1.0 {
        let s = format!("{n:.decimals$}");
        if decimals == 0 { group_thousands(&s) } else { s }
    } else if n >= 0.01 {
        format!("{n:.3}")
    } else {
        let d = ((-n.log10().floor() as i64) + 1).clamp(5, 12) as usize;
        format!("{n:.d$}")
    };
    format!("{symbol}{body}")
}

#[cfg(test)]
#[path = "test_currency.rs"]
mod test_currency;
