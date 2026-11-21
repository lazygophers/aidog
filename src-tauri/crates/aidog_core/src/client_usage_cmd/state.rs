//! 会话运行状态（扩展-会话页 票 05，spec D6 / S7）。
//!
//! Claude Code：`~/.claude/sessions/<pid>.json`（非公开格式，本机 2.1.329 实测键 `pid` /
//! `sessionId` / `status` / `procStart`）且进程存活 → busy 执行中、其余空闲；网关 proxy_log
//! 该会话近 5 分钟有 `done=0` 行 → 执行中。文件缺失 / 解析失败 / 键缺失 / 进程不在 → 回落活动判定。
//! 其余三家与回落：`last_activity_ms` 距今 ≤ 2 分钟执行中，≤ 30 分钟空闲，否则已结束。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aidog_db::Db;
use serde::Serialize;

/// 活动判定阈值（spec S7，自定可调；前端 `sessionState.ts` 同值对称）。
pub const RUNNING_WITHIN_MS: i64 = 2 * 60_000;
pub const IDLE_WITHIN_MS: i64 = 30 * 60_000;
/// 网关在途行窗口（perf-testing.md §9：不用整表 done=0）。
const INFLIGHT_WINDOW_MS: i64 = 5 * 60_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStateRow {
    pub client_code: String,
    pub session_id: String,
    /// running / idle / stopped
    pub state: String,
    /// pid_file / gateway / activity
    pub source: String,
    pub last_activity_ms: i64,
}

/// 活动阈值判定（纯函数）。
pub fn activity_state(last_activity_ms: i64, now_ms: i64) -> &'static str {
    let age = now_ms - last_activity_ms;
    if age <= RUNNING_WITHIN_MS {
        "running"
    } else if age <= IDLE_WITHIN_MS {
        "idle"
    } else {
        "stopped"
    }
}

/// 一份 pid 文件的有效内容（四键齐全才算）。
#[derive(Debug, Clone)]
struct PidEntry {
    pid: u32,
    status: String,
    proc_start: String,
}

/// 读 sessions 目录全部 `*.json`，按 sessionId 归集；缺键 / 损坏的文件跳过。
fn read_pid_files(dir: &Path) -> HashMap<String, Vec<PidEntry>> {
    let mut out: HashMap<String, Vec<PidEntry>> = HashMap::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Some(v) = std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        else {
            continue;
        };
        let (Some(pid), Some(sid), Some(status), Some(ps)) = (
            v.get("pid").and_then(|x| x.as_u64()),
            v.get("sessionId").and_then(|x| x.as_str()),
            v.get("status").and_then(|x| x.as_str()),
            v.get("procStart").and_then(|x| x.as_str()),
        ) else {
            continue;
        };
        out.entry(sid.to_string()).or_default().push(PidEntry {
            pid: pid as u32,
            status: status.to_string(),
            proc_start: ps.to_string(),
        });
    }
    out
}

/// 进程存活且启动时刻与 pid 文件一致（防 pid 复用）。
/// 选型：`TZ=UTC ps -p <pid> -o lstart=` 子进程——标准库即可、不新增 libc / nix 依赖；
/// lstart 同时给出存活与启动时刻（Claude Code 写的 procStart 即 UTC 的 lstart 格式，本机实测一致）。
/// ps 不可用（Windows）或进程不在 → false，调用方回落活动判定。
fn proc_alive(pid: u32, proc_start: &str) -> bool {
    let Ok(out) = std::process::Command::new("ps")
        .env("TZ", "UTC")
        .args(["-p", &pid.to_string(), "-o", "lstart="])
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    norm(&String::from_utf8_lossy(&out.stdout)) == norm(proc_start)
}

/// Claude Code pid 文件目录（生产路径）。
pub(super) fn claude_sessions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("sessions"))
}

/// 测试注入面：sessions 目录、now、存活判定均由调用方给定。
pub(super) async fn session_states_with(
    db: &Db,
    client_code: Option<&str>,
    session_ids: &[String],
    claude_sessions_dir: Option<&Path>,
    now_ms: i64,
    alive: &(dyn Fn(u32, &str) -> bool + Send + Sync),
) -> Result<Vec<SessionStateRow>, String> {
    let activity = aidog_db::client_session_activity(db, client_code, session_ids).await?;
    let claude_ids: Vec<String> = activity
        .iter()
        .filter(|(c, _, _)| c == "claude_code")
        .map(|(_, s, _)| s.clone())
        .collect();
    // Claude 专属两路信号：pid 文件 + 网关在途行（无 Claude 会话时不读文件、不查 log.db）。
    let (pid_files, inflight) = if claude_ids.is_empty() {
        Default::default()
    } else {
        let pids = claude_sessions_dir.map(read_pid_files).unwrap_or_default();
        let inflight = aidog_logs::inflight_session_ids(db, &claude_ids, now_ms - INFLIGHT_WINDOW_MS).await?;
        (pids, inflight)
    };
    Ok(activity
        .into_iter()
        .map(|(client_code, session_id, last_activity_ms)| {
            let (state, source) = if client_code == "claude_code" {
                // 同会话可能有多份 pid 文件（resume）：任一存活即算，busy 优先。
                let live: Vec<&PidEntry> = pid_files
                    .get(&session_id)
                    .map(|v| v.iter().filter(|e| alive(e.pid, &e.proc_start)).collect())
                    .unwrap_or_default();
                if live.iter().any(|e| e.status == "busy") {
                    ("running", "pid_file")
                } else if inflight.contains(&session_id) {
                    ("running", "gateway")
                } else if !live.is_empty() {
                    ("idle", "pid_file")
                } else {
                    (activity_state(last_activity_ms, now_ms), "activity")
                }
            } else {
                (activity_state(last_activity_ms, now_ms), "activity")
            };
            SessionStateRow {
                client_code,
                session_id,
                state: state.into(),
                source: source.into(),
                last_activity_ms,
            }
        })
        .collect())
}

crate::tauri_command! {
/// 会话运行状态：每个会话 `{state, source, lastActivityMs}`（spec S7）。
pub async fn client_usage_session_states(
    client_code: Option<String>,
    session_ids: Vec<String>,
) -> Result<Vec<SessionStateRow>, String> {
    let dir = claude_sessions_dir();
    session_states_with(
        aidog_ctx::db(),
        client_code.as_deref(),
        &session_ids,
        dir.as_deref(),
        aidog_db::now(),
        &proc_alive,
    )
    .await
}
}
