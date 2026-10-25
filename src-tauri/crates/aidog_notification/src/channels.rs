//! 外部通知渠道层（通知体系重设计票 03）。
//!
//! 7 渠道：generic_webhook（URL+JSON 模板）/ telegram / bark / discord / slack /
//! email（SMTP，lettre）/ feishu。无配置=旁路，配置即启用；投递失败退避重试
//! （1s/2s，上限 3 次尝试），终态（成功/失败/耗时）落 notification_delivery。
//!
//! 凭据安全：错误信息落库前抹掉 bot_token / SMTP 密码（reqwest 错误可能含 URL，
//! telegram URL 内嵌 token）；日志只记渠道名与脱敏后错误，不记请求头。

use std::sync::Arc;
use std::time::Duration;

use aidog_db::models::{EmailCfg, ExternalChannels, TelegramCfg, UrlWebhookCfg, WebhookCfg};
use aidog_db::Db;
use serde_json::json;

/// 外部渠道种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalChannel {
    Webhook,
    Telegram,
    Bark,
    Discord,
    Slack,
    Email,
    Feishu,
}

impl ExternalChannel {
    pub const ALL: &'static [ExternalChannel] = &[
        ExternalChannel::Webhook,
        ExternalChannel::Telegram,
        ExternalChannel::Bark,
        ExternalChannel::Discord,
        ExternalChannel::Slack,
        ExternalChannel::Email,
        ExternalChannel::Feishu,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ExternalChannel::Webhook => "webhook",
            ExternalChannel::Telegram => "telegram",
            ExternalChannel::Bark => "bark",
            ExternalChannel::Discord => "discord",
            ExternalChannel::Slack => "slack",
            ExternalChannel::Email => "email",
            ExternalChannel::Feishu => "feishu",
        }
    }

    pub fn try_from_str(s: &str) -> Option<Self> {
        ExternalChannel::ALL
            .iter()
            .copied()
            .find(|c| c.as_str() == s)
    }
}

/// 投递终态摘要（落库 + 命令回传用）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeliverySummary {
    pub channel: String,
    pub ok: bool,
    pub error: String,
    pub duration_ms: i64,
    pub attempts: u32,
}

const MAX_ATTEMPTS: u32 = 3;

/// 单渠道投递：退避重试（1s/2s）+ 终态落库。fire-and-forget 场景由调用方 spawn。
/// `secrets`：本渠道涉及的凭据值（错误信息抹除用）。
pub async fn deliver_external(
    db: &Arc<Db>,
    chan: ExternalChannel,
    settings: &ExternalChannels,
    title: &str,
    body: &str,
    action_key: &str,
) -> DeliverySummary {
    let started = std::time::Instant::now();
    let mut attempts = 0u32;
    let mut last_err = String::new();
    loop {
        attempts += 1;
        let (cfg_err, send_res) = send_once(chan, settings, title, body).await;
        match (cfg_err, send_res) {
            (Some(e), _) => {
                // 未配置/配置不全：不重试。
                last_err = e;
                break;
            }
            (None, Ok(())) => {
                last_err.clear();
                break;
            }
            (None, Err(e)) => {
                tracing::warn!(channel = chan.as_str(), attempt = attempts, "notify: external send failed");
                last_err = e;
                if attempts >= MAX_ATTEMPTS {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(attempts as u64)).await;
            }
        }
    }
    let summary = DeliverySummary {
        channel: chan.as_str().to_string(),
        ok: last_err.is_empty(),
        error: sanitize(last_err, chan, settings),
        duration_ms: started.elapsed().as_millis() as i64,
        attempts,
    };
    let status = if summary.ok { "success" } else { "failed" };
    if let Err(e) = aidog_db::insert_notification_delivery(
        db,
        action_key,
        &summary.channel,
        status,
        &summary.error,
        summary.duration_ms,
    )
    .await
    {
        tracing::warn!(error = %e, "notify: insert delivery record failed");
    }
    summary
}

/// 配置好的全部外部渠道逐个投递（并发；终态各自落库）。无任何配置 → 空列表零请求。
pub async fn fanout_external(
    db: &Arc<Db>,
    settings: &ExternalChannels,
    title: &str,
    body: &str,
    action_key: &str,
) -> Vec<DeliverySummary> {
    let mut tasks = Vec::new();
    for chan in ExternalChannel::ALL {
        if configured(*chan, settings) {
            let db = db.clone();
            let settings = settings.clone();
            let title = title.to_string();
            let body = body.to_string();
            let action_key = action_key.to_string();
            tasks.push(tokio::spawn(async move {
                deliver_external(&db, *chan, &settings, &title, &body, &action_key).await
            }));
        }
    }
    let mut out = Vec::new();
    for t in tasks {
        match t.await {
            Ok(s) => out.push(s),
            Err(e) => tracing::warn!(error = %e, "notify: channel task panicked"),
        }
    }
    out
}

