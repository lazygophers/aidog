//! 票 05 会话运行状态：tempdir 夹具 pid 文件 + proxy_log 在途行 + 仅活动三组。
//! HOME 经 HomeGuard 指向 tempdir；存活判定注入闭包，不依赖真实进程；now 注入，无挂钟断言。

use std::collections::HashMap;

use super::state::{activity_state, session_states_with};

const NOW: i64 = 1_800_000_000_000;
const MIN: i64 = 60_000;

#[test]
fn activity_thresholds() {
    assert_eq!(activity_state(NOW - 2 * MIN, NOW), "running");
    assert_eq!(activity_state(NOW - 2 * MIN - 1, NOW), "idle");
    assert_eq!(activity_state(NOW - 30 * MIN, NOW), "idle");
    assert_eq!(activity_state(NOW - 30 * MIN - 1, NOW), "stopped");
}

fn pid_file(dir: &std::path::Path, pid: u32, body: &str) {
    std::fs::write(dir.join(format!("{pid}.json")), body).unwrap();
}

fn pid_json(pid: u32, sid: &str, status: &str) -> String {
    serde_json::json!({"pid": pid, "sessionId": sid, "status": status, "procStart": "T"}).to_string()
}

#[tokio::test]
async fn session_states_pid_file_gateway_and_activity() {
    let home = aidog_db::test_support::HomeGuard::new();
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let sessions = home.home().join(".claude").join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    pid_file(&sessions, 100, &pid_json(100, "c-busy", "busy"));
    pid_file(&sessions, 101, &pid_json(101, "c-idle", "idle"));
    pid_file(&sessions, 102, &serde_json::json!({"pid": 102, "sessionId": "c-nokey", "status": "busy"}).to_string());
    pid_file(&sessions, 103, "{not json");
    pid_file(&sessions, 104, &pid_json(104, "c-dead", "busy"));
    pid_file(&sessions, 105, &pid_json(105, "c-idle-gw", "shell"));
    let alive = |pid: u32, ps: &str| ps == "T" && matches!(pid, 100 | 101 | 102 | 105);

    // 网关在途：近 1 分钟 done=0 计入；10 分钟前的孤儿行不计。
    for (id, sid, at) in [("l1", "c-gw", NOW - MIN), ("l2", "c-idle-gw", NOW - MIN), ("l3", "c-old", NOW - 10 * MIN)] {
        let mut log = aidog_db::test_support::sample_log(id, "g", at);
        log.session_id = sid.into();
        log.done = false;
        log.status_code = 0;
        aidog_logs::upsert_proxy_log(&db, log).await.unwrap();
    }

    let mut ids = Vec::new();
    let claude = ["c-busy", "c-idle", "c-nokey", "c-broken", "c-dead", "c-gw", "c-idle-gw", "c-old"];
    for sid in claude {
        aidog_db::upsert_client_session(&db, "claude_code".into(), sid.into(), String::new(), String::new(), 1, NOW - 120 * MIN)
            .await
            .unwrap();
        ids.push(sid.to_string());
    }
    for (sid, last) in [("x-run", NOW - MIN), ("x-idle", NOW - 10 * MIN), ("x-stop", NOW - 120 * MIN)] {
        aidog_db::upsert_client_session(&db, "codex".into(), sid.into(), String::new(), String::new(), 1, last)
            .await
            .unwrap();
        ids.push(sid.to_string());
    }

    let rows = session_states_with(&db, None, &ids, Some(&sessions), NOW, &alive).await.unwrap();
    let got: HashMap<String, (String, String, i64)> = rows
        .into_iter()
        .map(|r| (r.session_id, (r.state, r.source, r.last_activity_ms)))
        .collect();
    let want = |sid: &str, state: &str, source: &str| {
        let g = got.get(sid).unwrap_or_else(|| panic!("缺 {sid}"));
        assert_eq!((g.0.as_str(), g.1.as_str()), (state, source), "{sid}");
    };
    want("c-busy", "running", "pid_file");
    want("c-idle", "idle", "pid_file");
    want("c-idle-gw", "running", "gateway");
    want("c-gw", "running", "gateway");
    want("c-nokey", "stopped", "activity");
    want("c-broken", "stopped", "activity");
    want("c-dead", "stopped", "activity");
    want("c-old", "stopped", "activity");
    want("x-run", "running", "activity");
    want("x-idle", "idle", "activity");
    want("x-stop", "stopped", "activity");
    assert_eq!(got["x-idle"].2, NOW - 10 * MIN, "lastActivityMs 回传供前端派生");
    assert_eq!(got.len(), ids.len());

    // client 过滤：只返回 codex。
    let only = session_states_with(&db, Some("codex"), &ids, Some(&sessions), NOW, &alive).await.unwrap();
    assert!(only.iter().all(|r| r.client_code == "codex") && only.len() == 3);
}
