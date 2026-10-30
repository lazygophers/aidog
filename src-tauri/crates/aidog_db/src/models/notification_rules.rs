//! 规则引擎与预置规则（通知体系重设计票 02 建于 aidog_notification，票 04 迁入
//! aidog_db models：规则持久化在 NotificationSettings + TS 导出同处，aidog_notification
//! 保路径兼容 re-export）。
//!
//! 规则 = { 开关, 匹配（事件类型 + 可选条件表达式）, 动作（渠道集合 + 免打扰穿透 + 去重窗口）}。
//! 表达式 DSL：**自写**（非 rhai）——语法只有「字段比较 + &&/|| + 括号」，递归下降解析，
//! 编译期校验并报错定位（字符偏移）；rhai 是全功能脚本引擎，超出受限 DSL 需求。

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::notif_type_for_event;

// ── 规则模型 ────────────────────────────────────────────────────────────────

/// 动作渠道：本地 4 形态 + 外部 7 渠道（票 03；外部渠道配置在 models::ExternalChannels）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Popup,
    Inbox,
    Sound,
    Tts,
    // 外部渠道（票 03；配置见 models::ExternalChannels，无配置 = 该渠道旁路）
    Webhook,
    Telegram,
    Bark,
    Discord,
    Slack,
    Email,
    Feishu,
}

/// 规则动作：命中后的投递动作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct RuleAction {
    /// 投递渠道集合（空 = 匹配但静默，用于「先关掉某类」的用户规则）。
    pub channels: Vec<Channel>,
    /// 免打扰穿透（票 04 的免打扰总闸下仍投递）。
    #[serde(default)]
    pub bypass_dnd: bool,
    /// 去重窗口秒（同一 dedup_key 在窗口内只投一次；0 = 不去重）。
    #[serde(default)]
    pub dedup_window_secs: u32,
}

/// 一条通知规则。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// 系统级预置规则：只可开关 / 复制，不可改删。
    #[serde(default)]
    pub preset: bool,
    /// 匹配的事件类型（语义类型或透传事件名）；None = 任意类型。
    #[serde(default)]
    pub match_type: Option<String>,
    /// 条件表达式（受限 DSL）；None = 无附加条件。
    #[serde(default)]
    pub condition: Option<String>,
    pub action: RuleAction,
}

/// 匹配输入（Envelope 的最小投影；vars 全字符串化）。
#[derive(Debug, Clone)]
pub struct MatchInput<'a> {
    pub typ: &'a str,
    pub severity: &'a str,
    pub vars: &'a HashMap<String, String>,
}

// ── 表达式 DSL ──────────────────────────────────────────────────────────────
//
// expr        := or
// or          := and ( "||" and )*
// and         := atom ( "&&" atom )*
// atom        := "(" expr ")" | comparison
// comparison  := field op value
// field       := ident ("." ident)*      // type / severity / vars.<name>
// op          := "==" | "!=" | "contains"
// value       := "字符串" | 数字 | 裸词（按字符串比较）

/// 编译错误：带字符偏移定位（UI 据此高亮；报错文案形如「第 N 字符附近：…」）。
#[derive(Debug, Clone, PartialEq)]
pub struct ExprError {
    pub pos: usize,
    pub msg: String,
}

impl std::fmt::Display for ExprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "第 {} 字符附近：{}", self.pos, self.msg)
    }
}
impl std::error::Error for ExprError {}