/// 测试发送（设置页「测试」按钮）：渠道未配置 → Err。
pub async fn test_channel(
    db: &Arc<Db>,
    settings: &ExternalChannels,
    channel: &str,
) -> Result<DeliverySummary, String> {
    let chan =
        ExternalChannel::try_from_str(channel).ok_or_else(|| format!("未知渠道：{channel}"))?;
    if !configured(chan, settings) {
        return Err(format!("渠道 {} 未配置", chan.as_str()));
    }
    Ok(deliver_external(
        db,
        chan,
        settings,
        "AiDog 渠道测试",
        "这是一条测试通知（test send）。",
        "test",
    )
    .await)
}

pub fn configured(chan: ExternalChannel, settings: &ExternalChannels) -> bool {
    match chan {
        ExternalChannel::Webhook => settings.webhook.as_ref().is_some_and(|c| !c.url.is_empty()),
        ExternalChannel::Telegram => settings
            .telegram
            .as_ref()
            .is_some_and(|c| !c.bot_token.is_empty() && !c.chat_id.is_empty()),
        ExternalChannel::Bark => settings.bark.as_ref().is_some_and(|c| !c.url.is_empty()),
        ExternalChannel::Discord => settings.discord.as_ref().is_some_and(|c| !c.url.is_empty()),
        ExternalChannel::Slack => settings.slack.as_ref().is_some_and(|c| !c.url.is_empty()),
        ExternalChannel::Feishu => settings.feishu.as_ref().is_some_and(|c| !c.url.is_empty()),
        ExternalChannel::Email => settings
            .email
            .as_ref()
            .is_some_and(|c| !c.smtp_host.is_empty() && !c.from.is_empty() && !c.to.is_empty()),
    }
}

/// 一次发送。Ok(None) 语义 = 配置存在且发送成功；Err = 发送失败（可重试）；
/// Some(err) = 配置缺失/不全（不重试）。
async fn send_once(
    chan: ExternalChannel,
    settings: &ExternalChannels,
    title: &str,
    body: &str,
) -> (Option<String>, Result<(), String>) {
    match chan {
        ExternalChannel::Webhook => {
            let Some(cfg) = settings.webhook.clone() else { return (Some("webhook 未配置".into()), Ok(())) };
            (None, send_webhook(&cfg, title, body).await)
        }
        ExternalChannel::Telegram => {
            let Some(cfg) = settings.telegram.clone() else { return (Some("telegram 未配置".into()), Ok(())) };
            (None, send_telegram(&cfg, title, body).await)
        }
        ExternalChannel::Bark => {
            let Some(cfg) = settings.bark.clone() else { return (Some("bark 未配置".into()), Ok(())) };
            (None, send_url_webhook("bark", &cfg, json!({"title": title, "body": body})).await)
        }
        ExternalChannel::Discord => {
            let Some(cfg) = settings.discord.clone() else { return (Some("discord 未配置".into()), Ok(())) };
            (None, send_url_webhook("discord", &cfg, json!({"content": format!("{title}\n{body}")})).await)
        }
        ExternalChannel::Slack => {
            let Some(cfg) = settings.slack.clone() else { return (Some("slack 未配置".into()), Ok(())) };
            (None, send_url_webhook("slack", &cfg, json!({"text": format!("{title}\n{body}")})).await)
        }
        ExternalChannel::Feishu => {
            let Some(cfg) = settings.feishu.clone() else { return (Some("feishu 未配置".into()), Ok(())) };
            (None, send_url_webhook("feishu", &cfg, json!({"msg_type": "text", "content": {"text": format!("{title}\n{body}")}})).await)
        }
        ExternalChannel::Email => {
            let Some(cfg) = settings.email.clone() else { return (Some("email 未配置".into()), Ok(())) };
            (None, send_email(&cfg, title, body).await)
        }
    }
}

fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder().timeout(Duration::from_secs(10)).build()
}

