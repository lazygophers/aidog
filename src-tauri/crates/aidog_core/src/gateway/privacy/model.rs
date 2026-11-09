//! 票 03：本地 PII 模型后端接口。
//!
//! 设置 `privacy_scrub.model_enabled`（默认 false）+ `model_endpoint`（本地检测服务 URL）。
//! 检测契约：POST `{"text": ...}` → `{"spans": [{"start","end","type"}]}`；
//! 健康检查进程内缓存短 TTL（按端点键控）；`model_enabled=true` 且端点不可达 / 超时 /
//! 未配置 → `engine_unavailable` 置位走 02 的 fail-closed 503 置位路径；
//! `model_enabled=false` 零请求。

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use super::detect::SpanKind;
use super::{PiiKind, PrivacyScrubSettings, set_engine_unavailable};

/// 模型后端单次请求（探活 / 检测）超时。
const MODEL_TIMEOUT: Duration = Duration::from_secs(3);

/// 健康检查结果缓存 TTL。
const HEALTH_TTL: Duration = Duration::from_secs(30);

/// 本地检测服务客户端：端点是本机服务，禁走系统代理（本机 env 代理会把 localhost 请求绕道）。
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("reqwest client")
});

/// 健康检查缓存：端点 →（取样时刻，结果）。
static HEALTH_CACHE: LazyLock<Mutex<HashMap<String, (Instant, bool)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 检测契约 span（响应侧）。`type` 为 `PiiKind::as_str` 词表；词表外跳过。
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ContractSpan {
    pub start: usize,
    pub end: usize,
    #[serde(rename = "type")]
    pub kind: String,
}

/// 检测契约响应信封 `{"spans": [...]}`。
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ContractResp {
    #[serde(default)]
    pub spans: Vec<ContractSpan>,
}

/// 模型 span（已校验：区间在界内、落在 char boundary、类型可解析）。
#[derive(Debug, Clone)]
pub(crate) struct ModelSpan {
    pub start: usize,
    pub end: usize,
    pub kind: PiiKind,
}

fn kind_from_contract(s: &str) -> Option<PiiKind> {
    Some(match s {
        "API_KEY" => PiiKind::ApiKey,
        "PRIVATE_KEY" => PiiKind::PrivateKey,
        "BEARER_TOKEN" => PiiKind::BearerToken,
        "EMAIL" => PiiKind::Email,
        "BANK_CARD" => PiiKind::BankCard,
        "ID_NUMBER" => PiiKind::IdNumber,
        _ => return None,
    })
}

