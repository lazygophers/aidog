#![cfg(test)]
use super::*;
use crate::test_support::three_file_db;

#[test]
fn table_has_eight_rows_and_cny_tracks_billing_const() {
    assert_eq!(CURRENCIES.len(), 8);
    assert_eq!(currency_def("CNY").unwrap().default_rate, CNY_PER_USD);
    assert_eq!(currency_def("USD").unwrap().default_rate, 1.0);
    assert!(currency_def("XXX").is_none());
}

#[test]
fn golden_cases_match() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("currency_golden.json")).unwrap();
    assert_eq!(cases.len(), 56);
    for c in cases {
        let code = c["code"].as_str().unwrap();
        let def = currency_def(code).unwrap();
        // golden 自带 symbol/decimals/rate，同时与币种表互证
        assert_eq!(c["symbol"].as_str().unwrap(), def.symbol);
        assert_eq!(c["decimals"].as_u64().unwrap() as usize, def.decimals);
        assert_eq!(c["rate"].as_f64().unwrap(), def.default_rate);
        let got = format_money(c["usd"].as_f64().unwrap(), def.symbol, def.decimals, def.default_rate);
        assert_eq!(got, c["expected"].as_str().unwrap(), "{code} {}", c["usd"]);
    }
}

#[test]
fn format_money_nan_and_nonpositive_are_zero() {
    assert_eq!(format_money(f64::NAN, "$", 2, 1.0), "$0");
    assert_eq!(format_money(0.0, "¥", 2, 6.6675), "¥0");
    assert_eq!(format_money(-3.0, "$", 2, 1.0), "$0");
}

#[tokio::test]
async fn get_defaults_to_usd() {
    let (db, _dir) = three_file_db().await;
    let c = get_display_currency(&db).await;
    assert_eq!((c.code.as_str(), c.symbol.as_str(), c.rate), ("USD", "$", 1.0));
    assert_eq!(c.table.len(), 8);
}

#[tokio::test]
async fn set_persists_and_overrides_survive_reopen() {
    let (db, dir) = three_file_db().await;
    let mut rates = BTreeMap::new();
    rates.insert("CNY".to_string(), 7.0);
    let r = set_display_currency(&db, "CNY", rates).await.unwrap();
    assert_eq!((r.code.as_str(), r.rate), ("CNY", 7.0));
    drop(db);
    let db2 = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    let c = get_display_currency(&db2).await;
    assert_eq!((c.code.as_str(), c.symbol.as_str(), c.rate), ("CNY", "¥", 7.0));
    let eur = c.table.iter().find(|r| r.code == "EUR").unwrap();
    assert_eq!(eur.rate, eur.default_rate); // 未覆盖项回落默认
    let cny = c.table.iter().find(|r| r.code == "CNY").unwrap();
    assert_eq!((cny.rate, cny.default_rate), (7.0, CNY_PER_USD));
}

#[tokio::test]
async fn set_rejects_invalid_input_and_writes_nothing() {
    let (db, _dir) = three_file_db().await;
    let rate = |k: &str, v: f64| BTreeMap::from([(k.to_string(), v)]);
    assert!(set_display_currency(&db, "XXX", BTreeMap::new()).await.is_err());
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(set_display_currency(&db, "CNY", rate("CNY", bad)).await.is_err(), "{bad}");
    }
    assert!(set_display_currency(&db, "USD", rate("USD", 2.0)).await.is_err());
    assert!(set_display_currency(&db, "USD", rate("XXX", 2.0)).await.is_err());
    assert_eq!(get_display_currency(&db).await.code, "USD");
}

#[test]
fn balance_to_usd_cny_uses_table_rate_others_pass_through() {
    let cur = build_display_currency(&DisplayCurrencySetting::default());
    assert!((cur.balance_to_usd(66.675, Some("CNY")) - 10.0).abs() < 1e-9);
    assert_eq!(cur.balance_to_usd(10.0, Some("USD")), 10.0);
    assert_eq!(cur.balance_to_usd(10.0, None), 10.0);
    // 未知 / 非 CNY 币种按 USD 处理
    assert_eq!(cur.balance_to_usd(10.0, Some("EUR")), 10.0);
    // 用户改了 CNY 汇率：CNY 余额回除生效汇率，显示 CNY 时恒等于原值
    let mut s = DisplayCurrencySetting { code: "CNY".into(), ..Default::default() };
    s.rates.insert("CNY".into(), 7.0);
    let c = build_display_currency(&s);
    assert!((c.balance_to_usd(70.0, Some("cny")) * c.rate - 70.0).abs() < 1e-9);
}

#[tokio::test]
async fn balance_currencies_roundtrip_from_setting() {
    let (db, _dir) = three_file_db().await;
    for (k, v) in [("7", "CNY"), ("9", "USD"), ("x", "CNY")] {
        crate::set_setting(
            &db,
            SetSettingInput {
                scope: BALANCE_CCY_SCOPE.into(),
                key: k.into(),
                value: serde_json::Value::String(v.into()),
            },
        )
        .await
        .unwrap();
    }
    let m = get_balance_currencies(&db).await;
    assert_eq!(m.len(), 2); // 非数字 key 跳过
    assert_eq!(m[&7], "CNY");
    assert_eq!(m[&9], "USD");
}
