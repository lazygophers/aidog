//! 客户端本地会话用量：同意开关、读取导入编排与命令（issue #65；claude_code 票 02、codex 票 04）。
//!
//! 闸门唯一性：同意开关是读取器唯一入口判定（`consent_allows`）；关着 = 零文件读取。
//! 首次开启全量导入、启动/refresh 增量导入共用同一条 `run_import` 链。

mod agents;
pub mod carry;
pub mod reader;
pub mod reader_codex;
pub mod reader_gemini;
pub mod reader_opencode;
pub mod state;
pub mod watch;
#[cfg(test)]
#[path = "test_reader.rs"]
mod test_reader;
#[cfg(test)]
#[path = "test_reader_codex.rs"]
mod test_reader_codex;
#[cfg(test)]
#[path = "test_reader_gemini.rs"]
mod test_reader_gemini;
#[cfg(test)]
#[path = "test_watch.rs"]
mod test_watch;
#[cfg(test)]
#[path = "test_reader_opencode.rs"]
mod test_reader_opencode;
#[cfg(test)]
#[path = "plain_sqlite_probe.rs"]
mod plain_sqlite_probe;
#[cfg(test)]
#[path = "test_settings.rs"]
mod test_settings;
#[cfg(test)]
#[path = "test_agents.rs"]
mod test_agents;
#[cfg(test)]
#[path = "test_state.rs"]
mod test_state;
#[cfg(test)]
#[path = "test_carry.rs"]
mod test_carry;

/// 全局 IMPORT_STATE 只有一份，跨 reader 两个测试模块的用例用它串行化。
#[cfg(test)]
pub(crate) static TEST_IMPORT_STATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use aidog_db::ClientCallRow;
use serde::{Deserialize, Serialize};

/// setting 载荷：`{asked, enabled: {<client_code>: bool}}`（scope `client_usage`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientUsageConsent {
    pub asked: bool,
    pub enabled: HashMap<String, bool>,
}

/// 支持的客户端大类码（gemini_cli 读取器票 05 已落；opencode 在票 06）。
pub const SUPPORTED_CLIENTS: &[&str] = &["claude_code", "codex", "gemini_cli", "opencode"];

/// 读同意开关（缺省全关、未询问）。
pub async fn load_consent(db: &aidog_db::Db) -> ClientUsageConsent {
    aidog_db::get_setting(db, "client_usage", "client_usage_consent")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// 闸门判定：读取器入口唯一判定。
pub fn consent_allows(c: &ClientUsageConsent, code: &str) -> bool {
    c.enabled.get(code).copied().unwrap_or(false)
}

pub async fn save_consent(db: &aidog_db::Db, consent: &ClientUsageConsent) -> Result<(), String> {
    aidog_db::set_setting(
        db,
        aidog_db::SetSettingInput {
            scope: "client_usage".into(),
            key: "client_usage_consent".into(),
            value: serde_json::to_value(consent).map_err(|e| e.to_string())?,
        },
    )
    .await
}

/// 「允许读取对话原文」开关（scope `client_usage` / key `transcript_consent`，缺省 false）。
/// 与 `consent_allows` 闸门分开：原文只供换客户端继续时读取，不改读取器入口判定。
pub async fn load_transcript_consent(db: &aidog_db::Db) -> bool {
    aidog_db::get_setting(db, "client_usage", "transcript_consent")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub async fn save_transcript_consent(db: &aidog_db::Db, on: bool) -> Result<(), String> {
    aidog_db::set_setting(
        db,
        aidog_db::SetSettingInput {
            scope: "client_usage".into(),
            key: "transcript_consent".into(),
            value: serde_json::json!(on),
        },
    )
    .await
}

/// 导入进度（内存态；持久进度在 client_file_progress 表）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportState {
    pub running: bool,
    pub client_code: String,
    pub started_at_ms: i64,
    pub files_total: i64,
    pub files_done: i64,
    pub calls_imported: i64,
    pub parse_errors: i64,
    pub finished_at_ms: i64,
    pub error: String,
}

static IMPORT_STATE: OnceLock<Mutex<Option<ImportState>>> = OnceLock::new();

fn state_slot() -> &'static Mutex<Option<ImportState>> {
    IMPORT_STATE.get_or_init(Default::default)
}