/// 通用 webhook：模板（默认 `{"title":"{title}","body":"{body}"}`）做 `{title}`/`{body}`
/// 字面替换后原样 POST（application/json）。模板非法 JSON 不在本层校验——按文本发送，
/// 端点 4xx 即反馈（编辑期 UI 校验归票 04/05）。
async fn send_webhook(cfg: &WebhookCfg, title: &str, body: &str) -> Result<(), String> {
    let tpl = if cfg.template.trim().is_empty() {
        r#"{"title":"{title}","body":"{body}"}"#
    } else {
        cfg.template.as_str()
    };
    let payload = tpl.replace("{title}", title).replace("{body}", body);
    let client = http_client().map_err(|e| format!("build client: {e}"))?;
    let resp = client
        .post(&cfg.url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(payload)
        .send()
        .await
        .map_err(|e| format!("webhook send: {e}"))?;
    status_ok("webhook", resp).await
}

async fn send_telegram(cfg: &TelegramCfg, title: &str, body: &str) -> Result<(), String> {
    let base = if cfg.api_base.is_empty() {
        "https://api.telegram.org"
    } else {
        cfg.api_base.as_str()
    };
    let url = format!("{base}/bot{}/sendMessage", cfg.bot_token);
    let client = http_client().map_err(|e| format!("build client: {e}"))?;
    let resp = client
        .post(&url)
        .json(&json!({"chat_id": cfg.chat_id, "text": format!("{title}\n{body}")}))
        .send()
        .await
        .map_err(|e| format!("telegram send: {}", e.to_string().replace(&cfg.bot_token, "***")))?;
    status_ok("telegram", resp).await
}

/// 纯 URL webhook（bark/discord/slack/feishu 共用：POST JSON）。
async fn send_url_webhook(
    name: &str,
    cfg: &UrlWebhookCfg,
    payload: serde_json::Value,
) -> Result<(), String> {
    let client = http_client().map_err(|e| format!("build client: {e}"))?;
    let resp = client
        .post(&cfg.url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("{name} send: {e}"))?;
    status_ok(name, resp).await
}

async fn send_email(cfg: &EmailCfg, title: &str, body: &str) -> Result<(), String> {
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let msg = Message::builder()
        .from(cfg.from.parse().map_err(|e| format!("email from 非法: {e}"))?)
        .to(cfg.to.parse().map_err(|e| format!("email to 非法: {e}"))?)
        .subject(title)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .map_err(|e| format!("email build: {e}"))?;
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.smtp_host)
        .map_err(|e| format!("email relay: {e}"))?
        .port(if cfg.smtp_port == 0 { 587 } else { cfg.smtp_port });
    if !cfg.username.is_empty() {
        builder = builder.credentials(Credentials::new(
            cfg.username.clone(),
            cfg.password.clone(),
        ));
    }
    let mailer = builder.build();
    mailer
        .send(msg)
        .await
        .map(|_| ())
        .map_err(|e| format!("email send: {}", e.to_string().replace(&cfg.password, "***")))
}

async fn status_ok(name: &str, resp: reqwest::Response) -> Result<(), String> {
    let status = resp.status();
    if status.is_success() {
        Ok(())
    } else {
        let text = resp.text().await.unwrap_or_default();
        Err(format!("{name} HTTP {status}: {}", truncate(&text, 200)))
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n).collect();
        format!("{cut}…")
    }
}

