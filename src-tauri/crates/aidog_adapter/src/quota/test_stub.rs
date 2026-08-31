//! cfg(test) 共享 mock：axum stub server（quota-scripts T4 等价测试用）。
//! - [`spawn_stub`]：全路径固定 status + body（沿用 test_http.rs 先例）。
//! - [`spawn_capture`]：按 path 前缀路由的捕获 stub，记录每个请求的 path（含 query）
//!   与 Authorization 头，供断言 headers / 两步查询顺序 / instance_root 剥版本。

use std::sync::{Arc, Mutex};

/// 捕获到的一次出站请求。
pub(super) struct Recorded {
    /// path + query（如 `/api/usage/token/?key=sk&api_key=sk`）
    pub path: String,
    /// Authorization 头原值（未发送则为空串）
    pub authorization: String,
}

/// 固定 status + body 的 fallback stub，返回根 URL（如 `http://127.0.0.1:PORT`）。
pub(super) async fn spawn_stub(status: u16, body: &'static str) -> String {
    spawn_capture(vec![("/", status, body)]).await.0
}

/// 路由捕获 stub：path 前缀命中（首个匹配）返回对应 (status, body)，未命中 404。
/// 返回根 URL 与请求记录（含每个请求的 Authorization）。
pub(super) async fn spawn_capture(
    routes: Vec<(&'static str, u16, &'static str)>,
) -> (String, Arc<Mutex<Vec<Recorded>>>) {
    use axum::extract::Request;
    use axum::routing::any;
    let log: Arc<Mutex<Vec<Recorded>>> = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    let app = axum::Router::new().fallback(any(move |req: Request| {
        let log = log2.clone();
        async move {
            let uri = req.uri().clone();
            let auth = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            log.lock().unwrap().push(Recorded {
                path: uri
                    .path_and_query()
                    .map(|q| q.as_str().to_string())
                    .unwrap_or_default(),
                authorization: auth,
            });
            for (prefix, status, body) in &routes {
                if uri.path().starts_with(prefix) {
                    return (
                        axum::http::StatusCode::from_u16(*status).unwrap(),
                        [("content-type", "application/json")],
                        *body,
                    );
                }
            }
            (
                axum::http::StatusCode::NOT_FOUND,
                [("content-type", "application/json")],
                "{}",
            )
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (format!("http://{addr}"), log)
}
