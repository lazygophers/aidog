#![cfg(test)]
use super::*;

/// fmt_caller 取路径末段 + 行号，紧凑显示。
#[test]
fn fmt_caller_uses_basename_and_line() {
    let loc = std::panic::Location::caller(); // 本测试函数所在位置
    let out = fmt_caller(loc);
    // 形如 "<basename>.rs:<line>"：仅文件名末段（无路径分隔符）+ 冒号 + 数字行号。
    assert!(!out.contains('/') && !out.contains('\\'), "应只含文件名末段, got {out}");
    assert!(out.contains(".rs:"), "got {out}");
    assert!(out.rsplit(':').next().unwrap().parse::<u32>().is_ok(), "got {out}");
}

/// 空上下文（无 call_traced 设置）→ profile 回调取值应回退为 "-"。
#[test]
fn empty_ctx_renders_dash() {
    CURRENT_DB_CTX.with(|c| *c.borrow_mut() = DbCallCtx::default());
    let (req, caller) = CURRENT_DB_CTX.with(|c| {
        let c = c.borrow();
        (
            c.req.clone().unwrap_or_else(|| "-".to_string()),
            c.caller.map(fmt_caller).unwrap_or_else(|| "-".to_string()),
        )
    });
    assert_eq!(req, "-");
    assert_eq!(caller, "-");
}
