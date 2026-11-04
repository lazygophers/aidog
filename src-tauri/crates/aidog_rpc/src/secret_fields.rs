//! 密钥信息只写不读（API 票 02，GLOSSARY「密钥信息」）。仅作用于管理接口的 HTTP 形态，桌面 IPC 不经过这里。
//!
//! - 读：响应 JSON 里命中 [`SECRET_PATHS`] 的非空字符串一律换成 [`MASK`]（空值保持空）。
//! - 写：请求里命中声明的值恰为 [`MASK`] = 「保持原值」，从库中取原值回填后再执行命令（见 [`RESTORERS`]）。
use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::middleware::Next;
use axum::response::Response;
use serde_json::Value;

pub const MASK: &str = "***";

/// 密钥字段声明（唯一一处）：JSON 路径后缀；段名忽略大小写与下划线（兼容 camelCase）；数组下标不占段。
pub const SECRET_PATHS: &[&[&str]] = &[
    &["api_key"],                  // 平台 API 密钥
    &["auth_token"],               // 管理接口凭据
    &["bot_token"],                // 通知渠道：Telegram
    &["password"],                 // 通知渠道：Email SMTP / 出站代理
    &["passphrase"],               // 备份 / 导出口令
    &["private_key_pem"],          // MITM 证书私钥
    &["private_key"],
    &["webhook", "url"],           // 通知渠道：webhook 类（URL 即凭据）
    &["bark", "url"],
    &["discord", "url"],
    &["slack", "url"],
    &["feishu", "url"],
];

/// 平台 `extra`（JSON 串）里的密钥：透传平台的认证头配置（入站密钥通道，ADR 0009）。
pub const SECRET_EXTRA_KEYS: &[&str] = &["auth_header", "auth_template"];

fn norm(k: &str) -> String {
    k.chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect()
}

fn is_secret_path(path: &[String]) -> bool {
    SECRET_PATHS.iter().any(|sp| {
        path.len() >= sp.len()
            && path[path.len() - sp.len()..].iter().zip(*sp).all(|(a, b)| *a == norm(b))
    })
}

fn is_extra_secret(k: &str) -> bool {
    SECRET_EXTRA_KEYS.iter().any(|s| norm(s) == norm(k))
}