/// 探活：POST 空文本，2xx 即健康。
async fn probe(endpoint: &str) -> bool {
    CLIENT
        .post(endpoint)
        .json(&serde_json::json!({ "text": "" }))
        .timeout(MODEL_TIMEOUT)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// 健康检查（进程内缓存，TTL 内同端点复用）。
pub(crate) async fn model_health(endpoint: &str) -> bool {
    let now = Instant::now();
    if let Some((_, ok)) = HEALTH_CACHE
        .lock()
        .unwrap()
        .get(endpoint)
        .filter(|(at, _)| now.duration_since(*at) < HEALTH_TTL)
        .copied()
    {
        return ok;
    }
    let ok = probe(endpoint).await;
    HEALTH_CACHE
        .lock()
        .unwrap()
        .insert(endpoint.to_string(), (Instant::now(), ok));
    ok
}

/// 模型检测：POST 文本 → span 列表；请求失败 / 非 2xx / 响应不可解析 → Err。
pub(crate) async fn model_spans(endpoint: &str, text: &str) -> Result<Vec<ModelSpan>, ()> {
    let resp = CLIENT
        .post(endpoint)
        .json(&serde_json::json!({ "text": text }))
        .timeout(MODEL_TIMEOUT)
        .send()
        .await
        .map_err(|_| ())?;
    if !resp.status().is_success() {
        return Err(());
    }
    let parsed: ContractResp = resp.json().await.map_err(|_| ())?;
    Ok(parsed
        .spans
        .into_iter()
        .filter_map(|s| {
            if s.start >= s.end
                || s.end > text.len()
                || !text.is_char_boundary(s.start)
                || !text.is_char_boundary(s.end)
            {
                return None;
            }
            Some(ModelSpan {
                start: s.start,
                end: s.end,
                kind: kind_from_contract(&s.kind)?,
            })
        })
        .collect())
}

/// 合并模型 span 进规则 span：区间重叠时规则先占（模型 span 丢弃），其余按起始位置插入。
pub(crate) fn merge_model_spans(rule: &mut Vec<SpanKind>, model: Vec<ModelSpan>) {
    for m in model {
        if rule.iter().any(|s| m.start < s.end && s.start < m.end) {
            continue;
        }
        rule.push(SpanKind { start: m.start, end: m.end, kind: m.kind });
    }
    rule.sort_by_key(|s| s.start);
}

/// 票 03 闸门刷新：总开关开且 `model_enabled=true` 时按端点健康置位 / 复位
/// `engine_unavailable`（handler 在 privacy_gate 前调用一次；TTL 内零 HTTP）；
/// 总开关关或 `model_enabled=false` 恒复位（review #6：总开关关不探活，白探活浪费请求）。
pub async fn refresh_model_gate(settings: &PrivacyScrubSettings) {
    let ok = if settings.enabled
        && settings.model_enabled
        && !settings.model_endpoint.is_empty()
    {
        model_health(&settings.model_endpoint).await
    } else {
        true
    };
    set_engine_unavailable(!ok);
}

// ── 测试：本地 stub（axum，127.0.0.1:0），禁真实外呼 ──────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::Arc;

    use crate::gateway::privacy::{
        engine_unavailable, privacy_gate, scrub_json_with_model, PrivacyGate, ScrubOptions,
    };

    use axum::extract::State;
    use axum::routing::post;
    use axum::{Json, Router};

    const EMAIL: &str = "bob@example.com";

    /// 起本地检测 stub：计数每请求数，恒返 `{"spans": spans}`（2xx）。
    async fn spawn_stub(spans: serde_json::Value, counter: Arc<AtomicUsize>) -> String {
        struct StubState {
            counter: Arc<AtomicUsize>,
            spans: serde_json::Value,
        }
        async fn handler(State(st): State<Arc<StubState>>) -> Json<serde_json::Value> {
            st.counter.fetch_add(1, AtomicOrdering::SeqCst);
            Json(serde_json::json!({ "spans": st.spans }))
        }
        let app = Router::new()
            .route("/", post(handler))
            .with_state(Arc::new(StubState { counter, spans }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{addr}/")
    }

    fn model_settings(endpoint: &str) -> PrivacyScrubSettings {
        PrivacyScrubSettings {
            enabled: true,
            secrets: true,
            personal: true,
            model_enabled: true,
            model_endpoint: endpoint.to_string(),
            per_kind: Default::default(),
        }
    }

    #[tokio::test]
    async fn model_spans_merge_into_placeholders() {
        let counter = Arc::new(AtomicUsize::new(0));
        // stub span 指向文本里非邮箱区域（"hello"），规则引擎不会命中 → 只有模型路径能替换。
        let ep = spawn_stub(
            serde_json::json!([{ "start": 0, "end": 5, "type": "EMAIL" }]),
            counter,
        )
        .await;
        let mut body = serde_json::json!({ "content": "hello world" });
        let map = scrub_json_with_model(&mut body, &ScrubOptions::all(), &model_settings(&ep))
            .await
            .expect("stub 恒 2xx，不应 Err");
        let s = serde_json::to_string(&body).unwrap();
        assert!(s.contains("{{PII_EMAIL_"), "model span must scrub: {s}");
        assert!(!s.contains("hello"), "original region must be gone: {s}");
        assert_eq!(map.len(), 1);
        let (p, orig) = map.iter().next().unwrap();
        assert_eq!(orig, "hello");
        assert!(p.starts_with("{{PII_EMAIL_"));
    }

    #[tokio::test]
    async fn model_endpoint_down_fail_closed() {
        set_engine_unavailable(false);
        // 死端口：连接拒绝 → 探活 false → fail-closed 置位。
        let s = model_settings("http://127.0.0.1:9/");
        refresh_model_gate(&s).await;
        assert!(
            crate::gateway::privacy::engine_unavailable(),
            "dead endpoint + model_enabled must set fail-closed"
        );
        assert_eq!(
            privacy_gate(&s, !engine_unavailable()),
            PrivacyGate::Block
        );
        set_engine_unavailable(false);
    }

    #[tokio::test]
    async fn model_disabled_no_requests() {
        let counter = Arc::new(AtomicUsize::new(0));
        let ep = spawn_stub(serde_json::json!([]), counter.clone()).await;
        let mut s = model_settings(&ep);
        s.model_enabled = false;
        refresh_model_gate(&s).await;
        assert!(!engine_unavailable());
        let mut body = serde_json::json!({ "content": format!("mail {EMAIL} end") });
        let map = scrub_json_with_model(&mut body, &ScrubOptions::all(), &s)
            .await
            .expect("model 关闭走规则路径，不应 Err");
        assert_eq!(counter.load(AtomicOrdering::SeqCst), 0, "no request when model off");
        // 规则引擎照常工作
        assert_eq!(map.len(), 1);
        assert!(serde_json::to_string(&body).unwrap().contains("{{PII_EMAIL_"));
    }

    #[tokio::test]
    async fn health_cache_reuse_within_ttl() {
        let counter = Arc::new(AtomicUsize::new(0));
        let ep = spawn_stub(serde_json::json!([]), counter.clone()).await;
        assert!(model_health(&ep).await);
        assert_eq!(counter.load(AtomicOrdering::SeqCst), 1);
        // TTL 内复用缓存，不发请求
        assert!(model_health(&ep).await);
        assert_eq!(counter.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn health_cache_stale_reprobes() {
        let counter = Arc::new(AtomicUsize::new(0));
        let ep = spawn_stub(serde_json::json!([]), counter.clone()).await;
        // 预置过期缓存条目（时钟实走，不 sleep 30s）
        HEALTH_CACHE
            .lock()
            .unwrap()
            .insert(ep.clone(), (Instant::now() - HEALTH_TTL - Duration::from_secs(1), true));
        assert!(model_health(&ep).await);
        assert_eq!(counter.load(AtomicOrdering::SeqCst), 1, "stale entry must reprobe");
    }

    #[test]
    fn merge_drops_overlapping_model_span() {
        let mut rule = vec![SpanKind { start: 4, end: 20, kind: PiiKind::ApiKey }];
        merge_model_spans(
            &mut rule,
            vec![
                ModelSpan { start: 0, end: 4, kind: PiiKind::Email },
                // 与规则 span 重叠 → 丢弃
                ModelSpan { start: 10, end: 22, kind: PiiKind::Email },
            ],
        );
        assert_eq!(rule.len(), 2);
        assert_eq!(rule[0].start, 0);
        assert_eq!(rule[1].start, 4);
    }

    /// review #1 锁定测试：两个互相重叠的模型 span 经 merge 后只剩非重叠集——重叠检查
    /// `rule.iter().any` 同样覆盖已 push 的模型 span，第二个被丢弃，`apply_spans` 不存在
    /// 重叠 panic 路径。跑绿 = 实证 review #1 的 panic 前提不成立。
    #[test]
    fn merged_overlapping_model_spans_cannot_double_push() {
        let mut rule = Vec::new();
        merge_model_spans(
            &mut rule,
            vec![
                ModelSpan { start: 0, end: 10, kind: PiiKind::Email },
                ModelSpan { start: 5, end: 15, kind: PiiKind::Email },
            ],
        );
        assert_eq!(rule.len(), 1, "重叠模型 span 必须被合并逻辑丢弃");
        let text = "abcdefghij klmno";
        let (out, map) = crate::gateway::privacy::engine::apply_spans(
            text,
            rule.into_iter().map(|s| SpanKind { start: s.start, end: s.end, kind: s.kind }).collect(),
        );
        assert!(out.starts_with("{{PII_EMAIL_"), "模型 span 应替换为占位符: {out}");
        assert!(out.ends_with(" klmno"), "未命中区域应原样保留: {out}");
        assert_eq!(map.len(), 1);
        assert_eq!(crate::gateway::privacy::restore(&out, &map), text, "往返无损");
    }

    /// review #6 红测试：refresh_model_gate 判据缺 `settings.enabled` → 总开关关仍探活。
    /// 修后：总开关关 → 不探活（零请求）、恒复位 fail-closed 位。
    #[tokio::test]
    async fn refresh_gate_skips_probe_when_scrub_disabled() {
        let counter = Arc::new(AtomicUsize::new(0));
        let ep = spawn_stub(serde_json::json!([]), counter.clone()).await;
        let mut s = model_settings(&ep);
        s.enabled = false; // 总开关关，model_enabled=true、端点可达
        refresh_model_gate(&s).await;
        assert_eq!(
            counter.load(AtomicOrdering::SeqCst),
            0,
            "总开关关 → 不应发探活请求"
        );
        assert!(!engine_unavailable(), "总开关关 → 不应置位 fail-closed");
    }

    #[test]
    fn contract_unknown_type_and_bad_bounds_skipped() {
        let text = "hello world";
        let parsed: Vec<ContractSpan> =
            serde_json::from_str(r#"[{"start":0,"end":5,"type":"MYSTERY"}]"#).unwrap();
        let out: Vec<ModelSpan> = parsed
            .into_iter()
            .filter_map(|s| {
                if s.start >= s.end
                    || s.end > text.len()
                    || !text.is_char_boundary(s.start)
                    || !text.is_char_boundary(s.end)
                {
                    return None;
                }
                Some(ModelSpan { start: s.start, end: s.end, kind: kind_from_contract(&s.kind)? })
            })
            .collect();
        assert!(out.is_empty());
    }
}