/// 错误信息抹除凭据（落库/回传前）。reqwest 错误常含 URL——telegram URL 内嵌 bot token。
fn sanitize(err: String, chan: ExternalChannel, settings: &ExternalChannels) -> String {
    let mut out = err;
    if chan == ExternalChannel::Telegram
        && let Some(TelegramCfg { bot_token, .. }) = &settings.telegram
        && !bot_token.is_empty()
    {
        out = out.replace(bot_token, "***");
    }
    if chan == ExternalChannel::Email
        && let Some(EmailCfg { password, .. }) = &settings.email
        && !password.is_empty()
    {
        out = out.replace(password, "***");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 极简 HTTP 假端点：收一条请求，返回 status_body。返回 (port, 收到的请求)。
    type Captured = Arc<std::sync::Mutex<Option<(String, String, String)>>>;

    fn fake_endpoint(status_line: &'static str, resp_body: &'static str) -> (u16, Captured) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let captured = Arc::new(std::sync::Mutex::new(None));
        let cap = captured.clone();
        std::thread::spawn(move || {
            // 循环应答（重试会连多次；8 次上限防悬挂线程）
            for _ in 0..8 {
            if let Ok((mut sock, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                // 读到 header 结束，再按 Content-Length 补读 body
                let mut header_end = None;
                loop {
                    let n = sock.read(&mut tmp).unwrap_or(0);
                    if n == 0 { break; }
                    buf.extend_from_slice(&tmp[..n]);
                    if header_end.is_none()
                        && let Some(p) = find(&buf, b"\r\n\r\n")
                    {
                        header_end = Some(p + 4);
                    }
                    if let Some(he) = header_end {
                        let head = String::from_utf8_lossy(&buf[..he]).to_string();
                        let len = head
                            .lines()
                            .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                            .and_then(|l| l.split(':').nth(1))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if buf.len() >= he + len { break; }
                    }
                }
                let he = header_end.unwrap_or(buf.len());
                let head = String::from_utf8_lossy(&buf[..he]).to_string();
                let body = String::from_utf8_lossy(&buf[he..]).to_string();
                let req_line = head.lines().next().unwrap_or_default().to_string();
                *cap.lock().unwrap() = Some((req_line, head, body));
                let resp = format!(
                    "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
                    resp_body.len()
                );
                let _ = sock.write_all(resp.as_bytes());
            } else {
                break;
            }
            }
        });
        (port, captured)
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    // 隔离本机代理环境变量：127.0.0.1 直连（perf-testing §3 同因）。
    fn disable_proxy_env() {
        // edition 2024：set_var 是 unsafe；测试进程单线程写一次可接受。
        unsafe {
            std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
            std::env::set_var("no_proxy", "127.0.0.1,localhost");
        }
    }

    async fn mem_db() -> Arc<Db> {
        let db = Arc::new(Db::new(":memory:").await.unwrap());
        aidog_db::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
            .await
            .unwrap();
        db
    }

    #[tokio::test]
    async fn webhook_payload_shape_and_delivery_record() {
        disable_proxy_env();
        let (port, cap) = fake_endpoint("HTTP/1.1 200 OK", "ok");
        let settings = ExternalChannels {
            webhook: Some(WebhookCfg {
                url: format!("http://127.0.0.1:{port}/hook"),
                template: String::new(),
            }),
            ..Default::default()
        };
        let db = mem_db().await;
        let s = deliver_external(&db, ExternalChannel::Webhook, &settings, "标题", "内容", "act-1").await;
        assert!(s.ok, "{:?}", s.error);
        assert_eq!(s.attempts, 1);
        let (req_line, _head, body) = cap.lock().unwrap().take().unwrap();
        assert!(req_line.starts_with("POST /hook"), "{req_line}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["title"], "标题");
        assert_eq!(v["body"], "内容");
        // 投递记录落库
        let rows = aidog_db::list_notification_deliveries(&db, Some("act-1"), 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].channel, "webhook");
        assert_eq!(rows[0].status, "success");
    }

    #[tokio::test]
    async fn telegram_payload_and_token_never_in_record() {
        disable_proxy_env();
        let (port, cap) = fake_endpoint("HTTP/1.1 200 OK", "{\"ok\":true}");
        let settings = ExternalChannels {
            telegram: Some(TelegramCfg {
                bot_token: "SECRET-TOKEN-123".to_string(),
                chat_id: "-100200".to_string(),
                api_base: format!("http://127.0.0.1:{port}"),
            }),
            ..Default::default()
        };
        let db = mem_db().await;
        let s = deliver_external(&db, ExternalChannel::Telegram, &settings, "T", "B", "act-2").await;
        assert!(s.ok, "{:?}", s.error);
        let (req_line, _head, body) = cap.lock().unwrap().take().unwrap();
        assert!(req_line.starts_with("POST /botSECRET-TOKEN-123/sendMessage"), "{req_line}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["chat_id"], "-100200");
        // 成功路径错误为空；token 只在 URL，不出现在任何记录字段
        assert!(!format!("{s:?}").contains("SECRET-TOKEN-123"));
    }

    #[tokio::test]
    async fn retry_then_success_records_single_final_row() {
        disable_proxy_env();
        // 第一个假端点 500 → 重试换第二个端点 200（退避 1s）
        let (p1, _) = fake_endpoint("HTTP/1.1 500 Internal Server Error", "boom");
        let (p2, cap) = fake_endpoint("HTTP/1.1 200 OK", "ok");
        let settings = ExternalChannels {
            slack: Some(UrlWebhookCfg { url: format!("http://127.0.0.1:{p2}/slack") }),
            discord: Some(UrlWebhookCfg { url: format!("http://127.0.0.1:{p1}/dc") }),
            ..Default::default()
        };
        let db = mem_db().await;
        let out = fanout_external(&db, &settings, "T", "B", "act-3").await;
        assert_eq!(out.len(), 2);
        let slack = out.iter().find(|s| s.channel == "slack").unwrap();
        assert!(slack.ok && slack.attempts == 1);
        let dc = out.iter().find(|s| s.channel == "discord").unwrap();
        assert!(!dc.ok && dc.attempts == MAX_ATTEMPTS);
        assert!(dc.error.contains("500"), "{}", dc.error);
        let _ = cap;
        let rows = aidog_db::list_notification_deliveries(&db, Some("act-3"), 10).await.unwrap();
        assert_eq!(rows.len(), 2); // 每渠道一条终态（discord 3 次尝试只记 1 条）
        assert!(rows.iter().any(|r| r.channel == "discord" && r.status == "failed"));
    }

    #[tokio::test]
    async fn unconfigured_channel_is_bypass() {
        let db = mem_db().await;
        let settings = ExternalChannels::default();
        let out = fanout_external(&db, &settings, "T", "B", "act-4").await;
        assert!(out.is_empty());
        assert!(!configured(ExternalChannel::Telegram, &settings));
        assert!(test_channel(&db, &settings, "telegram").await.is_err());
        assert!(test_channel(&db, &settings, "nope").await.is_err());
    }
}