/// 遍历 `v`，对每个命中的密钥字符串调用 `f(旧新对照键路径, &mut 值)`。`extra` 串内再展开一层。
fn walk(v: &mut Value, path: &mut Vec<String>, f: &mut dyn FnMut(&[String], &mut String)) {
    match v {
        Value::Object(m) => {
            for (k, child) in m.iter_mut() {
                let nk = norm(k);
                path.push(nk.clone());
                match child {
                    Value::String(s) if nk == "extra" => walk_extra(s, path, f),
                    Value::String(s) if is_secret_path(path) => f(path, s),
                    _ => walk(child, path, f),
                }
                path.pop();
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|c| walk(c, path, f)),
        _ => {}
    }
}

fn walk_extra(s: &mut String, path: &mut Vec<String>, f: &mut dyn FnMut(&[String], &mut String)) {
    let Ok(Value::Object(mut m)) = serde_json::from_str::<Value>(s) else { return };
    let mut changed = false;
    for (k, child) in m.iter_mut() {
        if let (true, Value::String(inner)) = (is_extra_secret(k), child) {
            path.push(norm(k));
            let before = inner.clone();
            f(path, inner);
            path.pop();
            changed |= *inner != before;
        }
    }
    if changed {
        *s = Value::Object(m).to_string();
    }
}

/// 响应脱敏：非空密钥 → [`MASK`]。
pub fn mask_value(v: &mut Value) {
    walk(v, &mut Vec::new(), &mut |_, s| {
        if !s.is_empty() {
            *s = MASK.to_string();
        }
    });
}

/// 请求回填：`new` 里值为 [`MASK`] 的密钥字段，取 `old` 同路径的原值。
pub fn restore_value(new: &mut Value, old: &Value) {
    let mut old = old.clone();
    let mut olds: Vec<(Vec<String>, String)> = Vec::new();
    walk(&mut old, &mut Vec::new(), &mut |p, s| olds.push((p.to_vec(), s.clone())));
    // 路径含数组下标不占段，同路径多值时按出现顺序配对。
    let mut used = vec![false; olds.len()];
    walk(new, &mut Vec::new(), &mut |p, s| {
        if s != MASK {
            return;
        }
        if let Some(i) = (0..olds.len()).find(|&i| !used[i] && olds[i].0 == p) {
            used[i] = true;
            *s = olds[i].1.clone();
        }
    });
}

/// 哪些命令的请求要回填，以及去哪取原值：`(命令名, 参数名)`。
/// 新增「带密钥的写命令」时在这里登记，并在 [`load_old`] 补原值来源。
const RESTORERS: &[(&str, &str)] = &[
    ("platform_update", "input"),
    ("notification_settings_set", "settings"),
];

async fn load_old(cmd: &str, arg: &Value) -> Option<Value> {
    let db = aidog_ctx::try_db()?;
    match cmd {
        "platform_update" => {
            let id = arg.get("id")?.as_u64()?;
            let p = aidog_db::get_platform(db, id).await.ok()??;
            serde_json::to_value(p).ok()
        }
        "notification_settings_set" => serde_json::to_value(aidog_db::get_notification_settings(db).await).ok(),
        _ => None,
    }
}

async fn restore_request(cmd: &str, body: &mut Value) {
    let Some((_, arg_name)) = RESTORERS.iter().find(|(c, _)| *c == cmd) else { return };
    let Some(arg) = body.get_mut(*arg_name) else { return };
    if let Some(old) = load_old(cmd, arg).await {
        restore_value(arg, &old);
    }
}

const BODY_LIMIT: usize = 256 * 1024 * 1024;

/// 管理接口 `/rpc/*` 的 HTTP 层：回填请求里的 `***`，脱敏响应里的密钥。
pub async fn guard(req: Request, next: Next) -> Response {
    let cmd = req.uri().path().rsplit('/').next().unwrap_or("").to_string();
    let (parts, body) = req.into_parts();
    let Ok(bytes) = to_bytes(body, BODY_LIMIT).await else {
        return Response::builder().status(413).body(Body::empty()).unwrap();
    };
    let mut parts = parts;
    let bytes = match serde_json::from_slice::<Value>(&bytes) {
        Ok(mut v) => {
            restore_request(&cmd, &mut v).await;
            let out = serde_json::to_vec(&v).unwrap_or_else(|_| bytes.to_vec());
            parts.headers.remove(CONTENT_LENGTH);
            out.into()
        }
        Err(_) => bytes,
    };
    let resp = next.run(Request::from_parts(parts, Body::from(bytes))).await;

    let is_json = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("json"));
    if !is_json {
        return resp;
    }
    let (mut parts, body) = resp.into_parts();
    let Ok(bytes) = to_bytes(body, BODY_LIMIT).await else {
        return Response::builder().status(500).body(Body::empty()).unwrap();
    };
    let out = match serde_json::from_slice::<Value>(&bytes) {
        Ok(mut v) => {
            mask_value(&mut v);
            serde_json::to_vec(&v).unwrap_or_else(|_| bytes.to_vec())
        }
        Err(_) => bytes.to_vec(),
    };
    parts.headers.remove(CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mask_covers_each_secret_kind_and_keeps_empty() {
        let mut v = json!([{
            "api_key": "sk-real", "apiKey": "sk-camel", "base_url": "http://x",
            "extra": "{\"auth_header\":\"x-my-key\",\"auth_template\":\"Bearer {key}\",\"other\":1}",
            "channels": {
                "telegram": {"bot_token": "123:abc", "chat_id": "9"},
                "email": {"password": "pw"},
                "webhook": {"url": "https://h/hook?token=1"},
                "bark": {"url": "https://bark/k"}
            },
            "auth_token": "bearer-1", "passphrase": "pp", "private_key_pem": "-----BEGIN",
            "empty": {"api_key": ""}
        }]);
        mask_value(&mut v);
        let s = v.to_string();
        for leak in ["sk-real", "sk-camel", "123:abc", "\"pw\"", "hook?token", "bark/k", "bearer-1", "\"pp\"", "BEGIN", "x-my-key", "Bearer {key}"] {
            assert!(!s.contains(leak), "{leak} 泄漏: {s}");
        }
        assert_eq!(v[0]["base_url"], "http://x");
        assert_eq!(v[0]["channels"]["telegram"]["chat_id"], "9");
        assert_eq!(v[0]["empty"]["api_key"], "", "空值保持空");
        assert_eq!(v[0]["api_key"], MASK);
        let extra: Value = serde_json::from_str(v[0]["extra"].as_str().unwrap()).unwrap();
        assert_eq!(extra, json!({"auth_header": MASK, "auth_template": MASK, "other": 1}));
    }

    #[test]
    fn restore_fills_mask_from_old_and_keeps_new_values() {
        let old = json!({"api_key": "sk-old", "extra": "{\"auth_header\":\"x-k\"}", "channels": {"webhook": {"url": "https://old"}}});
        let mut new = json!({"api_key": "***", "extra": "{\"auth_header\":\"***\"}", "channels": {"webhook": {"url": "https://new"}}});
        restore_value(&mut new, &old);
        assert_eq!(new["api_key"], "sk-old");
        assert_eq!(new["channels"]["webhook"]["url"], "https://new", "第三方写入新值照常生效");
        let extra: Value = serde_json::from_str(new["extra"].as_str().unwrap()).unwrap();
        assert_eq!(extra["auth_header"], "x-k");
    }
}
