//! `model_entry` / `platform_preset` 两表的读写层（migration 20260826-01）。
//!
//! 真值源是 `defaults/registry/`：远程同步（票 T3）把 registry 逐文件 upsert 进这两表，
//! DB 空（还没同步过 / 新装）时读取层回落编译期内置的 registry 快照。
//! 「DB 优先、bundled 兜底」与旧 `resolve_price` 的约定一致，兜底只读不写。

use super::*;
use crate::models::{ModelEntry, ModelEntryGroup, ModelInfoSnapshot, PlatformPreset};
use rusqlite::{params, OptionalExtension, Result as SqlResult};
use std::sync::OnceLock;

const MODEL_ENTRY_COLUMNS: &str = "platform_code, model_id, canonical_model, family, version, predecessor, capabilities, builtin_tools_excluded, max_input_tokens, max_output_tokens, context_window, official, price_data, updated_at, display_name";

/// JSON 数组文本 → `Vec<String>`。非数组 / 解析失败 → 空（列有 DEFAULT '[]'，脏值不该阻断读取）。
fn json_str_array(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

/// 展示名回落（票 T10）：缺省 / 空串 / 纯空白 → `model_id`。
/// 只在读取层调用，写入层原样存 registry 值。
fn resolve_display_name(raw: &str, model_id: &str) -> String {
    match raw.trim() {
        "" => model_id.to_string(),
        s => s.to_string(),
    }
}

fn row_to_model_entry(row: &rusqlite::Row) -> SqlResult<ModelEntry> {
    let model_id: String = row.get(1)?;
    Ok(ModelEntry {
        platform_code: row.get(0)?,
        display_name: resolve_display_name(&row.get::<_, String>(14)?, &model_id),
        model_id,
        canonical_model: row.get(2)?,
        family: row.get(3)?,
        version: row.get(4)?,
        predecessor: row.get(5)?,
        capabilities: json_str_array(&row.get::<_, String>(6)?),
        builtin_tools_excluded: json_str_array(&row.get::<_, String>(7)?),
        max_input_tokens: row.get(8)?,
        max_output_tokens: row.get(9)?,
        context_window: row.get(10)?,
        official: row.get::<_, i64>(11)? != 0,
        price_data: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

/// registry 模型 JSON → 行形状。`price_data` 保留整份原文，缺省字段落空值；
/// `canonical_model` 缺省回落 `model_id`（聚合键必须非空）。`model_id` 缺失 → None（跳过该文件）。
/// `display_name` **不在此回落**——这是写入路径，缺省即空串入库，回落在读取层。
pub fn model_entry_from_json(platform_code: &str, raw: &str) -> Option<ModelEntry> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let model_id = v.get("model_id")?.as_str()?.to_string();
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let list = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    let canonical_model = match text("canonical_model") {
        s if s.is_empty() => model_id.clone(),
        s => s,
    };
    Some(ModelEntry {
        platform_code: platform_code.to_string(),
        model_id,
        display_name: text("display_name"),
        canonical_model,
        family: text("family"),
        version: text("version"),
        predecessor: text("predecessor"),
        capabilities: list("capabilities"),
        builtin_tools_excluded: list("builtin_tools_excluded"),
        max_input_tokens: v.get("max_input_tokens").and_then(|x| x.as_i64()),
        max_output_tokens: v.get("max_output_tokens").and_then(|x| x.as_i64()),
        context_window: v.get("context_window").and_then(|x| x.as_i64()),
        official: v.get("official").and_then(|x| x.as_bool()).unwrap_or(false),
        price_data: raw.to_string(),
        updated_at: 0,
    })
}

static BUNDLED_ENTRIES: OnceLock<Vec<ModelEntry>> = OnceLock::new();
static BUNDLED_PRESETS: OnceLock<Vec<PlatformPreset>> = OnceLock::new();

/// 编译期内置 registry 的模型条目快照，`(platform_code, model_id)` 升序。首次访问解析一次。
/// 这是读取路径（DB 空兜底），故此处补上 `display_name` 回落，与 DB 行返回同一契约。
pub fn bundled_model_entries() -> &'static [ModelEntry] {
    BUNDLED_ENTRIES.get_or_init(|| {
        let mut out: Vec<ModelEntry> = crate::registry::bundled_model_files()
            .iter()
            .filter_map(|(code, _file, raw)| model_entry_from_json(code, raw))
            .map(|mut e| {
                e.display_name = resolve_display_name(&e.display_name, &e.model_id);
                e
            })
            .collect();
        out.sort_by(|a, b| (&a.platform_code, &a.model_id).cmp(&(&b.platform_code, &b.model_id)));
        out
    })
}

/// 编译期内置 registry 的平台预设快照，`code` 升序。
pub fn bundled_platform_presets() -> &'static [PlatformPreset] {
    BUNDLED_PRESETS.get_or_init(|| {
        let mut out: Vec<PlatformPreset> = crate::registry::bundled_platform_files()
            .iter()
            .map(|(code, raw)| PlatformPreset {
                code: (*code).to_string(),
                preset_data: (*raw).to_string(),
                updated_at: 0,
            })
            .collect();
        out.sort_by(|a, b| a.code.cmp(&b.code));
        out
    })
}

/// 按 `canonical_model` 聚合。代表平台优先取 `official = true` 的那条，
/// 否则取 `platform_code` 字典序第一条（entries 已按该序排好，取首即可）。
/// 输入顺序不作要求，输出按 `canonical_model` 升序、组内按 `platform_code` 升序。
pub fn group_by_canonical(mut entries: Vec<ModelEntry>) -> Vec<ModelEntryGroup> {
    entries.sort_by(|a, b| {
        (&a.canonical_model, &a.platform_code, &a.model_id)
            .cmp(&(&b.canonical_model, &b.platform_code, &b.model_id))
    });
    let mut out: Vec<ModelEntryGroup> = Vec::new();
    for e in entries {
        match out.last_mut() {
            Some(g) if g.canonical_model == e.canonical_model => g.entries.push(e),
            _ => out.push(ModelEntryGroup {
                canonical_model: e.canonical_model.clone(),
                display_name: String::new(),
                primary_platform: String::new(),
                entries: vec![e],
            }),
        }
    }
    for g in &mut out {
        // 代表条目同时决定 primary_platform 与聚合行展示名（票 T10：官方那条的展示名）。
        let primary = g.entries.iter().find(|e| e.official).or_else(|| g.entries.first());
        g.primary_platform = primary.map(|e| e.platform_code.clone()).unwrap_or_default();
        g.display_name = primary
            .map(|e| resolve_display_name(&e.display_name, &e.model_id))
            .unwrap_or_else(|| g.canonical_model.clone());
    }
    out
}

/// 批量 upsert 模型条目（单事务）。主键 `(platform_code, model_id)` 冲突即整行覆盖并复活软删。
/// 入参的 `updated_at` 被忽略，统一写当前时间。
/// `display_name` 原样入库（registry 缺省即空串），回落只在读取层做。
#[track_caller]
pub fn upsert_model_entries(db: &Db, entries: Vec<ModelEntry>) -> impl std::future::Future<Output = Result<u32, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let ts = now();
        db.call_traced(None, __db_caller, move |conn| {
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO model_entry (platform_code, model_id, canonical_model, family, version, predecessor,
                        capabilities, builtin_tools_excluded, max_input_tokens, max_output_tokens, context_window,
                        official, price_data, created_at, updated_at, deleted_at, display_name)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, 0, ?15)
                     ON CONFLICT(platform_code, model_id) DO UPDATE SET
                       canonical_model = ?3, family = ?4, version = ?5, predecessor = ?6,
                       capabilities = ?7, builtin_tools_excluded = ?8,
                       max_input_tokens = ?9, max_output_tokens = ?10, context_window = ?11,
                       official = ?12, price_data = ?13, updated_at = ?14, display_name = ?15, deleted_at = 0",
                )?;
                for e in &entries {
                    stmt.execute(params![
                        e.platform_code,
                        e.model_id,
                        e.canonical_model,
                        e.family,
                        e.version,
                        e.predecessor,
                        serde_json::to_string(&e.capabilities).unwrap_or_else(|_| "[]".into()),
                        serde_json::to_string(&e.builtin_tools_excluded).unwrap_or_else(|_| "[]".into()),
                        e.max_input_tokens,
                        e.max_output_tokens,
                        e.context_window,
                        i64::from(e.official),
                        e.price_data,
                        ts,
                        e.display_name,
                    ])?;
                }
            }
            let n = entries.len() as u32;
            tx.commit()?;
            Ok(n)
        })
        .await
        .map_err(|e| format!("upsert model entries: {e}"))
    }
}