pub fn set_import_state(s: Option<ImportState>) {
    *state_slot().lock().unwrap_or_else(|e| e.into_inner()) = s;
}

pub fn snapshot_import_state() -> Option<ImportState> {
    state_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// 客户端会话文件根（生产路径；CODEX_HOME 优先于 ~/.codex）。其余客户端读取器落地后补根。
pub(crate) fn client_root(code: &str) -> Option<std::path::PathBuf> {
    match code {
        "claude_code" => dirs::home_dir().map(|h| h.join(".claude").join("projects")),
        "codex" => {
            let home = std::env::var("CODEX_HOME").ok().map(std::path::PathBuf::from);
            let home = home.or_else(dirs::home_dir)?;
            Some(home.join("sessions"))
        }
        "gemini_cli" => dirs::home_dir().map(|h| h.join(".gemini").join("tmp")),
        "opencode" => reader_opencode::preferred_root(),
        _ => None,
    }
}

/// 枚举客户端会话文件（递归全部子目录，按路径排序）。
fn enumerate_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// 折算用平台码：客户端 → 官方平台（codex → openai、gemini_cli → gemini；
/// opencode → 空串：模型按名匹配，`model_entry_for_billing` 平台 miss 走
/// any-platform 官方优先查找，不固定单一平台）。
fn billing_platform(client_code: &str) -> &'static str {
    match client_code {
        "codex" => "openai",
        "claude_code" => "anthropic",
        "gemini_cli" => "gemini",
        _ => "",
    }
}

/// 折算：客户端官方平台条目优先，缺失时跨平台取官方条目（`model_entry_for_billing` 内建）；
/// 条目缺失或解析落 fallback（条目无有效价）→ None = 未定价，不回落兜底单价。
/// is_peak=false：会话文件无平台倍率语境，按默认官方价折算。
async fn resolve_cost(db: &aidog_db::Db, platform: &str, model: &str, r: &reader::UsageLine) -> Option<f64> {
    if model.is_empty() {
        return None;
    }
    let entry = aidog_db::model_entry_for_billing(db, platform, model).await.ok()??;
    let res = aidog_db::resolve_price_from(
        Some(&entry.price_data),
        false,
        0.0,
        0.0,
        r.input_tokens,
        r.call_time_ms,
    );
    if res.price.source == "fallback" {
        return None;
    }
    let p = &res.price;
    Some(
        r.input_tokens as f64 * p.input_cost_per_token
            + r.output_tokens as f64 * p.output_cost_per_token
            + r.cache_read_tokens as f64 * p.cache_read_input_token_cost
            + r.cache_write_tokens as f64 * p.cache_write_input_token_cost,
    )
}

