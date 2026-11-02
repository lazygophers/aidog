use super::*;
use std::collections::HashMap;

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn substitute_known_and_unknown() {
    let v = vars(&[("project", "aidog"), ("status", "done")]);
    assert_eq!(substitute_vars("{project} {status}", &v), "aidog done");
    // 未知占位保留
    assert_eq!(substitute_vars("{project} {time}", &v), "aidog {time}");
    // 无占位
    assert_eq!(substitute_vars("plain text", &v), "plain text");
    // 非法键（含空格）原样保留
    assert_eq!(substitute_vars("{not a key}", &v), "{not a key}");
    // 孤立 {
    assert_eq!(substitute_vars("a { b", &v), "a { b");
    // 多字节中文不被切断
    assert_eq!(
        substitute_vars("项目 {project} 完成", &v),
        "项目 aidog 完成"
    );
}

#[test]
fn substitute_all_vars() {
    let v = vars(&[
        ("project", "p"),
        ("status", "s"),
        ("time", "t"),
        ("session", "se"),
        ("group", "g"),
    ]);
    assert_eq!(
        substitute_vars("{project}/{status}/{time}/{session}/{group}", &v),
        "p/s/t/se/g"
    );
}

#[test]
fn optional_prefix_placeholder() {
    let named = vars(&[("project", "aidog"), ("session_name", "托盘重做")]);
    let empty = vars(&[("project", "aidog"), ("session_name", "")]);
    let missing = vars(&[("project", "aidog")]);
    let tpl = "{project}{ · |session_name} 完成";
    for f in [substitute_vars, substitute_vars_fill_empty] {
        assert_eq!(f(tpl, &named), "aidog · 托盘重做 完成");
        assert_eq!(f(tpl, &empty), "aidog 完成");
        // 类型路径也不保留缺失的可选占位
        assert_eq!(f(tpl, &missing), "aidog 完成");
    }
    // 普通占位语义不变：类型路径保留未知，事件路径置空
    assert_eq!(substitute_vars("{x}", &missing), "{x}");
    assert_eq!(substitute_vars_fill_empty("{x}", &missing), "");
    // 前缀含空格 / 与普通占位混用 / 多个可选占位
    let v = vars(&[("a", "1"), ("b", "")]);
    assert_eq!(substitute_vars("[{ - |a}][{ - |b}]{a}", &v), "[ - 1][]1");
    // 变量名非法 → 原样；前缀含 '{' → 内层 '{' 不被吞
    assert_eq!(substitute_vars("{p|bad name}", &v), "{p|bad name}");
    assert_eq!(substitute_vars("{{ -|a}", &v), "{ -1");
}