/// 批量 upsert 平台预设（单事务）。`preset_data` 整份覆盖——同步失败的平台由调用方
/// 直接不传，DB 旧行原样保留（票 12 的 best-effort 语义靠「不调用」实现，不靠部分字段合并）。
#[track_caller]
pub fn upsert_platform_presets(db: &Db, presets: Vec<PlatformPreset>) -> impl std::future::Future<Output = Result<u32, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let ts = now();
        db.call_traced(None, __db_caller, move |conn| {
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO platform_preset (code, preset_data, created_at, updated_at, deleted_at)
                     VALUES (?1, ?2, ?3, ?3, 0)
                     ON CONFLICT(code) DO UPDATE SET preset_data = ?2, updated_at = ?3, deleted_at = 0",
                )?;
                for p in &presets {
                    stmt.execute(params![p.code, p.preset_data, ts])?;
                }
            }
            let n = presets.len() as u32;
            tx.commit()?;
            Ok(n)
        })
        .await
        .map_err(|e| format!("upsert platform presets: {e}"))
    }
}

#[track_caller]
pub fn count_model_entries(db: &Db) -> impl std::future::Future<Output = Result<u32, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_traced(None, __db_caller, move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM model_entry WHERE deleted_at = 0", [], |r| r.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 列模型条目裸行（无 bundled 兜底）：`platform_code` 为 None 即全量。
/// 同步的「新增 / 更新 / 未变」分类必须走这条——带兜底的 [`list_model_entries`] 在空表时
/// 会返回 bundled 快照，会把首次同步的全部条目误判成「未变」。
#[track_caller]
pub fn select_model_entries<'a>(db: &'a Db, platform_code: Option<&'a str>) -> impl std::future::Future<Output = Result<Vec<ModelEntry>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let code = platform_code.map(str::to_string);
        db.call_read_traced(None, __db_caller, move |conn| {
            let base = format!("SELECT {MODEL_ENTRY_COLUMNS} FROM model_entry WHERE deleted_at = 0");
            let order = " ORDER BY platform_code, model_id";
            match &code {
                Some(c) => {
                    let mut stmt = conn.prepare(&format!("{base} AND platform_code = ?1{order}"))?;
                    Ok(stmt.query_map(params![c], row_to_model_entry)?.collect::<SqlResult<Vec<_>>>()?)
                }
                None => {
                    let mut stmt = conn.prepare(&format!("{base}{order}"))?;
                    Ok(stmt.query_map([], row_to_model_entry)?.collect::<SqlResult<Vec<_>>>()?)
                }
            }
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 列模型条目：`platform_code` 为 None 即全量。DB 无任何条目 → 回落 bundled registry。
/// 本函数不带 `#[track_caller]`（DB 访问在 `select_model_entries` / `count_model_entries`
/// 内各自记 caller），故用 `async fn` 而非本模块其余处的 `impl Future` idiom。
pub async fn list_model_entries(db: &Db, platform_code: Option<&str>) -> Result<Vec<ModelEntry>, String> {
    let rows = select_model_entries(db, platform_code).await?;
    if !rows.is_empty() {
        return Ok(rows);
    }
    // 空结果分两种：DB 整表空（未同步）→ bundled 兜底；表非空只是该平台没有 → 照实返回空。
    if count_model_entries(db).await? > 0 {
        return Ok(rows);
    }
    Ok(bundled_model_entries()
        .iter()
        .filter(|e| platform_code.is_none_or(|c| e.platform_code == c))
        .cloned()
        .collect())
}

/// 单条模型条目（按平台 + 真实请求名）。DB 未命中且整表空 → 回落 bundled registry。
#[track_caller]
pub fn get_model_entry<'a>(db: &'a Db, platform_code: &'a str, model_id: &'a str) -> impl std::future::Future<Output = Result<Option<ModelEntry>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let (code, id) = (platform_code.to_string(), model_id.to_string());
        let hit: Option<ModelEntry> = db
            .call_read_traced(None, __db_caller, move |conn| {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {MODEL_ENTRY_COLUMNS} FROM model_entry WHERE platform_code = ?1 AND model_id = ?2 AND deleted_at = 0"
                ))?;
                Ok(stmt.query_row(params![code, id], row_to_model_entry).optional()?)
            })
            .await
            .map_err(|e| e.to_string())?;
        if hit.is_some() || count_model_entries(db).await? > 0 {
            return Ok(hit);
        }
        Ok(bundled_model_entries()
            .iter()
            .find(|e| e.platform_code == platform_code && e.model_id == model_id)
            .cloned())
    }
}