/// 单文件处理：增量起点 → 扫描 → 分批入库 → 推进进度 / 会话行。
/// 返回 (新增明细数, 新进度, 新解析错误累计)。截断 → 删状态重读。
async fn scan_one_file(
    db: &aidog_db::Db,
    client_code: &str,
    path: &std::path::Path,
    prog: Option<aidog_db::ClientFileProgress>,
    sessions_root: &std::path::Path,
) -> Result<(i64, aidog_db::ClientFileProgress), String> {
    let path_str = path.to_string_lossy().into_owned();
    let mut start = prog;
    if let Some(s) = &start
        && let Ok(meta) = std::fs::metadata(path)
    {
        // 截断 / 替换：文件比已读偏移小，或 mtime 回退 → 明细重建（删明细与进度重读）。
        if (meta.len() as i64) < s.read_offset || meta_mtime_ms(&meta) < s.mtime_ms {
            aidog_db::delete_client_file_state(db, client_code, &path_str).await?;
            start = None;
        }
    }
    let (offset, last_id) = start
        .as_ref()
        .map(|s| (s.read_offset as u64, s.last_msg_id.clone()))
        .unwrap_or((0, String::new()));
    let (path2, off2, id2) = (path.to_path_buf(), offset, last_id.clone());
    let root2 = sessions_root.to_path_buf();
    let scanned = match client_code {
        "codex" => {
            tokio::task::spawn_blocking(move || reader_codex::scan_file(&path2, off2, &id2, Some(&root2)))
                .await
                .map_err(|e| format!("scan task: {e}"))?
        }
        "gemini_cli" => {
            tokio::task::spawn_blocking(move || reader_gemini::scan_file(&path2, off2, &id2))
                .await
                .map_err(|e| format!("scan task: {e}"))?
        }
        _ => tokio::task::spawn_blocking(move || reader::scan_file(&path2, off2, &id2))
            .await
            .map_err(|e| format!("scan task: {e}"))?,
    };
    let out = match scanned {
        Ok(o) => o,
        Err(reader::ScanErr::Truncated) => {
            // 扫描中又被截：删状态重读一次。
            aidog_db::delete_client_file_state(db, client_code, &path_str).await?;
            let p2 = path.to_path_buf();
            let r2 = sessions_root.to_path_buf();
            let rescan = match client_code {
                "codex" => tokio::task::spawn_blocking(move || reader_codex::scan_file(&p2, 0, "", Some(&r2)))
                    .await
                    .map_err(|e| format!("rescan task: {e}"))?,
                "gemini_cli" => tokio::task::spawn_blocking(move || reader_gemini::scan_file(&p2, 0, ""))
                    .await
                    .map_err(|e| format!("rescan task: {e}"))?,
                _ => tokio::task::spawn_blocking(move || reader::scan_file(&p2, 0, ""))
                    .await
                    .map_err(|e| format!("rescan task: {e}"))?,
            };
            rescan
                .map_err(|e| format!("scan {}: {e}", path.display()))?
        }
        Err(reader::ScanErr::Io(e)) => return Err(format!("scan {}: {e}", path.display())),
    };
    // 折算 + 组装行。
    let mut rows = Vec::with_capacity(out.entries.len());
    for (i, r) in out.entries.iter().enumerate() {
        let cost = resolve_cost(db, billing_platform(client_code), &r.model, r).await;
        rows.push(ClientCallRow {
            client_code: client_code.to_string(),
            session_id: r.session_id.clone(),
            is_sidechain: r.is_sidechain,
            call_time_ms: r.call_time_ms,
            model: r.model.clone(),
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_read_tokens: r.cache_read_tokens,
            cache_write_tokens: r.cache_write_tokens,
            reasoning_tokens: r.reasoning_tokens,
            cost,
            source_file_id: path_str.clone(),
            file_pos: out.positions[i],
        });
    }
    // 分批提交（500/批）。
    let mut inserted = 0i64;
    for chunk in rows.chunks(500) {
        inserted += aidog_db::insert_client_calls(db, chunk).await? as i64;
    }
    // 会话标题：transcript 标题行 > 已存标题（upsert 空标题保留现值）> 首条用户提示 60 字。
    let title = out
        .title
        .map(|t| t.1)
        .or(out.first_user_prompt.clone())
        .unwrap_or_default();
    for sid in [&out.session_id] {
        if sid.is_empty() {
            continue;
        }
        aidog_db::upsert_client_session(
            db,
            client_code.to_string(),
            sid.to_string(),
            title.clone(),
            out.workdir.clone(),
            out.first_ms,
            out.last_ms,
        )
        .await?;
    }
    let prog = aidog_db::ClientFileProgress {
        client_code: client_code.to_string(),
        path: path_str.clone(),
        file_size: out.file_size,
        mtime_ms: out.mtime_ms,
        read_offset: out.new_offset as i64,
        parse_errors: out.parse_errors as i64 + start.as_ref().map_or(0, |s| s.parse_errors),
        last_msg_id: out.last_msg_id,
    };
    aidog_db::upsert_client_file_progress(db, prog.clone()).await?;
    Ok((inserted, prog))
}

