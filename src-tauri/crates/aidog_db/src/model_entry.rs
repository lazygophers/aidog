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

/// 读取层展示名回落：整行原样带出，只把 `display_name` 补成非空。
/// 导出（`select_model_entries`）**不走这里**，否则回落值会被写死进备份。
fn with_display_name(mut e: ModelEntry) -> ModelEntry {
    e.display_name = resolve_display_name(&e.display_name, &e.model_id);
    e
}

/// 裸行映射：`display_name` 原样带出（可能是空串）。回落由 [`with_display_name`] 在
/// 面向 UI 的读取入口上做。
fn row_to_model_entry(row: &rusqlite::Row) -> SqlResult<ModelEntry> {
    let model_id: String = row.get(1)?;
    Ok(ModelEntry {
        platform_code: row.get(0)?,
        display_name: row.get(14)?,
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

/// 读取层价格结构归一化：DB 未重同步的旧形状行（价格平铺顶层）→ `price` 子树，
/// 前端展示只认新形状。前置 `contains` 过滤让已是新形状的行（绝大多数）零解析开销。
/// 导出（`select_model_entries`）**不走这里**——同步的内容比较与备份必须拿原文。
fn with_price_normalized(mut e: ModelEntry) -> ModelEntry {
    if !e.price_data.contains("input_cost_per_token") {
        return e;
    }
    // 脏 JSON 保持原文，前端 parsePriceData 自有「无价格」回落
    if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&e.price_data)
        && crate::price_resolve::legacy_price_into(&mut v)
    {
        e.price_data = v.to_string();
    }
    e
}

/// UI 读取入口的统一出口：展示名回落 + 价格结构归一化（计费路径在
/// `price_resolve::parse_price_data` 各自归一化，不走这里）。
fn ui_entry(e: ModelEntry) -> ModelEntry {
    with_price_normalized(with_display_name(e))
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

/// 按 `canonical_model` 聚合。代表平台按「可选平台优先、其中再 `official` 优先」挑：
/// `pricing_only` 里的 code（纯协议豁免，没有 `platform.json`，用户选不到）
/// 不能当聚合行的代表平台（票 13-I）。全组都是 pricing_only 时才退给它们。
/// 2026-08-31 起该清单为空，机制保留供纯协议条目复用。
/// 输入顺序不作要求，输出按 `canonical_model` 升序、组内按 `platform_code` 升序。
pub fn group_by_canonical(
    mut entries: Vec<ModelEntry>,
    pricing_only: &std::collections::HashSet<String>,
) -> Vec<ModelEntryGroup> {
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
        let selectable = |e: &&ModelEntry| !pricing_only.contains(&e.platform_code);
        let primary = g
            .entries
            .iter()
            .find(|e| e.official && selectable(e))
            .or_else(|| g.entries.iter().find(selectable))
            .or_else(|| g.entries.iter().find(|e| e.official))
            .or_else(|| g.entries.first());
        g.primary_platform = primary.map(|e| e.platform_code.clone()).unwrap_or_default();
        g.display_name = primary
            .map(|e| resolve_display_name(&e.display_name, &e.model_id))
            .unwrap_or_else(|| g.canonical_model.clone());
    }
    out
}

/// 一行写失败的记账：`(registry 内文件定位, 错误)`。整轮同步不因一行脏数据全丢。
pub type WriteFailure = (String, String);

/// 批量 upsert 模型条目（单事务）。整批必须全成功，任一行失败即整批回滚并 Err。
/// 同步路径请用 [`upsert_model_entries_best_effort`]。
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

/// best-effort 版：单行写失败只记账并继续，其余行照常提交（票 13-F）。
/// 返回 `(成功行数, 失败清单)`；整个事务打不开才 Err。
#[track_caller]
pub fn upsert_model_entries_best_effort(
    db: &Db,
    entries: Vec<ModelEntry>,
) -> impl std::future::Future<Output = Result<(u32, Vec<WriteFailure>), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let ts = now();
        db.call_traced(None, __db_caller, move |conn| {
            let tx = conn.transaction()?;
            let mut ok = 0u32;
            let mut failed: Vec<WriteFailure> = Vec::new();
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
                    let r = stmt.execute(params![
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
                    ]);
                    match r {
                        Ok(_) => ok += 1,
                        Err(err) => failed
                            .push((format!("{}/{}", e.platform_code, e.model_id), err.to_string())),
                    }
                }
            }
            tx.commit()?;
            Ok((ok, failed))
        })
        .await
        .map_err(|e| format!("upsert model entries: {e}"))
    }
}