/// 列平台预设裸行（无 bundled 兜底）。空 = DB 从未同步过。
#[track_caller]
pub fn select_platform_presets(db: &Db) -> impl std::future::Future<Output = Result<Vec<PlatformPreset>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT code, preset_data, updated_at FROM platform_preset WHERE deleted_at = 0 ORDER BY code",
            )?;
            Ok(stmt
                .query_map([], |r| {
                    Ok(PlatformPreset { code: r.get(0)?, preset_data: r.get(1)?, updated_at: r.get(2)? })
                })?
                .collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 列平台预设。DB 空 → 回落 bundled registry。
pub async fn list_platform_presets(db: &Db) -> Result<Vec<PlatformPreset>, String> {
    let rows = select_platform_presets(db).await?;
    if rows.is_empty() {
        return Ok(bundled_platform_presets().to_vec());
    }
    Ok(rows)
}

/// 旧 `platform-presets.json` 形状的整篇文档（`get_defaults_json` 的数据源）。
/// DB 有同步数据即用 DB（`last_updated` 取各行 `updated_at` 最大值，秒），
/// 从未同步过则原样回落编译期内置的那份（含 registry 自带的 version / last_updated）。
pub async fn presets_doc_json(db: &Db) -> Result<String, String> {
    let rows = select_platform_presets(db).await?;
    if rows.is_empty() {
        return Ok(crate::registry::presets_json().to_string());
    }
    let last_updated = rows.iter().map(|r| r.updated_at).max().unwrap_or(0) / 1000;
    let version = crate::registry::presets()["version"].clone();
    Ok(crate::registry::presets_doc(
        rows.iter().map(|r| (r.code.as_str(), r.preset_data.as_str())),
        version,
        serde_json::Value::from(last_updated),
    )
    .to_string())
}

/// 模型信息页一次性数据源：模型维度聚合行 + 全部平台预设（含品牌字段）。
/// `bundled = true` 表示模型条目来自编译期内置 registry（DB 尚未同步）。
pub async fn model_info_snapshot(db: &Db) -> Result<ModelInfoSnapshot, String> {
    let bundled = count_model_entries(db).await? == 0;
    Ok(ModelInfoSnapshot {
        groups: group_by_canonical(list_model_entries(db, None).await?),
        platforms: list_platform_presets(db).await?,
        bundled,
    })
}