fn meta_mtime_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// opencode 单文件处理：进度未变 → None（跳过）；变了 → 删状态全量重读
/// （消息文件与 opencode.db 都可能原地改写，幂等插入挡不住 token 更新，必须删重读）。
pub(crate) async fn scan_opencode_path(
    db: &aidog_db::Db,
    path: &std::path::Path,
    root: &std::path::Path,
    prog: Option<aidog_db::ClientFileProgress>,
) -> Result<Option<i64>, String> {
    let path_str = path.to_string_lossy().into_owned();
    let unchanged = std::fs::metadata(path).ok().is_some_and(|m| {
        prog.as_ref().is_some_and(|p| {
            p.file_size == m.len() as i64 && p.mtime_ms == meta_mtime_ms(&m)
        })
    });
    if unchanged {
        return Ok(None);
    }
    if prog.is_some() {
        aidog_db::delete_client_file_state(db, "opencode", &path_str).await?;
    }
    let (path2, root2) = (path.to_path_buf(), root.to_path_buf());
    let out = tokio::task::spawn_blocking(move || reader_opencode::scan(&path2, &root2))
        .await
        .map_err(|e| format!("scan task: {e}"))??;
    // 折算：平台码空串 → model_entry_for_billing 走 any-platform 官方优先按名匹配。
    let mut rows = Vec::with_capacity(out.entries.len());
    for (i, r) in out.entries.iter().enumerate() {
        let cost = resolve_cost(db, "", &r.model, r).await;
        rows.push(ClientCallRow {
            client_code: "opencode".to_string(),
            session_id: r.session_id.clone(),
            is_sidechain: r.is_sidechain,
            call_time_ms: r.call_time_ms,
            model: r.model.clone(),
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_read_tokens: r.cache_read_tokens,
            cache_write_tokens: r.cache_write_tokens,
            reasoning_tokens: r.reasoning_tokens,
            cost,
            source_file_id: path_str.clone(),
            file_pos: out.positions[i],
        });
    }
    let mut inserted = 0i64;
    for chunk in rows.chunks(500) {
        inserted += aidog_db::insert_client_calls(db, chunk).await? as i64;
    }
    for s in &out.sessions {
        aidog_db::upsert_client_session(
            db,
            "opencode".to_string(),
            s.id.clone(),
            aidog_db::truncate_name(&s.title, 60),
            s.workdir.clone(),
            s.first_ms,
            s.last_ms,
        )
        .await?;
    }
    aidog_db::upsert_client_file_progress(
        db,
        aidog_db::ClientFileProgress {
            client_code: "opencode".to_string(),
            path: path_str,
            file_size: out.file_size,
            mtime_ms: out.mtime_ms,
            read_offset: out.file_size,
            parse_errors: out.parse_errors as i64,
            last_msg_id: String::new(),
        },
    )
    .await?;
    Ok(Some(inserted))
}

/// opencode 导入（db 单文件或 JSON store 消息文件集）。`only_with_progress` 同
/// `run_import` 语义（启动增量只读进度表已有文件）。
async fn run_import_opencode(
    db: &aidog_db::Db,
    root: &std::path::Path,
    only_with_progress: bool,
) -> Result<i64, String> {
    let mut paths = reader_opencode::enumerate(root);
    let progress = aidog_db::list_client_file_progress(db, "opencode").await?;
    let prog_map: HashMap<String, aidog_db::ClientFileProgress> =
        progress.into_iter().map(|p| (p.path.clone(), p)).collect();
    if only_with_progress {
        paths.retain(|p| prog_map.contains_key(&p.to_string_lossy().into_owned()));
    }
    set_import_state(Some(ImportState {
        running: true,
        client_code: "opencode".to_string(),
        started_at_ms: aidog_db::now(),
        files_total: paths.len() as i64,
        ..Default::default()
    }));
    let mut total = 0i64;
    for path in &paths {
        let path_str = path.to_string_lossy().into_owned();
        let prog = prog_map.get(&path_str).cloned();
        match scan_opencode_path(db, path, root, prog).await {
            Ok(Some(n)) => {
                total += n;
                bump_state(|s| {
                    s.files_done += 1;
                    s.calls_imported += n;
                });
            }
            Ok(None) => bump_state(|s| s.files_done += 1),
            Err(e) => bump_state(|s| {
                s.files_done += 1;
                s.error = e;
            }),
        }
    }
    bump_state(|s| {
        s.running = false;
        s.finished_at_ms = aidog_db::now();
    });
    Ok(total)
}

