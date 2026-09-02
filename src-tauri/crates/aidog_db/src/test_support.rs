//! 测试共享 helper（自原 db.rs 单一 tests 模块拆出，pub 供各 tests_* 子模块复用）。
use crate::Db;
use crate::models::*;
use rusqlite::params;

/// HOME / CODEX_HOME 是进程全局，所有触 FS（写 ~/.aidog、~/.claude、~/.codex）的测试必须
/// 串行在 **同一把** 锁上，跨模块共享，避免并行线程互相覆盖。
pub static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// HOME / CODEX_HOME / CLAUDE_CONFIG_DIR 指向 tempdir 的 RAII 守卫；Drop 时恢复原值。
/// 构造时持有 ENV_LOCK（跨模块共享，所有 env-mutating 测试串行）。
/// 吸收了 skills/test_list 的 EnvGuard 语义：CLAUDE_CONFIG_DIR 默认设为 tempdir 内的
/// `.claude`（与 HOME/.claude 同一物理目录），与原 EnvGuard「remove CLAUDE_CONFIG_DIR」不同，
/// 但 test_list 的 global 测试本来就把 ~/.claude 建在 HOME 下，等价。
pub struct HomeGuard {
    pub dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
    prev_home: Option<String>,
    prev_codex: Option<String>,
    prev_claude_cfg: Option<String>,
    prev_pi: Option<String>,
}
impl Default for HomeGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeGuard {
    pub fn new() -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let prev_home = std::env::var("HOME").ok();
        let prev_codex = std::env::var("CODEX_HOME").ok();
        let prev_claude_cfg = std::env::var("CLAUDE_CONFIG_DIR").ok();
        // pi 的 agent 目录优先认 PI_CODING_AGENT_DIR，不隔离它则开发机上设了该变量的测试
        // 会写进真实 ~/.pi/agent。
        let prev_pi = std::env::var("PI_CODING_AGENT_DIR").ok();
        std::fs::create_dir_all(dir.path().join(".codex")).unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::create_dir_all(dir.path().join(".pi").join("agent")).unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
            std::env::set_var("CODEX_HOME", dir.path().join(".codex"));
            std::env::set_var("CLAUDE_CONFIG_DIR", dir.path().join(".claude"));
            std::env::set_var("PI_CODING_AGENT_DIR", dir.path().join(".pi").join("agent"));
        }
        Self {
            dir,
            _lock: lock,
            prev_home,
            prev_codex,
            prev_claude_cfg,
            prev_pi,
        }
    }
    pub fn home(&self) -> &std::path::Path {
        self.dir.path()
    }
}
impl Drop for HomeGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match &self.prev_codex {
                Some(v) => std::env::set_var("CODEX_HOME", v),
                None => std::env::remove_var("CODEX_HOME"),
            }
            match &self.prev_claude_cfg {
                Some(v) => std::env::set_var("CLAUDE_CONFIG_DIR", v),
                None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
            }
            match &self.prev_pi {
                Some(v) => std::env::set_var("PI_CODING_AGENT_DIR", v),
                None => std::env::remove_var("PI_CODING_AGENT_DIR"),
            }
        }
    }
}

/// 创建一个初始化好的内存库
pub async fn test_db() -> Db {
    let db = Db::new(":memory:").await.expect("open memory db");
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_conn, _map| Ok(())))
        .await
        .expect("init tables");
    db
}

pub fn sample_platform(name: &str) -> CreatePlatform {
    CreatePlatform {
        name: name.to_string(),
        platform_type: Protocol::Anthropic,
        base_url: "https://example.com".to_string(),
        api_key: "sk-test".to_string(),
        extra: String::new(),
        models: None,
        available_models: None,
        endpoints: None,
        manual_budgets: None,
        auto_group: None,
        join_group_ids: None,
        expires_at: None,
    }
}

