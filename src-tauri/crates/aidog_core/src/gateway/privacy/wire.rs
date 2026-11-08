//! 代理链路接线辅助层（票 02）：设置读取、JSON 体脱敏 / 还原、流式还原窗口、fail-closed 闸。
//!
//! 引擎（票 01 产物）提供纯函数 scrub / restore；本层只做接线：
//! - 设置键 scope=`privacy` key=`scrub`（组合即 `privacy_scrub`），默认关。
//! - fail-closed 闸：脱敏开启但检测层不可用 → 调用方 503 拦截，绝不按原样发出。
//!   规则引擎常驻内存进程级；`engine_unavailable` 是票 03 本地模型后端健康检查的接口位
//!   （健康检查不过 → 置位），兼作集成测试强制触发 fail-closed 的测试缝。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use super::{scrub as engine_scrub, ScrubMap, ScrubOptions};

/// 设置键 scope=`privacy` key=`scrub` 的形状。默认全关。
/// `secrets` / `personal` 两档类型开关；细粒度 per-kind 覆盖留票 04（映射 ScrubOptions.per_kind）。
/// 字段缺省全 false（bool Default 语义与 fail-closed 方向一致：默认不脱敏 = 零改动直通）。
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PrivacyScrubSettings {
    /// 出站脱敏总开关（默认 false）。
    pub enabled: bool,
    /// 机密类（API key / 私钥 / Bearer token）。
    pub secrets: bool,
    /// 个人信息类（邮箱 / 银行卡 / 证件号）。
    pub personal: bool,
    /// 本地 PII 模型后端开关（票 03，默认 false = 完全不走模型路径、零请求）。
    pub model_enabled: bool,
    /// 本地检测服务 URL（POST 文本 → span 列表，契约见 `privacy::model`）。
    pub model_endpoint: String,
    /// 单类型细粒度覆盖（票 04）：键 = `PiiKind::as_str` 词表，优先于两档总开关；词表外键忽略。
    pub per_kind: std::collections::HashMap<String, bool>,
}

/// 读取脱敏设置（缺省 / 解析失败 → 默认全关）。
pub async fn get_privacy_scrub_settings(db: &aidog_db::Db) -> PrivacyScrubSettings {
    match aidog_db::get_setting(db, "privacy", "scrub").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => PrivacyScrubSettings::default(),
    }
}

/// 按 `PrivacyScrubSettings` 构造引擎 ScrubOptions。
pub fn scrub_options(s: &PrivacyScrubSettings) -> ScrubOptions {
    ScrubOptions {
        secrets: s.secrets,
        personal: s.personal,
        per_kind: s
            .per_kind
            .iter()
            .filter_map(|(k, &on)| pii_kind_from_str(k).map(|kind| (kind, on)))
            .collect(),
    }
}

/// `PiiKind::as_str` 词表反查（票 04 per-kind 设置键）；词表外 None。
fn pii_kind_from_str(s: &str) -> Option<super::PiiKind> {
    [
        super::PiiKind::ApiKey,
        super::PiiKind::PrivateKey,
        super::PiiKind::BearerToken,
        super::PiiKind::Email,
        super::PiiKind::BankCard,
        super::PiiKind::IdNumber,
    ]
    .into_iter()
    .find(|k| k.as_str() == s)
}

/// 检测层不可用标记（票 03 接口位 + 集成测试缝）。
static ENGINE_UNAVAILABLE: AtomicBool = AtomicBool::new(false);

/// 检测层当前是否不可用。规则引擎常驻内存，此位恒 false；票 03 模型后端健康检查
/// 不过时置位 → fail-closed 闸生效。
pub fn engine_unavailable() -> bool {
    ENGINE_UNAVAILABLE.load(Ordering::SeqCst)
}

/// 置位 / 复位检测层不可用标记（票 03 健康检查写入；集成测试强制触发 fail-closed 用）。
pub fn set_engine_unavailable(unavailable: bool) {
    ENGINE_UNAVAILABLE.store(unavailable, Ordering::SeqCst);
}

/// fail-closed 判定：设置开启但检测层不可用 → Block（宁拦不发）。
pub fn privacy_gate(settings: &PrivacyScrubSettings, available: bool) -> PrivacyGate {
    if settings.enabled && !available {
        PrivacyGate::Block
    } else {
        PrivacyGate::Allow
    }
}