async fn run_import(
    db: &aidog_db::Db,
    root: Option<std::path::PathBuf>,
    clients: &[String],
    only_with_progress: bool,
) -> Result<i64, String> {
    let consent = load_consent(db).await;
    let total: &mut i64 = &mut 0;
    for code in clients {
        // 闸门：关着 = 该客户端零文件读取（枚举也不做）。
        if !consent_allows(&consent, code) {
            continue;
        }
        let Some(root) = root.clone().or_else(|| client_root(code)) else {
            continue;
        };
        // opencode 走专用链（db 单文件 / JSON store 消息文件集，与 jsonl 文件语义不同）。
        if code == "opencode" {
            match run_import_opencode(db, &root, only_with_progress).await {
                Ok(n) => *total += n,
                Err(e) => set_import_state(Some(ImportState {
                    running: false,
                    client_code: code.clone(),
                    error: e,
                    finished_at_ms: aidog_db::now(),
                    ..Default::default()
                })),
            }
            continue;
        }
        let progress = aidog_db::list_client_file_progress(db, code).await?;
        let mut paths: Vec<std::path::PathBuf> = if only_with_progress {
            progress.iter().map(|p| std::path::PathBuf::from(&p.path)).collect()
        } else {
            enumerate_files(&root)
        };
        // 启动模式只读进度表已有文件；refresh 模式补充枚举（含新文件，从 0 读）。
        if only_with_progress {
            let known: std::collections::HashSet<String> =
                progress.iter().map(|p| p.path.clone()).collect();
            paths.retain(|p| known.contains(&p.to_string_lossy().into_owned()));
        }
        let prog_map: HashMap<String, aidog_db::ClientFileProgress> =
            progress.into_iter().map(|p| (p.path.clone(), p)).collect();
        set_import_state(Some(ImportState {
            running: true,
            client_code: code.clone(),
            started_at_ms: aidog_db::now(),
            files_total: paths.len() as i64,
            ..Default::default()
        }));
        for path in &paths {
            let path_str = path.to_string_lossy().into_owned();
            let prog = prog_map.get(&path_str).cloned();
            match scan_one_file(db, code, path, prog, &root).await {
                Ok((n, _)) => {
                    *total += n;
                    bump_state(|s| {
                        s.files_done += 1;
                        s.calls_imported += n;
                    });
                }
                Err(e) => {
                    bump_state(|s| {
                        s.files_done += 1;
                        s.error = e;
                    });
                }
            }
        }
        bump_state(|s| {
            s.running = false;
            s.finished_at_ms = aidog_db::now();
        });
    }
    Ok(*total)
}

fn bump_state(f: impl FnOnce(&mut ImportState)) {
    if let Some(mut cur) = snapshot_import_state() {
        f(&mut cur);
        set_import_state(Some(cur));
    }
}

/// 启动增量导入（只读进度表已有文件；consent 关着的客户端直接跳过）。调用方 spawn。
pub async fn startup_incremental_import() -> Result<i64, String> {
    let db = aidog_ctx::db();
    let consent = load_consent(db).await;
    let clients: Vec<String> = SUPPORTED_CLIENTS
        .iter()
        .filter(|c| consent_allows(&consent, c))
        .map(|c| c.to_string())
        .collect();
    run_import(db, None, &clients, true).await
}