/// 编译后的表达式（AST）。解析产物，eval 无 panic。
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Cmp {
        field: String,
        op: CmpOp,
        value: String,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Contains,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),   // 裸词：字段名 / 值
    Str(String),     // "..." 字面量
    Num(String),     // 数字字面量（按字符串比较，保留原样）
    Op(String),      // == != contains && || ( )
    Eof,
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, bytes: src.as_bytes(), pos: 0 }
    }

    fn err(&self, pos: usize, msg: &str) -> ExprError {
        ExprError { pos, msg: msg.to_string() }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn next(&mut self) -> Result<(Tok, usize), ExprError> {
        self.skip_ws();
        let start = self.pos;
        if self.pos >= self.bytes.len() {
            return Ok((Tok::Eof, start));
        }
        let b = self.bytes[self.pos];
        // 字符串字面量（仅双引号，禁转义——配置值里不会出现引号内的引号）
        if b == b'"' {
            let rest = &self.src[self.pos + 1..];
            match rest.find('"') {
                Some(end) => {
                    let s = rest[..end].to_string();
                    self.pos += end + 2;
                    return Ok((Tok::Str(s), start));
                }
                None => return Err(self.err(start, "字符串缺少收尾双引号")),
            }
        }
        // 多字符符号
        for sym in ["==", "!=", "&&", "||"] {
            if self.src[self.pos..].starts_with(sym) {
                self.pos += sym.len();
                return Ok((Tok::Op(sym.to_string()), start));
            }
        }
        if b == b'(' || b == b')' {
            self.pos += 1;
            return Ok((Tok::Op((b as char).to_string()), start));
        }
        if b == b'!' || b == b'=' || b == b'&' || b == b'|' {
            return Err(self.err(start, "非法符号（可用：== != contains && || 括号）"));
        }
        // 裸词（字母/数字/下划线/点/连字符；含中文）
        let mut end = self.pos;
        while end < self.bytes.len() {
            let c = self.bytes[end];
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'-' || c >= 0x80 {
                end += 1;
            } else {
                break;
            }
        }
        if end == self.pos {
            return Err(self.err(start, "无法识别的字符"));
        }
        let word = self.src[self.pos..end].to_string();
        self.pos = end;
        let tok = if word.starts_with(|c: char| c.is_ascii_digit()) {
            Tok::Num(word)
        } else if word == "contains" {
            Tok::Op("contains".to_string())
        } else {
            Tok::Ident(word)
        };
        Ok((tok, start))
    }
}

struct Parser<'a> {
    lex: Lexer<'a>,
    peeked: Option<Result<(Tok, usize), ExprError>>,
}

impl<'a> Parser<'a> {
    fn peek(&mut self) -> Result<&(Tok, usize), ExprError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lex.next());
        }
        self.peeked.as_ref().unwrap().as_ref().map_err(|e| e.clone())
    }

    fn bump(&mut self) -> Result<(Tok, usize), ExprError> {
        match self.peeked.take() {
            Some(r) => r,
            None => self.lex.next(),
        }
    }

    fn parse(mut self) -> Result<Expr, ExprError> {
        let e = self.parse_or()?;
        match self.peek()? {
            (Tok::Eof, _) => Ok(e),
            (t, pos) => Err(ExprError { pos: *pos, msg: format!("多余的内容：{t:?}") }),
        }
    }

    fn parse_or(&mut self) -> Result<Expr, ExprError> {
        let mut lhs = self.parse_and()?;
        loop {
            if let (Tok::Op(op), _) = self.peek()?
                && op == "||"
            {
                self.bump()?;
                let rhs = self.parse_and()?;
                lhs = Expr::Or(Box::new(lhs), Box::new(rhs));
                continue;
            }
            return Ok(lhs);
        }
    }

    fn parse_and(&mut self) -> Result<Expr, ExprError> {
        let mut lhs = self.parse_atom()?;
        loop {
            if let (Tok::Op(op), _) = self.peek()?
                && op == "&&"
            {
                self.bump()?;
                let rhs = self.parse_atom()?;
                lhs = Expr::And(Box::new(lhs), Box::new(rhs));
                continue;
            }
            return Ok(lhs);
        }
    }

    fn parse_atom(&mut self) -> Result<Expr, ExprError> {
        match self.peek()?.clone() {
            (Tok::Op(op), _pos) if op == "(" => {
                self.bump()?;
                let inner = self.parse_or()?;
                match self.bump()? {
                    (Tok::Op(c), _) if c == ")" => Ok(inner),
                    (t, p) => Err(ExprError { pos: p, msg: format!("期望右括号，收到 {t:?}") }),
                }
            }
            (Tok::Ident(_), _) => self.parse_cmp(),
            (t, pos) => Err(ExprError { pos, msg: format!("期望字段名或左括号，收到 {t:?}") }),
        }
    }

    fn parse_cmp(&mut self) -> Result<Expr, ExprError> {
        let (field, _pos) = self.bump()?; // Ident（parse_atom 已保证）
        let field = match field {
            Tok::Ident(s) => s,
            _ => unreachable!(),
        };
        let (op_tok, op_pos) = self.bump()?;
        let op = match op_tok {
            Tok::Op(s) if s == "==" => CmpOp::Eq,
            Tok::Op(s) if s == "!=" => CmpOp::Ne,
            Tok::Op(s) if s == "contains" => CmpOp::Contains,
            t => {
                return Err(ExprError {
                    pos: op_pos,
                    msg: format!("期望比较符（== != contains），收到 {t:?}"),
                })
            }
        };
        let (val, val_pos) = self.bump()?;
        let value = match val {
            Tok::Str(s) | Tok::Num(s) | Tok::Ident(s) => s,
            t => {
                return Err(ExprError {
                    pos: val_pos,
                    msg: format!("期望值（字符串/数字/裸词），收到 {t:?}"),
                })
            }
        };
        Ok(Expr::Cmp { field, op, value })
    }
}

