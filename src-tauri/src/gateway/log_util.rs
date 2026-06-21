//! 控制台 / 日志文件输出的截断 helper（与 DB 持久化解耦）。
//!
//! 两类截断，**只作用于 tracing 输出，不影响 proxy_log 等 DB 写入的完整正文**：
//! 1. `log_body_preview` —— 代理请求 / 响应等原始 body 在控制台 / 日志文件最多保留 28 字符。
//! 2. `truncate_sql_literals` —— SQL trace 日志里超长单引号字符串字面量截断。
//!
//! 所有截断按 `.chars()` 处理，防 UTF-8 字节边界 panic。

/// body 在控制台 / 日志文件的最大可见字符数（DB 持久化不受此限）。
const BODY_PREVIEW_MAX: usize = 28;

/// SQL trace 中单引号字符串字面量超过此字符数即截断。
const SQL_LITERAL_MAX: usize = 64;

/// SQL trace 单条语句整体兜底上限（防极端长 SQL 刷屏，字面量截断后仍兜底一次）。
const SQL_TOTAL_MAX: usize = 4096;

/// body 日志预览：≤28 字符原样返回；超长取前 28 字符 + 截断标记 `…[+N]`（N=剩余字符数）。
///
/// 仅供 tracing `body = %...` 使用，不得用于 DB 写入。
pub fn log_body_preview(s: &str) -> String {
    let total = s.chars().count();
    if total <= BODY_PREVIEW_MAX {
        return s.to_string();
    }
    let head: String = s.chars().take(BODY_PREVIEW_MAX).collect();
    format!("{head}…[+{}]", total - BODY_PREVIEW_MAX)
}

/// 截断 SQL 字符串中的单引号字符串字面量，超过阈值的内容替换为
/// `'<前N字符>…[truncated +M]'`（M=被截字符数），保留可见截断标记。
///
/// 实测说明：rusqlite 0.32 的 `Connection::trace` 走 legacy `sqlite3_trace`，
/// 会把预编译语句的 `?` 占位替换为实际值并内联进 SQL 文本，故此处的字面量截断
/// 对「字段值已内联」场景有效。若某版本返回未展开 `?` 的原始 SQL（值不内联），
/// 则不含长字面量，本函数自然 no-op，无副作用。
///
/// 解析按 SQLite 单引号字符串语法：`'` 开始，`''` 为转义的单引号（不结束字面量）。
/// 整条 SQL 末尾再做 `SQL_TOTAL_MAX` 字符兜底。
pub fn truncate_sql_literals(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\'' {
            out.push(c);
            continue;
        }
        // 进入单引号字面量：收集内容直到非转义的结束 '。
        out.push('\'');
        let mut lit: String = String::new();
        loop {
            match chars.next() {
                None => {
                    // 未闭合（SQL 截断/非法）：截断写出已收集内容，不补结束引号。
                    push_literal(&mut out, &lit);
                    break;
                }
                Some('\'') => {
                    // '' = 转义单引号，属字面量内容；否则字面量结束。
                    if chars.peek() == Some(&'\'') {
                        chars.next();
                        lit.push('\'');
                        lit.push('\'');
                    } else {
                        // 字面量结束：按需截断后写出 + 结束引号。
                        push_literal(&mut out, &lit);
                        out.push('\'');
                        break;
                    }
                }
                Some(other) => lit.push(other),
            }
        }
    }

    if out.chars().count() > SQL_TOTAL_MAX {
        let head: String = out.chars().take(SQL_TOTAL_MAX).collect();
        return format!("{head}…[truncated]");
    }
    out
}

/// 写出一个字面量内容，超过阈值则截断并加标记。
fn push_literal(out: &mut String, lit: &str) {
    let total = lit.chars().count();
    if total <= SQL_LITERAL_MAX {
        out.push_str(lit);
    } else {
        let head: String = lit.chars().take(SQL_LITERAL_MAX).collect();
        out.push_str(&head);
        out.push_str(&format!("…[truncated +{}]", total - SQL_LITERAL_MAX));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_preview_short_passthrough() {
        assert_eq!(log_body_preview("hello"), "hello");
        let exactly28 = "a".repeat(28);
        assert_eq!(log_body_preview(&exactly28), exactly28);
    }

    #[test]
    fn body_preview_truncates_with_marker() {
        let s = "a".repeat(40);
        assert_eq!(log_body_preview(&s), format!("{}…[+12]", "a".repeat(28)));
    }

    #[test]
    fn body_preview_utf8_no_panic() {
        let s = "中".repeat(40); // 每字 3 字节，char take 安全
        let out = log_body_preview(&s);
        assert_eq!(out, format!("{}…[+12]", "中".repeat(28)));
    }

    #[test]
    fn sql_short_literal_untouched() {
        let sql = "INSERT INTO t(a) VALUES ('short value')";
        assert_eq!(truncate_sql_literals(sql), sql);
    }

    #[test]
    fn sql_long_literal_truncated() {
        let long = "x".repeat(100);
        let sql = format!("INSERT INTO t(body) VALUES ('{long}')");
        let out = truncate_sql_literals(&sql);
        let expected_lit = format!("{}…[truncated +36]", "x".repeat(64));
        assert_eq!(out, format!("INSERT INTO t(body) VALUES ('{expected_lit}')"));
    }

    #[test]
    fn sql_escaped_quote_preserved() {
        let sql = "SELECT 'it''s ok'";
        assert_eq!(truncate_sql_literals(sql), sql);
    }

    #[test]
    fn sql_no_literal_noop() {
        let sql = "INSERT INTO t(body) VALUES (?)";
        assert_eq!(truncate_sql_literals(sql), sql);
    }
}