/// 批量 upsert 平台预设。`preset_data` 整份覆盖——同步失败的平台由调用方直接不传，
/// DB 旧行原样保留（票 12 的 best-effort 语义靠「不调用」实现，不靠部分字段合并）。
/// 单行写失败只记账并继续（票 13-F）。返回 `(成功行数, 失败清单)`。
///
/// 写完作废 preset 缓存：热路径的 `effective_presets()` 会退回 bundled，
/// 直到调用方跑 [`refresh_presets_cache`] 把新值装回去。
#[track_caller]
pub fn upsert_platform_presets(
    db: &Db,
    presets: Vec<PlatformPreset>,
) -> impl std::future::Future<Output = Result<(u32, Vec<WriteFailure>), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let ts = now();
        let out = db
            .call_traced(None, __db_caller, move |conn| {
                let tx = conn.transaction()?;
                let mut ok = 0u32;
                let mut failed: Vec<WriteFailure> = Vec::new();
                {
                    let mut stmt = tx.prepare(
                        "INSERT INTO platform_preset (code, preset_data, created_at, updated_at, deleted_at)
                         VALUES (?1, ?2, ?3, ?3, 0)
                         ON CONFLICT(code) DO UPDATE SET preset_data = ?2, updated_at = ?3, deleted_at = 0",
                    )?;
                    for p in &presets {
                        match stmt.execute(params![p.code, p.preset_data, ts]) {
                            Ok(_) => ok += 1,
                            Err(e) => failed.push((p.code.clone(), e.to_string())),
                        }
                    }
                }
                tx.commit()?;
                Ok((ok, failed))
            })
            .await
            .map_err(|e| format!("upsert platform presets: {e}"))?;
        crate::registry::invalidate_presets_cache();
        Ok(out)
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
        return Ok(rows.into_iter().map(ui_entry).collect());
    }
    // 空结果分两种：DB 整表空（未同步）→ bundled 兜底；表非空只是该平台没有 → 照实返回空。
    if count_model_entries(db).await? > 0 {
        return Ok(Vec::new());
    }
    Ok(bundled_model_entries()
        .iter()
        .filter(|e| platform_code.is_none_or(|c| e.platform_code == c))
        .cloned()
        .collect())
}

/// bundled 快照里按主键找一条（切片已按 `(platform_code, model_id)` 升序，二分即可）。
fn bundled_entry(platform_code: &str, model_id: &str) -> Option<&'static ModelEntry> {
    let all = bundled_model_entries();
    all.binary_search_by(|e| {
        (e.platform_code.as_str(), e.model_id.as_str()).cmp(&(platform_code, model_id))
    })
    .ok()
    .map(|i| &all[i])
}

/// 单条模型条目（按平台 + 真实请求名）。**逐键**回落 bundled registry：DB 查不到该键就查
/// bundled，而不是「整表为空才兜底」——首次同步只要成功一个文件表就非空，按整表判空会让
/// 那批拉失败的模型在下一轮成功前一直掉进 fallback 单价（票 13-E）。
/// 顺带去掉了每次 miss 的全表 `COUNT(*)`（票 13-G：这是计费热路径）。
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
        Ok(hit
            .or_else(|| bundled_entry(platform_code, model_id).cloned())
            .map(ui_entry))
    }
}

/// 跨平台价格回退：本平台没有该模型的条目时，按 `model_id` 取任一平台上的条目，
/// `official = true` 优先，其次 `platform_code` 字典序。
///
/// 中转镜像类平台（claude_code / packycode / aihubmix / newapi …，registry 里
/// `models/` 目录为空的那 50 个）靠这条恢复旧 `resolve_price` 的
/// 「`pricing[platform]` → 顶层单价」回退链：没有它，这些平台的每一次请求都按
/// fallback 单价计费（`claude-sonnet-4-5` 输出价 $15 → $3），余额扣减长期偏高。
#[track_caller]
pub fn get_model_entry_any_platform<'a>(db: &'a Db, model_id: &'a str) -> impl std::future::Future<Output = Result<Option<ModelEntry>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let id = model_id.to_string();
        let hit: Option<ModelEntry> = db
            .call_read_traced(None, __db_caller, move |conn| {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {MODEL_ENTRY_COLUMNS} FROM model_entry WHERE model_id = ?1 AND deleted_at = 0
                     ORDER BY official DESC, platform_code LIMIT 1"
                ))?;
                Ok(stmt.query_row(params![id], row_to_model_entry).optional()?)
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(hit
            .or_else(|| {
                let all = bundled_model_entries();
                all.iter()
                    .find(|e| e.model_id == model_id && e.official)
                    .or_else(|| all.iter().find(|e| e.model_id == model_id))
                    .cloned()
            })
            .map(ui_entry))
    }
}