/// refresh：枚举全部文件（新文件从 0 读），增量推进（有进度从偏移读）。
pub async fn refresh_import() -> Result<i64, String> {
    let db = aidog_ctx::db();
    let consent = load_consent(db).await;
    let clients: Vec<String> = SUPPORTED_CLIENTS
        .iter()
        .filter(|c| consent_allows(&consent, c))
        .map(|c| c.to_string())
        .collect();
    run_import(db, None, &clients, false).await
}

/// 首次开启单客户端的全量后台导入。
pub async fn full_import_client(client: &str) -> Result<i64, String> {
    let db = aidog_ctx::db();
    let consent = load_consent(db).await;
    if !consent_allows(&consent, client) {
        return Ok(0);
    }
    let clients = vec![client.to_string()];
    run_import(db, None, &clients, false).await
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUsageStatus {
    pub consent: ClientUsageConsent,
    pub import: ImportState,
    pub persisted_parse_errors: i64,
}

crate::tauri_command! {
/// 同意开关读取。
pub async fn client_usage_consent_get() -> Result<ClientUsageConsent, String> {
    Ok(load_consent(aidog_ctx::db()).await)
}
}

crate::tauri_command! {
/// 同意开关写入；新开启的客户端触发一次后台全量导入。
pub async fn client_usage_consent_set(consent: ClientUsageConsent) -> Result<(), String> {
    let db = aidog_ctx::db();
    let prev = load_consent(db).await;
    save_consent(db, &consent).await?;
    let newly: Vec<String> = SUPPORTED_CLIENTS
        .iter()
        .filter(|c| {
            consent_allows(&consent, c)
                && !consent_allows(&prev, c)
        })
        .map(|c| c.to_string())
        .collect();
    if !newly.is_empty() {
        tokio::spawn(async move {
            if let Err(e) = run_import(aidog_ctx::db(), None, &newly, false).await {
                tracing::warn!(error = %e, "client usage full import failed");
            }
        });
    }
    // 票 03：监听面随同意开关同步（新开启 → 挂 watch，关闭 → 摘除）。
    watch::sync_watchers(db).await;
    Ok(())
}
}

crate::tauri_command! {
/// 状态：同意开关 + 导入进度 + 持久化解析错误累计。
pub async fn client_usage_status() -> Result<ClientUsageStatus, String> {
    let db = aidog_ctx::db();
    let consent = load_consent(db).await;
    let mut persisted = 0i64;
    for code in SUPPORTED_CLIENTS {
        if let Ok(rows) = aidog_db::list_client_file_progress(db, code).await {
            for r in rows {
                persisted += r.parse_errors;
            }
        }
    }
    Ok(ClientUsageStatus {
        import: snapshot_import_state().unwrap_or_default(),
        persisted_parse_errors: persisted,
        consent,
    })
}
}

crate::tauri_command! {
/// 触发一次对齐 + 增量读取（后台跑，进度走 client_usage_status）。
pub async fn client_usage_refresh() -> Result<ImportState, String> {
    set_import_state(None);
    tokio::spawn(async move {
        if let Err(e) = refresh_import().await {
            tracing::warn!(error = %e, "client usage refresh failed");
        }
    });
    Ok(snapshot_import_state().unwrap_or_default())
}
}

crate::tauri_command! {
/// 汇总查询。
pub async fn client_usage_summary(
    from_ms: i64,
    to_ms: i64,
    client_code: Option<String>,
) -> Result<aidog_db::ClientUsageSummary, String> {
    aidog_db::client_usage_summary(aidog_ctx::db(), from_ms, to_ms, client_code.as_deref()).await
}
}

crate::tauri_command! {
/// 按日趋势查询。
pub async fn client_usage_trend(
    from_ms: i64,
    to_ms: i64,
    client_code: Option<String>,
) -> Result<Vec<aidog_db::ClientUsageTrendRow>, String> {
    aidog_db::client_usage_trend(aidog_ctx::db(), from_ms, to_ms, client_code.as_deref()).await
}
}

crate::tauri_command! {
/// 按模型聚合查询。
pub async fn client_usage_by_model(
    from_ms: i64,
    to_ms: i64,
    client_code: Option<String>,
) -> Result<Vec<aidog_db::ClientUsageModelRow>, String> {
    aidog_db::client_usage_by_model(aidog_ctx::db(), from_ms, to_ms, client_code.as_deref()).await
}
}

crate::tauri_command! {
/// 会话列表查询。
pub async fn client_usage_sessions(
    from_ms: i64,
    to_ms: i64,
    client_code: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<aidog_db::ClientUsageSessionRow>, String> {
    aidog_db::client_usage_sessions(
        aidog_ctx::db(),
        from_ms,
        to_ms,
        client_code.as_deref(),
        limit.unwrap_or(200),
    )
    .await
}
}

crate::tauri_command! {
/// 从会话列表隐藏（票 09）：只记隐藏标记，不删客户端文件与用量明细。
pub async fn client_usage_session_hide(client_code: String, session_id: String) -> Result<(), String> {
    aidog_db::hide_client_session(aidog_ctx::db(), &client_code, &session_id).await
}
}

crate::tauri_command! {
/// 从回收站恢复到会话列表（票 09）。
pub async fn client_usage_session_unhide(client_code: String, session_id: String) -> Result<(), String> {
    aidog_db::unhide_client_session(aidog_ctx::db(), &client_code, &session_id).await
}
}

crate::tauri_command! {
/// 回收站列表（票 09）：已隐藏会话 + hidden_at；区间只约束聚合口径，缺省全量。
pub async fn client_usage_hidden_sessions(
    from_ms: Option<i64>,
    to_ms: Option<i64>,
) -> Result<Vec<aidog_db::ClientUsageHiddenSessionRow>, String> {
    aidog_db::client_usage_hidden_sessions(aidog_ctx::db(), from_ms.unwrap_or(0), to_ms.unwrap_or(i64::MAX)).await
}
}

crate::tauri_command! {
/// 单会话详情：模型链（相邻同模型合并）+ token 时间序列 + 主线 / 子代理分组。
/// `part`（票 12）：points / chain / agents 只读对应部分，缺省全部。
pub async fn client_usage_session_detail(
    client_code: String,
    session_id: String,
    part: Option<String>,
) -> Result<aidog_db::ClientUsageSessionDetail, String> {
    let mut d = aidog_db::client_usage_session_detail(aidog_ctx::db(), &client_code, &session_id, part.as_deref()).await?;
    d.agents = agents::group_agents(&client_code, std::mem::take(&mut d.files));
    Ok(d)
}
}

crate::tauri_command! {
/// 客户端用量保留天数（0 = 永久，缺省 90）。
pub async fn client_usage_retention_get() -> Result<u32, String> {
    Ok(aidog_db::client_usage::load_retention_days(aidog_ctx::db()).await)
}
}

crate::tauri_command! {
/// 设置保留天数，并立即按新值清理一次。
pub async fn client_usage_retention_set(days: u32) -> Result<(), String> {
    let db = aidog_ctx::db();
    aidog_db::client_usage::save_retention_days(db, days).await?;
    aidog_db::cleanup_client_usage(db, days).await
}
}

crate::tauri_command! {
/// 「允许读取对话原文」开关读取（缺省 false）。
pub async fn client_usage_transcript_consent_get() -> Result<bool, String> {
    Ok(load_transcript_consent(aidog_ctx::db()).await)
}
}

crate::tauri_command! {
/// 「允许读取对话原文」开关写入；不改读取闸门（`consent_allows`）。
pub async fn client_usage_transcript_consent_set(enabled: bool) -> Result<(), String> {
    save_transcript_consent(aidog_ctx::db(), enabled).await
}
}