/// fail-closed 判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyGate {
    Allow,
    Block,
}

/// 遍历 JSON body 的字符串值做脱敏，返回合并映射表（无命中返回空表）。
/// 结构键（字段名 / 数组形状）不动，只替换叶子字符串内的命中。
pub fn scrub_json(body: &mut Value, opts: &ScrubOptions) -> ScrubMap {
    let mut merged = ScrubMap::new();
    scrub_value(body, opts, &mut merged);
    merged
}

/// 票 03：`model_enabled=true` 时每个叶子字符串额外走本地模型检测（契约见 `privacy::model`），
/// 模型 span 与规则 span 合并后统一替换；`model_enabled=false` / 端点未配置 → 与 `scrub_json`
/// 等价（零请求）。
///
/// fail-closed（review #2）：模型检测失败（网络 / 非 2xx / 不可解析）不上抛被吞——部分
/// 脱敏结果照样出站等于把规则漏掉的部分原样外泄。Err 时先置位 `engine_unavailable`
/// （后续 attempt 由闸拦截）再上抛，调用方按 fail-closed 503 拦截本次请求。
// Err(()) 是有意的 fail-closed 信号，调用方不读错误内容，无可携带信息。
#[allow(clippy::result_unit_err)]
pub async fn scrub_json_with_model(
    body: &mut Value,
    opts: &ScrubOptions,
    settings: &PrivacyScrubSettings,
) -> Result<ScrubMap, ()> {
    if !settings.model_enabled || settings.model_endpoint.is_empty() {
        return Ok(scrub_json(body, opts));
    }
    let endpoint = settings.model_endpoint.clone();
    let mut merged = ScrubMap::new();
    let mut leaves: Vec<&mut String> = Vec::new();
    collect_string_leaves(body, &mut leaves);
    for s in leaves {
        let mut spans = super::detect::collect_spans(s, |k| opts.enabled(k));
        // review #2 fail-closed：检测失败（网络 / 非 2xx / 不可解析）→ 置位
        // engine_unavailable（后续 attempt 由闸拦截）后上抛 Err，调用方 503 拦本次。
        let model = match super::model::model_spans(&endpoint, s).await {
            Ok(m) => m,
            Err(()) => {
                super::set_engine_unavailable(true);
                return Err(());
            }
        };
        super::model::merge_model_spans(&mut spans, model);
        let (new_s, map) = super::engine::apply_spans(s, spans);
        if !map.is_empty() {
            for (p, o) in map.iter() {
                merged.insert(p, o);
            }
            *s = new_s;
        }
    }
    Ok(merged)
}

/// 收集 JSON 树的全部叶子字符串（可变引用，原地替换用）。
fn collect_string_leaves<'a>(v: &'a mut Value, out: &mut Vec<&'a mut String>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(items) => items.iter_mut().for_each(|i| collect_string_leaves(i, out)),
        Value::Object(map) => map.values_mut().for_each(|x| collect_string_leaves(x, out)),
        _ => {}
    }
}

