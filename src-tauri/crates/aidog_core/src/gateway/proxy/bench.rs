use super::*;

/// ponytail: 量测专用调试端点。SQLite page cache 常驻治理任务的量测脚本靠它驱动三条固定查询
/// 各 N 次（走 app 自己的只读连接池，才能复现被测的 page cache 常驻效果 —— 外部另开进程/连接
/// 打同一 db 文件，cache 会长在别的进程里，测不出本进程 phys_footprint 的变化）。
/// 只读、无副作用，不落 proxy_log，返回逐次耗时（毫秒）供脚本本地算 p95，无需额外解析日志。
#[derive(serde::Deserialize)]
pub(crate) struct BenchQueryReq {
    /// "logs" | "stats" | "balance"
    which: String,
    #[serde(default = "default_n")]
    n: u32,
}

fn default_n() -> u32 {
    30
}

#[derive(serde::Serialize)]
pub(crate) struct BenchQueryResp {
    which: String,
    n: u32,
    /// 每次调用耗时（毫秒），脚本侧自算 p95
    durations_ms: Vec<f64>,
}

pub(crate) async fn handle_bench_query(
    AxumState(state): AxumState<Arc<ProxyState>>,
    Json(req): Json<BenchQueryReq>,
) -> Response {
    let mut durations_ms = Vec::with_capacity(req.n as usize);
    for _ in 0..req.n {
        let t0 = std::time::Instant::now();
        let ok = match req.which.as_str() {
            "logs" => aidog_logs::list_proxy_logs(&state.db, 50, 0).await.is_ok(),
            "stats" => {
                let q = super::models::StatsQuery {
                    start: None,
                    end: None,
                    granularity: None,
                    group_by: None,
                    filter_group: None,
                    filter_model: None,
                    filter_platform: None,
                };
                aidog_stats::query_stats(&state.db, &q).await.is_ok()
            }
            "balance" => aidog_stats::platform_usage_stats_all(&state.db).await.is_ok(),
            _ => {
                let mut r = (StatusCode::BAD_REQUEST, "which must be logs|stats|balance")
                    .into_response();
                inject_trace_header(&mut r);
                return r;
            }
        };
        if !ok {
            tracing::warn!(which = %req.which, "bench-query: query failed, duration still recorded");
        }
        durations_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let mut r = (
        StatusCode::OK,
        Json(BenchQueryResp { which: req.which, n: req.n, durations_ms }),
    )
        .into_response();
    inject_trace_header(&mut r);
    r
}
