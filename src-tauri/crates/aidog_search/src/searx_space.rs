//! searx.space 公共实例列表（只在用户点「刷新公共列表」时拉一次，AiDog 从不自动拉取）。
use serde::{Deserialize, Serialize};

pub const SEARX_INSTANCES_URL: &str = "https://searx.space/data/instances.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearxInstance {
    pub url: String,
    pub seconds: Option<f64>,
}

/// instances.json → 健康 https 实例（http 200、搜索成功率 >= 80%），按 searx.space 实测搜索耗时升序（同脚本 _searx_healthy）。
pub fn healthy_instances(json: &str) -> Vec<SearxInstance> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let mut out: Vec<SearxInstance> = v["instances"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(url, info)| {
            url.starts_with("https://")
                && info["http"]["status_code"].as_i64() == Some(200)
                && info["timing"]["search"]["success_percentage"].as_f64().unwrap_or(0.0) >= 80.0
        })
        .map(|(url, info)| {
            let all = &info["timing"]["search"]["all"];
            SearxInstance { url: url.clone(), seconds: all["value"].as_f64().or_else(|| all.as_f64()) }
        })
        .collect();
    // 稳定排序：耗时未知排最后
    out.sort_by(|a, b| a.seconds.unwrap_or(9999.0).total_cmp(&b.seconds.unwrap_or(9999.0)));
    out
}

/// 手动刷新：GET 一次 searx.space 列表。仅由管理命令（用户点按钮）调用，AiDog 不自动拉取。
pub async fn fetch_searx_instances(url: &str) -> Result<Vec<SearxInstance>, String> {
    let r = crate::http_client()?.get(url).send().await.map_err(|e| format!("searx.space: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("searx.space: HTTP {}", r.status().as_u16()));
    }
    let body = r.text().await.map_err(|e| format!("searx.space: {e}"))?;
    let v = healthy_instances(&body);
    if v.is_empty() {
        return Err("searx.space 无健康实例".into());
    }
    Ok(v)
}