fn scrub_value(v: &mut Value, opts: &ScrubOptions, merged: &mut ScrubMap) {
    match v {
        Value::String(s) => {
            let (new_s, map) = engine_scrub(s, opts);
            if !map.is_empty() {
                for (p, o) in map.iter() {
                    merged.insert(p, o);
                }
                *s = new_s;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|i| scrub_value(i, opts, merged)),
        Value::Object(map) => map.values_mut().for_each(|x| scrub_value(x, opts, merged)),
        _ => {}
    }
}

/// JSON 字符串字面量转义（替换值安全嵌入 JSON / SSE 的 JSON 帧用）。
/// 与 serde_json::to_string 的区别：不外层加引号——调用方是在已有 JSON 字符串值内做替换。
fn json_escape_inner(original: &str) -> String {
    let mut esc = String::with_capacity(original.len());
    for c in original.chars() {
        match c {
            '"' => esc.push_str("\\\""),
            '\\' => esc.push_str("\\\\"),
            '\n' => esc.push_str("\\n"),
            '\r' => esc.push_str("\\r"),
            '\t' => esc.push_str("\\t"),
            c if (c as u32) < 0x20 => esc.push_str(&format!("\\u{:04x}", c as u32)),
            c => esc.push(c),
        }
    }
    esc
}

/// 把 `text` 中登记过的占位符换回原值（JSON / SSE 文本上下文安全形态）：
/// 与引擎 restore 的差别只在替换值先做 JSON 字符串转义——原值含引号 / 反斜杠 /
/// 换行（PEM 私钥）时直接替换会打破外层 JSON / SSE JSON 帧结构。
pub fn restore_json_escaped(text: &str, map: &ScrubMap) -> String {
    // 票 03 短路：全部占位符均以 `{{PII_` 开头（json_escape_inner 不转义 `{`，
    // 占位符在任何 wire 文本中都是字面形态），文本不含该前缀 ⇒ 无占位符在场
    // ⇒ 逐占位符 replace 必为零改动，原样返回。严格推理，漏还原为零。
    if !text.contains(PLACEHOLDER_HEAD_STR) {
        return text.to_string();
    }
    let mut out = text.to_string();
    for (placeholder, original) in map.iter() {
        if !out.contains(placeholder.as_str()) {
            continue;
        }
        out = out.replace(placeholder.as_str(), &json_escape_inner(original));
    }
    out
}

/// 占位符最大长度：`{{PII_BEARER_TOKEN_8hex}}` = 29 字节（当前规则表最长类型名）。
/// 新增更长类型名时同步上调。
const MAX_PLACEHOLDER_LEN: usize = 29;

/// 占位符起始标记（全部占位符的公共 ASCII 前缀）。
const PLACEHOLDER_HEAD_STR: &str = "{{PII_";
const PLACEHOLDER_HEAD: &[u8] = PLACEHOLDER_HEAD_STR.as_bytes();

/// 流式还原窗口：按占位符完整性切分（票 02 选定策略，理由见票面 ## Answer）。
/// 每次 `push_bytes` 扣留尾部可能的半截占位符（≤ MAX_PLACEHOLDER_LEN-1 字节），
/// 攒齐再还原下发，占位符永不被 chunk 边界切断；流末 / 断连 / 终止符时
/// `finish()` 冲刷残留。与中间件 StreamMasker 同一滑窗 idiom（首字延迟上界 28 字节）。
/// 扣留按字节扫 ASCII 标记，下发前回落到 char boundary（防多字节字符被切半）。
pub struct StreamRestorer {
    map: Arc<ScrubMap>,
    pending: Vec<u8>,
}

/// `haystack[..i..]` 是否以某个已登记占位符完整开头（占位符均在 haystack 内结束）。
fn starts_with_registered_placeholder(haystack: &[u8], i: usize, map: &ScrubMap) -> bool {
    map.iter()
        .any(|(p, _)| haystack[i..].starts_with(p.as_bytes()))
}

/// `haystack` 尾部与 `PLACEHOLDER_HEAD` 的最长公共前缀匹配长度（半截起始标记扣留用），
/// 不匹配返回 0。k ∈ 1..PLACEHOLDER_HEAD.len()。
fn tail_head_prefix_len(haystack: &[u8]) -> usize {
    let max = PLACEHOLDER_HEAD.len() - 1;
    let start = haystack.len().saturating_sub(max);
    let tail = &haystack[start..];
    for k in (1..=tail.len()).rev() {
        if PLACEHOLDER_HEAD[..k] == tail[tail.len() - k..] {
            return k;
        }
    }
    0
}

impl StreamRestorer {
    pub fn new(map: Arc<ScrubMap>) -> Self {
        Self { map, pending: Vec::new() }
    }

    /// 喂一段（未还原）字节，返回可安全下发的还原后文本（UTF-8 无损）。
    pub fn push_bytes(&mut self, data: &[u8]) -> String {
        self.pending.extend_from_slice(data);
        let mut emit_len = self.pending.len();
        // 扣留判定：末个起始标记若可能是半截占位符（完整标记未到 / 长度不足 29）→ 从标记处扣留。
        if let Some(i) = last_head_occurrence(&self.pending) {
            let remaining = self.pending.len() - i;
            if remaining < MAX_PLACEHOLDER_LEN
                && !starts_with_registered_placeholder(&self.pending, i, &self.map)
            {
                emit_len = i;
            }
        } else {
            // 无起始标记：尾部可能是半截 `{{PII_` → 扣留匹配前缀长度。
            emit_len -= tail_head_prefix_len(&self.pending);
        }
        // 下发段回落 char boundary（被切半的多字节字符留在 pending 等下块拼齐）。
        while emit_len > 0 && !is_char_boundary(&self.pending, emit_len) {
            emit_len -= 1;
        }
        if emit_len == 0 {
            return String::new();
        }
        let text = String::from_utf8_lossy(&self.pending[..emit_len]).to_string();
        self.pending.drain(..emit_len);
        restore_json_escaped(&text, &self.map)
    }

    /// 冲刷扣留的残留（还原后返回；此后窗口为空，幂等）。
    pub fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }
        let text = String::from_utf8_lossy(&self.pending).to_string();
        self.pending.clear();
        restore_json_escaped(&text, &self.map)
    }
}