pub fn sample_group(name: &str, mappings: Vec<ModelMapping>) -> CreateGroup {
    CreateGroup {
        name: name.to_string(),
        group_key: Some(name.to_string()),
        routing_mode: RoutingMode::Failover,
        auto_from_platform: String::new(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
        source_protocol: None,
        max_retries: 2,
        model_mappings: mappings,
        env_vars: Vec::new(),
    }
}

pub fn sample_log(id: &str, group_key: &str, created_at: i64) -> ProxyLog {
    ProxyLog {
        id: id.to_string(),
        group_key: group_key.to_string(),
        model: "claude-sonnet-4".to_string(),
        actual_model: "glm-4-plus".to_string(),
        source_protocol: "anthropic".to_string(),
        target_protocol: "openai".to_string(),
        platform_id: 1,
        request_headers: String::new(),
        request_body: String::new(),
        upstream_request_headers: String::new(),
        upstream_request_body: String::new(),
        response_body: String::new(),
        request_url: String::new(),
        upstream_request_url: String::new(),
        upstream_response_headers: String::new(),
        upstream_status_code: 200,
        user_response_headers: String::new(),
        user_response_body: String::new(),
        status_code: 200,
        duration_ms: 100,
        input_tokens: 10,
        output_tokens: 20,
        cache_tokens: 0,
        est_cost: 0.0,
        is_stream: false,
        attempts: Vec::new(),
        retry_count: 0,
        blocked_by: String::new(),
        blocked_reason: String::new(),
        created_at,
        updated_at: created_at,
        deleted_at: 0,
        cli_proxy_provider_id: None,
        done: true,
        field_trace: String::new(),
    }
}

/// 插入一行最小 platform（platform.db），返回 id。跨 crate 测试用：
/// aidog_stats 等不能反向 dev-dep aidog_core（成环），替代 create_platform 场景。
pub async fn insert_test_platform(db: &Db, name: &str) -> u64 {
    let name = name.to_string();
    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
            conn.execute(
                "INSERT INTO platform (name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at, manual_budgets, expires_at) VALUES (?1, '\"anthropic\"', 'https://example.com', 'sk-test', '{}', '{}', '{}', '[]', 1, 0, 0, '', 0)",
                params![name],
            )?;
            Ok(conn.last_insert_rowid() as u64)
        })
        .await
        .expect("insert test platform")
}

/// 插入一行最小 group（platform.db，其余列走 DDL 默认值），返回 group_key。
/// 同 insert_test_platform：跨 crate 测试替代 create_group（auto 分组归属场景只需行本身）。
pub async fn insert_test_group(
    db: &Db,
    name: &str,
    group_key: Option<&str>,
    auto_from_platform: &str,
) -> String {
    let key = group_key.unwrap_or(name).to_string();
    let (name, auto, key_clone) = (
        name.to_string(),
        auto_from_platform.to_string(),
        key.clone(),
    );
    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT INTO \"group\" (name, group_key, auto_from_platform) VALUES (?1, ?2, ?3)",
            params![name, key_clone, auto],
        )?;
        Ok(())
    })
    .await
    .expect("insert test group");
    key
}

// ─── DB Vacuum/Hard-delete (Tier 1 + Tier 2) ───────────────

/// 辅助：插入一行 proxy_log（指定 created_at），返回 id。
pub async fn insert_proxy_log_at(db: &Db, created_at: i64) -> String {
    let id = format!("test-{created_at}");
    let id_clone = id.clone();
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
                     status_code, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, \
                     created_at, deleted_at) \
                     VALUES (?1, 0, '', 'm', 'anthropic', 200, 10, 5, 0, 0.001, 0, ?2, 0)",
            params![id_clone, created_at],
        )?;
        Ok(())
    })
    .await
    .expect("insert test proxy_log");
    id
}

/// 辅助：COUNT(*) FROM proxy_log（含 tombstone，不过滤 deleted_at）。
pub async fn count_all_proxy_logs(db: &Db) -> i64 {
    db.call_traced(None, std::panic::Location::caller(), |conn| {
        Ok(conn.query_row("SELECT COUNT(*) FROM proxy_log", [], |r| r.get(0))?)
    })
    .await
    .unwrap()
}
