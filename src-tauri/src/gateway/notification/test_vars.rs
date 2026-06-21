use super::*;
use std::collections::HashMap;

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
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
    assert_eq!(substitute_vars("项目 {project} 完成", &v), "项目 aidog 完成");
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
