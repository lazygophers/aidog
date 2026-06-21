//! JSON 值提取助手 + 时间戳。供 apply 各子模块共享。

pub(super) fn now_ts() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub(super) fn json_str(v: &serde_json::Value, k: &str) -> String {
    match v.get(k) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

pub(super) fn json_bool(v: &serde_json::Value, k: &str) -> bool {
    v.get(k).and_then(|x| x.as_bool()).unwrap_or(false)
}

pub(super) fn json_i64(v: &serde_json::Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

pub(super) fn json_u32(v: &serde_json::Value, k: &str) -> u32 {
    json_i64(v, k).max(0) as u32
}

pub(super) fn json_u64(v: &serde_json::Value, k: &str) -> u64 {
    json_i64(v, k).max(0) as u64
}

pub(super) fn json_f64(v: &serde_json::Value, k: &str) -> f64 {
    v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)
}
