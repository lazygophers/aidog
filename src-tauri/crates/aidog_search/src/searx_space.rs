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

/// 单实例可用性探测（用户点行内「测试」时调用，AiDog 从不自动探测）。
/// GET `<url>/search?q=aidog&format=json`（对齐 e_searxng 真实搜索路径），2xx 且响应可解析为
/// JSON 才算可用，返回耗时秒；网络错 / 非 2xx / 非 JSON → Err（带原文）。
pub async fn probe_instance(url: &str) -> Result<f64, String> {
    let u = format!("{}/search?q=aidog&format=json", url.trim_end_matches('/'));
    let t0 = std::time::Instant::now();
    let r = crate::http_client()?
        .get(&u)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("{u}: {e}"))?;
    let status = r.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("{u}: HTTP {status}"));
    }
    let body = r.text().await.map_err(|e| format!("{u}: {e}"))?;
    let _v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("{u}: 非 JSON 响应 {e}"))?;
    Ok(t0.elapsed().as_secs_f64())
}

#[cfg(test)]
mod probe_tests {
    use crate::test_search::spawn_stub;
    use std::sync::Arc;

    #[tokio::test]
    async fn probe_ok_2xx_json_returns_seconds_and_trims_slash() {
        let port = spawn_stub(Arc::new(
            |_m: String, _p: String, _h: Vec<String>, _b: String| (200, "application/json".into(), "{\"results\":[]}".into()),
        ))
        .await;
        let secs = super::probe_instance(&format!("http://127.0.0.1:{port}/")).await.unwrap();
        assert!(secs >= 0.0, "secs={secs}");
    }

    #[tokio::test]
    async fn probe_non_2xx_errs() {
        let port = spawn_stub(Arc::new(
            |_m: String, _p: String, _h: Vec<String>, _b: String| (503, "text/html".into(), "down".into()),
        ))
        .await;
        let e = super::probe_instance(&format!("http://127.0.0.1:{port}")).await.unwrap_err();
        assert!(e.contains("HTTP 503"), "err={e}");
    }

    #[tokio::test]
    async fn probe_200_non_json_errs() {
        let port = spawn_stub(Arc::new(
            |_m: String, _p: String, _h: Vec<String>, _b: String| (200, "text/html".into(), "<html>format=json disabled</html>".into()),
        ))
        .await;
        let e = super::probe_instance(&format!("http://127.0.0.1:{port}")).await.unwrap_err();
        assert!(e.contains("非 JSON"), "err={e}");
    }
}