/// 编译表达式（每次匹配前调；调用方可缓存）。
pub fn compile_expr(src: &str) -> Result<Expr, ExprError> {
    if src.trim().is_empty() {
        return Err(ExprError { pos: 0, msg: "表达式为空".to_string() });
    }
    Parser { lex: Lexer::new(src), peeked: None }.parse()
}

/// 求值。字段解析：`type` → 事件类型；`severity` → 级别；`vars.<name>` → 变量。
/// 未知字段 / 缺失变量 → 空串（与 substitute_vars_fill_empty 同口径，不报错）。
pub fn eval_expr(e: &Expr, input: &MatchInput) -> bool {
    match e {
        Expr::And(a, b) => eval_expr(a, input) && eval_expr(b, input),
        Expr::Or(a, b) => eval_expr(a, input) || eval_expr(b, input),
        Expr::Cmp { field, op, value } => {
            let lhs = resolve_field(field, input);
            match op {
                CmpOp::Eq => lhs == value,
                CmpOp::Ne => lhs != value,
                CmpOp::Contains => lhs.contains(value.as_str()),
            }
        }
    }
}

fn resolve_field<'a>(field: &str, input: &'a MatchInput) -> &'a str {
    if field == "type" {
        return input.typ;
    }
    if field == "severity" {
        return input.severity;
    }
    if let Some(name) = field.strip_prefix("vars.") {
        return input.vars.get(name).map(String::as_str).unwrap_or("");
    }
    ""
}

// ── 引擎 ────────────────────────────────────────────────────────────────────

/// 单条规则是否匹配（enabled + 类型 + 条件表达式）。条件编译失败视为不匹配
/// （编辑期 UI 已校验报错，运行期静默跳过并 warn——一条坏规则不拖垮全部通知）。
///
/// 类型匹配双口径：规则的 match_type 写语义类型（规则编辑 UI 的词表），入站 typ 可能是
/// CC hook 原始事件名（如 `Stop`）——先比对原文，再经 `notif_type_for_event` 归一比对；
/// 否则预置规则（`task_complete` 等）永远匹配不到 hook 事件，完成通知整体静默。
pub fn rule_matches(rule: &Rule, input: &MatchInput) -> bool {
    if !rule.enabled {
        return false;
    }
    if let Some(t) = &rule.match_type
        && t != input.typ
        && Some(t.as_str()) != notif_type_for_event(input.typ).map(|n| n.as_str())
    {
        return false;
    }
    if let Some(cond) = rule.condition.as_deref().filter(|c| !c.trim().is_empty()) {
        match compile_expr(cond) {
            Ok(e) => {
                if !eval_expr(&e, input) {
                    return false;
                }
            }
            Err(err) => {
                tracing::warn!(rule = %rule.id, error = %err, "notify: rule condition invalid, skip");
                return false;
            }
        }
    }
    true
}