/// 计费 / 出站裁剪共用的条目查找：本平台条目优先，缺失时跨平台取官方条目。
/// 返回的 `bool` = 是否走了跨平台回退（价格 `source` 据此区分）。
pub async fn model_entry_for_billing(
    db: &Db,
    platform_code: &str,
    model_id: &str,
) -> Result<Option<(ModelEntry, bool)>, String> {
    if let Some(e) = get_model_entry(db, platform_code, model_id).await? {
        return Ok(Some((e, false)));
    }
    Ok(get_model_entry_any_platform(db, model_id).await?.map(|e| (e, true)))
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

/// 列平台预设：**DB 行与 bundled 取并集**，同 code 以 DB 行为准（票 13-C）。
/// 不能「DB 非空即整篇接管」——首次同步几个 `platform.json` 拉失败、或用户关掉自动同步后
/// 升级二进制新增了协议，那几个协议就会从下拉里整个消失。
pub async fn list_platform_presets(db: &Db) -> Result<Vec<PlatformPreset>, String> {
    let rows = select_platform_presets(db).await?;
    let have: std::collections::HashSet<&str> = rows.iter().map(|r| r.code.as_str()).collect();
    let mut out: Vec<PlatformPreset> = bundled_platform_presets()
        .iter()
        .filter(|p| !have.contains(p.code.as_str()))
        .cloned()
        .collect();
    out.extend(rows);
    out.sort_by(|a, b| a.code.cmp(&b.code));
    Ok(out)
}

/// 旧 `platform-presets.json` 形状的整篇文档（`get_defaults_json` 的数据源）。
/// DB 行覆盖 bundled 同 code、bundled 补齐 DB 缺的（票 13-C）；
/// `last_updated` 有 DB 行时取各行 `updated_at` 最大值（秒），否则用 registry 自带那个。
pub async fn presets_doc_json(db: &Db) -> Result<String, String> {
    Ok(presets_doc_value(db).await?.to_string())
}

/// [`presets_doc_json`] 的未序列化版本（调用方只需查其中一两个字段时别再解析一遍文本）。
pub async fn presets_doc_value(db: &Db) -> Result<serde_json::Value, String> {
    let rows = select_platform_presets(db).await?;
    let last_updated = rows.iter().map(|r| r.updated_at).max().map(|ms| ms / 1000);
    Ok(crate::registry::merge_presets_doc(
        rows.iter().map(|r| (r.code.as_str(), r.preset_data.as_str())),
        last_updated,
    ))
}

/// 单个协议的 preset 条目：DB 行优先，缺失回落 bundled（票 13-H）。
/// logo 懒加载这类「只查一个协议的一两个字段」的场景走它，别为一次查询重建整篇文档。
pub async fn preset_entry(db: &Db, code: &str) -> Result<Option<serde_json::Value>, String> {
    let owned = code.to_string();
    let row: Option<String> = db
        .call_read_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn
                .query_row(
                    "SELECT preset_data FROM platform_preset WHERE code = ?1 AND deleted_at = 0",
                    params![owned],
                    |r| r.get::<_, String>(0),
                )
                .optional()?)
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Some(raw) = row {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            return Ok(Some(v));
        }
        tracing::warn!(code, "platform_preset 行 JSON 解析失败，回落 bundled");
    }
    Ok(crate::registry::presets().get("protocols").and_then(|p| p.get(code)).cloned())
}

/// 把 DB 里的 preset 合并视图装进进程内缓存，供热路径 `effective_presets()` 同步读取。
/// 启动时与每轮 registry / logo 同步写库之后各跑一次。
pub async fn refresh_presets_cache(db: &Db) -> Result<(), String> {
    crate::registry::store_presets_cache(presets_doc_value(db).await?);
    Ok(())
}

/// 模型信息页一次性数据源：模型维度聚合行 + 全部平台预设（含品牌字段）。
/// `bundled = true` 表示模型条目来自编译期内置 registry（DB 尚未同步）。
pub async fn model_info_snapshot(db: &Db) -> Result<ModelInfoSnapshot, String> {
    let bundled = count_model_entries(db).await? == 0;
    let entries = list_model_entries(db, None).await?;
    let platforms = list_platform_presets(db).await?;
    // 有模型条目却没有 platform.json 的 code = `index.json` 的 pricing_only 来源
    // （litellm / meta / mistral），是比价参考而非可选平台。
    let selectable: std::collections::HashSet<&str> =
        platforms.iter().map(|p| p.code.as_str()).collect();
    let pricing_only: std::collections::HashSet<String> = entries
        .iter()
        .map(|e| e.platform_code.clone())
        .filter(|c| !selectable.contains(c.as_str()))
        .collect();
    let mut pricing_only_list: Vec<String> = pricing_only.iter().cloned().collect();
    pricing_only_list.sort();
    Ok(ModelInfoSnapshot {
        groups: group_by_canonical(entries, &pricing_only),
        platforms,
        pricing_only: pricing_only_list,
        bundled,
    })
}