/// `bytes[..cut]` 是否落在 UTF-8 字符边界（Vec<u8> 无 is_char_boundary，自行判）。
fn is_char_boundary(bytes: &[u8], cut: usize) -> bool {
    cut == 0
        || cut >= bytes.len()
        || (bytes[cut] & 0b1100_0000) != 0b1000_0000
}

/// `haystack` 内 `PLACEHOLDER_HEAD` 最后一次出现的字节下标（无则 None）。
fn last_head_occurrence(haystack: &[u8]) -> Option<usize> {
    if haystack.len() < PLACEHOLDER_HEAD.len() {
        return None;
    }
    (0..=haystack.len() - PLACEHOLDER_HEAD.len())
        .rev()
        .find(|&i| &haystack[i..i + PLACEHOLDER_HEAD.len()] == PLACEHOLDER_HEAD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(enabled: bool) -> PrivacyScrubSettings {
        PrivacyScrubSettings {
            enabled,
            secrets: true,
            personal: true,
            model_enabled: false,
            model_endpoint: String::new(),
            per_kind: Default::default(),
        }
    }

    #[test]
    fn gate_blocks_only_when_enabled_and_unavailable() {
        assert_eq!(privacy_gate(&settings(true), false), PrivacyGate::Block);
        assert_eq!(privacy_gate(&settings(true), true), PrivacyGate::Allow);
        assert_eq!(privacy_gate(&settings(false), false), PrivacyGate::Allow);
    }

    /// 票 03：旧值兼容——无新字段的旧 JSON 反序列化得默认（model 路径关、零请求）。
    #[test]
    fn privacy_scrub_settings_old_json_defaults() {
        let s: PrivacyScrubSettings = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(s.enabled);
        assert!(!s.model_enabled);
        assert!(s.model_endpoint.is_empty());
        assert!(s.per_kind.is_empty());
    }

    /// 票 04：per-kind 细粒度覆盖两档总开关；词表外键忽略。
    #[test]
    fn scrub_options_per_kind_overrides() {
        let s = PrivacyScrubSettings {
            enabled: true,
            secrets: true,
            personal: false,
            per_kind: [
                ("EMAIL".to_string(), true),
                ("BEARER_TOKEN".to_string(), false),
                ("NOT_A_KIND".to_string(), true),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let o = scrub_options(&s);
        use crate::gateway::privacy::PiiKind;
        assert!(o.enabled(PiiKind::Email), "per_kind true overrides personal=false");
        assert!(!o.enabled(PiiKind::BearerToken), "per_kind false overrides secrets=true");
        assert!(!o.enabled(PiiKind::BankCard), "personal=false applies without override");
        assert!(o.enabled(PiiKind::ApiKey), "secrets=true applies");
    }

    #[test]
    fn scrub_json_replaces_nested_string_values() {
        let opts = ScrubOptions::all();
        let mut body = serde_json::json!({
            "model": "claude-x",
            "messages": [{ "role": "user", "content": "key is sk-ant-api03-ABCDEFGHIJKLMNOPQRST" }]
        });
        let map = scrub_json(&mut body, &opts);
        assert!(!map.is_empty());
        let s = serde_json::to_string(&body).unwrap();
        assert!(!s.contains("sk-ant-api03-ABCDEFGHIJKLMNOPQRST"));
        assert!(s.contains("{{PII_"));
        // model 等非命中值不动
        assert!(s.contains("\"claude-x\""));
    }

    #[test]
    fn restore_json_escaped_roundtrip_preserves_json_frame() {
        let opts = ScrubOptions::all();
        let original = "line1\n\"quoted\" back\\slash";
        // 引擎只在命中规则时替换；用 email（personal 类规则）构造确定命中。
        let mut body = serde_json::json!({ "content": format!("mail me: bob@example.com {original}") });
        let map = scrub_json(&mut body, &opts);
        let scrubbed = serde_json::to_string(&body).unwrap();
        let restored = restore_json_escaped(&scrubbed, &map);
        let back: Value = serde_json::from_str(&restored).expect("restored must stay valid JSON");
        let text = back["content"].as_str().unwrap();
        assert!(text.contains("bob@example.com"));
        assert!(text.contains(original));
    }

    #[test]
    fn restore_json_escaped_identity_without_hits() {
        let opts = ScrubOptions::all();
        let mut body = serde_json::json!({ "content": "no sensitive data here" });
        let map = scrub_json(&mut body, &opts);
        assert!(map.is_empty());
        let s = serde_json::to_string(&body).unwrap();
        assert_eq!(restore_json_escaped(&s, &map), s);
    }

    #[test]
    fn stream_restorer_reassembles_split_placeholder() {
        let opts = ScrubOptions::all();
        let mut body = serde_json::json!({ "content": "mail me: bob@example.com thanks" });
        let map = Arc::new(scrub_json(&mut body, &opts));
        let scrubbed = serde_json::to_string(&body).unwrap();
        let mut r = StreamRestorer::new(map);
        // 按不规则边界切块（含切在占位符正中）
        let mut out = String::new();
        for chunk in scrubbed.as_bytes().chunks(7) {
            out.push_str(&r.push_bytes(chunk));
        }
        out.push_str(&r.finish());
        let back: Value = serde_json::from_str(&out).expect("stream output must stay valid JSON");
        let text = back["content"].as_str().unwrap();
        assert_eq!(text, "mail me: bob@example.com thanks");
    }

    #[test]
    fn stream_restorer_passthrough_without_hits() {
        let mut r = StreamRestorer::new(Arc::new(ScrubMap::new()));
        let text = "plain text 没有 placeholder，多字节字符完整下发";
        let mut out = r.push_bytes(text.as_bytes());
        out.push_str(&r.finish());
        assert_eq!(out, text);
    }

    /// 检测契约 stub：探活（text 空）→ 200，检测（text 非空）→ 500。
    /// 确定性构造「探活过、检测败」，禁真实外呼。
    async fn spawn_detect_fail_stub() -> String {
        use axum::body::Bytes;
        use axum::response::IntoResponse as _;
        use axum::routing::any;
        let app = axum::Router::new().fallback(any(|body: Bytes| async move {
            let text = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_string))
                .unwrap_or_default();
            if text.is_empty() {
                axum::Json(serde_json::json!({ "spans": [] })).into_response()
            } else {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "detect fail",
                )
                    .into_response()
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{addr}/")
    }

    /// review #2 红测试：探活过（健康闸放行）但检测层 500 → 修前 `if let Ok` 吞 Err，
    /// 返回只剩规则脱敏的部分结果；修后置位 fail-closed 并上抛 Err。
    #[tokio::test]
    async fn model_detect_failure_propagates_err_and_sets_fail_closed_flag() {
        set_engine_unavailable(false);
        let ep = spawn_detect_fail_stub().await;
        let s = PrivacyScrubSettings {
            enabled: true,
            secrets: true,
            personal: true,
            model_enabled: true,
            model_endpoint: ep,
            per_kind: Default::default(),
        };
        let mut body = serde_json::json!({ "content": "mail bob@example.com hi" });
        let r = scrub_json_with_model(&mut body, &ScrubOptions::all(), &s).await;
        assert!(r.is_err(), "检测层失败必须上抛 Err（fail-closed），实际: {r:?}");
        assert!(engine_unavailable(), "Err 时必须置位 engine_unavailable");
        set_engine_unavailable(false);
    }
}
