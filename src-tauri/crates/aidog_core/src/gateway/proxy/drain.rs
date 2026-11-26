//! 更新排空闸门（票 magpie-05）：确认更新后拒新代理请求、等在途清零再退出。
//!
//! 计数随 `handle_proxy` 生命周期；流式响应延到 body 结束。CONNECT 升级后的 tunnel
//! 生命周期未计入；在此约束补证和实现前，不得宣称 CONNECT 长连接已排空。
use super::{Body, Bytes, Response};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Notify;

#[derive(Default)]
struct DrainState {
    draining: bool,
    inflight: usize,
}

#[derive(Default)]
struct DrainGate {
    state: Mutex<DrainState>,
    empty: Notify,
}

impl DrainGate {
    fn begin(&self) -> bool {
        let mut s = self.state.lock().expect("drain state poisoned");
        if s.draining {
            return false;
        }
        s.draining = true;
        true
    }

    fn abort(&self) -> bool {
        let mut s = self.state.lock().expect("drain state poisoned");
        if !s.draining {
            return false;
        }
        s.draining = false;
        true
    }

    fn admit(self: &Arc<Self>) -> Option<InflightGuard> {
        let mut s = self.state.lock().expect("drain state poisoned");
        if s.draining {
            return None;
        }
        s.inflight += 1;
        Some(InflightGuard(self.clone()))
    }

    fn inflight(&self) -> usize {
        self.state.lock().expect("drain state poisoned").inflight
    }

    async fn wait_empty(&self) {
        loop {
            let notified = self.empty.notified();
            if self.inflight() == 0 {
                return;
            }
            notified.await;
        }
    }
}

fn gate() -> &'static Arc<DrainGate> {
    static GATE: OnceLock<Arc<DrainGate>> = OnceLock::new();
    GATE.get_or_init(|| Arc::new(DrainGate::default()))
}

/// 在途请求 guard：body 完结或请求取消时自动减计数。
pub struct InflightGuard(Arc<DrainGate>);

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let mut s = self.0.state.lock().expect("drain state poisoned");
        s.inflight -= 1;
        let empty = s.inflight == 0;
        drop(s);
        if empty {
            self.0.empty.notify_one();
        }
    }
}

/// 开始排空：此后 [`admit`] 拒绝新代理请求。锁同时串行化入场与排空，避免漏记。
pub fn begin_drain() -> bool {
    gate().begin()
}

/// 解除排空态：放弃本次更新回到正常服务，新请求恢复放行；已入场请求计数不变。
/// 返回 false = 当前不在排空中。
pub fn abort_drain() -> bool {
    gate().abort()
}

/// 新请求入场：排空中返回 None；否则计数 +1 并返回自动释放 guard。
pub fn admit() -> Option<InflightGuard> {
    gate().admit()
}

pub fn inflight() -> usize {
    gate().inflight()
}

/// 等待所有已入场请求及流式响应结束。
pub async fn wait_empty() {
    gate().wait_empty().await
}

/// 将 guard 生命周期从 handler 返回延到响应 body 流结束，不缓冲或改写帧。
pub fn track_response(resp: Response, guard: InflightGuard) -> Response {
    let (parts, body) = resp.into_parts();
    Response::from_parts(
        parts,
        Body::from_stream(TrackedStream {
            inner: body.into_data_stream(),
            _guard: guard,
        }),
    )
}

struct TrackedStream {
    inner: axum::body::BodyDataStream,
    _guard: InflightGuard,
}

impl futures::Stream for TrackedStream {
    type Item = Result<Bytes, axum::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // BodyDataStream: Unpin —— 直接 Pin::new 转发，不引入缓冲。
        Pin::new(&mut self.get_mut().inner).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn abort_releases_drain_and_admits_again() {
        let gate = Arc::new(DrainGate::default());
        assert!(gate.begin());
        assert!(gate.admit().is_none(), "排空中拒绝新请求");
        assert!(gate.abort(), "首次 abort 必须成功");
        assert!(!gate.abort(), "非排空态 abort 返回 false");
        let guard = gate.admit().expect("解除后必须放行");
        assert_eq!(gate.inflight(), 1);
        drop(guard);
        assert_eq!(gate.inflight(), 0);
        // 放弃更新后允许重新发起更新。
        assert!(gate.begin());
    }

    #[tokio::test]
    async fn drain_tracks_requests_and_waits_for_stream_body() {
        let gate = Arc::new(DrainGate::default());
        let guard = gate.admit().expect("非排空态必须放行");
        assert_eq!(gate.inflight(), 1);
        drop(guard);
        assert_eq!(gate.inflight(), 0);

        let guard = gate.admit().unwrap();
        assert!(gate.begin(), "首次 begin 必须成功");
        assert!(!gate.begin(), "重复 begin 返回 false");
        assert!(gate.admit().is_none(), "排空中新请求必须被拒");
        assert_eq!(gate.inflight(), 1, "排空前已入场请求仍计数");

        let response = track_response(
            Response::builder()
                .status(200)
                .header("content-type", "text/event-stream")
                .body(Body::from("hello"))
                .unwrap(),
            guard,
        );
        assert_eq!(gate.inflight(), 1, "body 未消费完 guard 仍持有");
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        let bytes = {
            let mut stream = response.into_body().into_data_stream();
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                bytes.extend_from_slice(&chunk.unwrap());
            }
            bytes
            // stream 在此 drop：guard 藏在 BodyDataStream 内部，随其释放。
        };
        assert_eq!(bytes, b"hello");
        assert_eq!(gate.inflight(), 0, "body 结束后 guard 释放");

        let wait_gate = Arc::new(DrainGate::default());
        let guard = wait_gate.admit().unwrap();
        let mut wait = Box::pin(wait_gate.wait_empty());
        tokio::select! {
            _ = &mut wait => panic!("wait_empty must wait while a request is active"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(1)) => {}
        }
        drop(guard);
        tokio::time::timeout(std::time::Duration::from_secs(1), wait)
            .await
            .expect("guard 释放后 wait_empty 必须返回");
    }
}