/// 全量规则匹配，返回命中规则（按传入顺序；首条命中优先的语义由调用方决定）。
pub fn evaluate<'r>(rules: &'r [Rule], input: &MatchInput) -> Vec<&'r Rule> {
    rules.iter().filter(|r| rule_matches(r, input)).collect()
}

// ── 去重窗口 ────────────────────────────────────────────────────────────────

/// 进程内去重闸（(rule_id, dedup_key) → 上次放行时刻 unix ms）。
/// 窗口 0 秒 = 不去重恒放行。票 04 接入 live dispatch 时以通知 dedup_key 调用。
#[derive(Default)]
pub struct DedupGate {
    inner: Mutex<HashMap<(String, String), i64>>,
}

impl DedupGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pass(&self, rule_id: &str, dedup_key: &str, now_ms: i64, window_secs: u32) -> bool {
        if window_secs == 0 {
            return true;
        }
        let Ok(mut g) = self.inner.lock() else { return true };
        let key = (rule_id.to_string(), dedup_key.to_string());
        match g.get(&key) {
            Some(&last) if now_ms - last < window_secs as i64 * 1000 => false,
            _ => {
                g.insert(key, now_ms);
                true
            }
        }
    }
}

/// 全局单例（与 CALIBRATE_ATTEMPT 同 idiom；供 dispatch 接入用）。
pub fn dedup_gate() -> &'static DedupGate {
    static GATE: OnceLock<DedupGate> = OnceLock::new();
    GATE.get_or_init(DedupGate::new)
}

// ── 预置规则集（系统级）────────────────────────────────────────────────────

fn rule(id: &str, name: &str, typ: &str, channels: &[Channel], dedup: u32, bypass: bool) -> Rule {
    Rule {
        id: id.to_string(),
        name: name.to_string(),
        enabled: true,
        preset: true,
        match_type: Some(typ.to_string()),
        condition: None,
        action: RuleAction {
            channels: channels.to_vec(),
            bypass_dnd: bypass,
            dedup_window_secs: dedup,
        },
    }
}

/// 预置规则（含 spec「规则引擎」节所列 4 条 + waiting_input/error 补位，共 6 条；只可开关/复制，不可改删）。
/// 注意：quota 预测的「提前 30 分钟」阈值在发射点（estimate/db_ops.rs
/// FORECAST_THRESHOLD_MIN），此处 dedup 1800s 是展示层去重兜底，两者同源对齐。
pub fn preset_rules() -> Vec<Rule> {
    vec![
        rule(
            "preset-quota-forecast",
            "配额将耗尽（提前 30 分钟，每窗口一次）",
            "quota_exhaust_forecast",
            &[Channel::Inbox, Channel::Popup],
            1800,
            false,
        ),
        rule(
            "preset-breaker-on",
            "熔断触发",
            "breaker_on",
            &[Channel::Inbox],
            0,
            false,
        ),
        rule("preset-breaker-off", "熔断恢复", "breaker_off", &[Channel::Inbox], 0, false),
        rule(
            "preset-task-complete",
            "任务完成（沿用现有默认）",
            "task_complete",
            &[Channel::Popup, Channel::Inbox, Channel::Sound, Channel::Tts],
            0,
            false,
        ),
        // 票 04 补：waiting_input / error 各一条预置（「每一种通知类型都有开关」；
        // 通道沿用旧 per_type 缺省 = 全开）。spec 所列 4 条为子集，本表含全部 4 条。
        rule(
            "preset-waiting-input",
            "等待用户输入",
            "waiting_input",
            &[Channel::Popup, Channel::Inbox, Channel::Sound, Channel::Tts],
            0,
            false,
        ),
        rule(
            "preset-error",
            "错误",
            "error",
            &[Channel::Popup, Channel::Inbox, Channel::Sound, Channel::Tts],
            0,
            false,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn input<'a>(typ: &'a str, severity: &'a str, v: &'a HashMap<String, String>) -> MatchInput<'a> {
        MatchInput { typ, severity, vars: v }
    }

    // ── 解析与报错定位 ──

    #[test]
    fn parse_errors_have_positions() {
        let e = compile_expr("severity == ").unwrap_err();
        assert_eq!(e.pos, 12);
        assert!(e.msg.contains("期望值"));
        let e = compile_expr("severity & \"x\"").unwrap_err();
        assert!(e.pos > 0 && e.msg.contains("非法符号"), "{e}");
        let e = compile_expr("(a == 1 && b == 2").unwrap_err();
        assert!(e.msg.contains("右括号"), "{e}");
        let e = compile_expr("\"unterminated").unwrap_err();
        assert!(e.msg.contains("双引号"), "{e}");
        let e = compile_expr("").unwrap_err();
        assert!(e.msg.contains("为空"));
        let e = compile_expr("a == 1 1").unwrap_err();
        assert!(e.msg.contains("多余"), "{e}");
    }

    #[test]
    fn precedence_and_semantic_equivalence() {
        // a&&b||c ≡ (a&&b)||c ≢ a&&(b||c)：用同一输入验证两种括号化结果不同、无括号与前者等价。
        let ab = compile_expr("type == \"x\" && severity == \"error\" || vars.k == \"v\"").unwrap();
        let paren_left = compile_expr("(type == \"x\" && severity == \"error\") || vars.k == \"v\"").unwrap();
        let paren_right = compile_expr("type == \"x\" && (severity == \"error\" || vars.k == \"v\")").unwrap();
        let v = vars(&[("k", "v")]);
        let i1 = input("x", "info", &v); // a=1 b=0 c=1
        let i2 = input("x", "error", &v); // a=1 b=1 c=1
        let i3 = input("y", "info", &v); // a=0 b=0 c=1
        let empty = HashMap::new();
        let i4 = input("x", "info", &empty); // a=1 b=0 c=0
        for i in [&i1, &i2, &i3, &i4] {
            assert_eq!(eval_expr(&ab, i), eval_expr(&paren_left, i), "a&&b||c ≡ (a&&b)||c");
        }
        // a&&(b||c) 在 i1（a=1,b=0,c=1）上应为 true，而 a&&b||c 在 i3（a=0,c=1）上 true、右括号版 false。
        assert!(eval_expr(&paren_right, &i1));
        assert!(eval_expr(&ab, &i3));
        assert!(!eval_expr(&paren_right, &i3));
    }

    #[test]
    fn eval_field_resolution_and_ops() {
        let v = vars(&[("platform", "glm_coding"), ("n", "3")]);
        let i = input("breaker_on", "error", &v);
        assert!(eval_expr(&compile_expr("type == \"breaker_on\"").unwrap(), &i));
        assert!(eval_expr(&compile_expr("vars.platform contains \"glm\"").unwrap(), &i));
        assert!(eval_expr(&compile_expr("vars.n != \"5\"").unwrap(), &i));
        assert!(eval_expr(&compile_expr("severity == error").unwrap(), &i)); // 裸词值
        // 缺失变量 → 空串
        assert!(eval_expr(&compile_expr("vars.missing == \"\"").unwrap(), &i));
        // 数字按字符串比较（保留原样字面量）
        assert!(eval_expr(&compile_expr("vars.n == 3").unwrap(), &i));
    }

    // ── 引擎 ──

    #[test]
    fn rule_matching_type_condition_and_disabled() {
        let mut r = rule("r1", "测试", "error", &[Channel::Inbox], 0, false);
        r.condition = Some("severity == \"error\"".to_string());
        let v = HashMap::new();
        assert!(rule_matches(&r, &input("error", "error", &v)));
        assert!(!rule_matches(&r, &input("error", "warn", &v))); // 条件不满足
        assert!(!rule_matches(&r, &input("task_complete", "error", &v))); // 类型不匹配
        r.enabled = false;
        assert!(!rule_matches(&r, &input("error", "error", &v)));
        // 坏条件 → 不匹配不 panic
        r.condition = Some("severity ~".to_string());
        r.enabled = true;
        assert!(!rule_matches(&r, &input("error", "error", &v)));
        // match_type None = 任意类型
        let mut any = rule("r2", "任意", "whatever", &[Channel::Inbox], 0, false);
        any.match_type = None;
        assert!(rule_matches(&any, &input("anything", "info", &v)));
    }

    #[test]
    fn evaluate_returns_all_hits_in_order() {
        let mut rules = preset_rules();
        let v = HashMap::new();
        let hits = evaluate(&rules, &input("breaker_on", "error", &v));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "preset-breaker-on");
        // 叠一条用户规则同类型
        rules.push(rule("user-1", "熔断也弹窗", "breaker_on", &[Channel::Popup], 0, true));
        let hits = evaluate(&rules, &input("breaker_on", "error", &v));
        assert_eq!(hits.len(), 2);
        assert!(hits[1].action.bypass_dnd);
    }

    // ── 去重窗口 ──

    #[test]
    fn dedup_gate_window() {
        let g = DedupGate::new();
        assert!(g.pass("r", "k1", 1000, 60));
        assert!(!g.pass("r", "k1", 60_000, 60)); // 59s 内
        assert!(g.pass("r", "k1", 61_000, 60)); // 过窗
        assert!(g.pass("r", "k2", 1000, 60)); // 不同 key
        assert!(g.pass("other", "k1", 2000, 60)); // 不同规则
        assert!(g.pass("r", "k3", 0, 0)); // 窗口 0 = 不去重
        assert!(g.pass("r", "k3", 1, 0));
    }

    // ── 预置规则 ──

    #[test]
    fn preset_rules_shape() {
        let presets = preset_rules();
        assert_eq!(presets.len(), 6);
        assert!(presets.iter().all(|r| r.preset && r.enabled));
        // spec 四条各就位
        let ids: Vec<&str> = presets.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"preset-quota-forecast"));
        assert!(ids.contains(&"preset-breaker-on"));
        assert!(ids.contains(&"preset-breaker-off"));
        assert!(ids.contains(&"preset-task-complete"));
        assert!(ids.contains(&"preset-waiting-input"));
        assert!(ids.contains(&"preset-error"));
        // 配额预测：收件箱+弹窗 + 30 分钟去重兜底
        let q = presets.iter().find(|r| r.id == "preset-quota-forecast").unwrap();
        assert!(q.action.channels.contains(&Channel::Inbox));
        assert!(q.action.channels.contains(&Channel::Popup));
        assert_eq!(q.action.dedup_window_secs, 1800);
        // 任务完成：沿用现有默认四通道
        let t = presets.iter().find(|r| r.id == "preset-task-complete").unwrap();
        assert_eq!(t.action.channels.len(), 4);
    }

    #[test]
    fn every_semantic_type_coverable() {
        // 「每一种通知类型都有开关」：预置覆盖的类型由预置承接；
        // 其余语义类型必须能被空 match_type 规则承接（引擎能力面）。
        let catch_all = Rule {
            id: "all".into(),
            name: "任意".into(),
            enabled: true,
            preset: false,
            match_type: None,
            condition: None,
            action: RuleAction { channels: vec![Channel::Inbox], bypass_dnd: false, dedup_window_secs: 0 },
        };
        let v = HashMap::new();
        for t in ["task_complete", "waiting_input", "error", "breaker_on", "breaker_off", "quota_exhaust_forecast"] {
            assert!(
                rule_matches(&catch_all, &input(t, "info", &v)),
                "{t} 应可被通用规则匹配"
            );
        }
    }
}
