use super::*;
use rusqlite::{params, OptionalExtension, Result as SqlResult};

const MODEL_PRICE_COLUMNS: &str =
    "id, model_name, source, price_data, max_input_tokens, max_output_tokens, context_window, created_at, updated_at, deleted_at";

fn row_to_model_price(row: &rusqlite::Row) -> SqlResult<crate::gateway::models::ModelPrice> {
    Ok(crate::gateway::models::ModelPrice {
        id: row.get::<_, i64>(0)? as u64,
        model_name: row.get(1)?,
        source: row.get(2)?,
        price_data: row.get(3)?,
        max_input_tokens: row.get::<_, Option<i64>>(4)?,
        max_output_tokens: row.get::<_, Option<i64>>(5)?,
        context_window: row.get::<_, Option<i64>>(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        deleted_at: row.get(9)?,
    })
}

/// 提取关键字段构建摘要
fn price_data_to_summary(mp: &crate::gateway::models::ModelPrice) -> crate::gateway::models::ModelPriceSummary {
    let pd: serde_json::Value = serde_json::from_str(&mp.price_data).unwrap_or_default();
    let input = pd.get("input_cost_per_token").and_then(|v| v.as_f64());
    let output = pd.get("output_cost_per_token").and_then(|v| v.as_f64());
    let cache_read = pd.get("cache_read_input_token_cost").and_then(|v| v.as_f64());
    let default_platform = pd.get("default_platform").and_then(|v| v.as_str()).map(String::from);

    crate::gateway::models::ModelPriceSummary {
        id: mp.id,
        model_name: mp.model_name.clone(),
        source: mp.source.clone(),
        default_platform,
        // Convert $/token → $/M tokens for display
        input_price: input.map(|v| v * 1_000_000.0),
        output_price: output.map(|v| v * 1_000_000.0),
        cache_read_price: cache_read.map(|v| v * 1_000_000.0),
        max_input_tokens: mp.max_input_tokens,
        max_output_tokens: mp.max_output_tokens,
        context_window: mp.context_window,
        updated_at: mp.updated_at,
    }
}

#[track_caller]
pub fn list_model_prices(db: &Db, limit: u32, offset: u32) -> impl std::future::Future<Output = Result<Vec<crate::gateway::models::ModelPriceSummary>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE deleted_at = 0 ORDER BY model_name LIMIT ?1 OFFSET ?2")
            )?;
            let rows = stmt.query_map(params![limit, offset], row_to_model_price)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(price_data_to_summary(&r?));
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[track_caller]
pub fn count_model_prices(db: &Db) -> impl std::future::Future<Output = Result<u32, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM model_price WHERE deleted_at = 0", [], |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 获取指定模型的最新价格记录（优先 manual > github）
#[track_caller]
pub fn get_model_price<'a>(db: &'a Db, model_name: &'a str) -> impl std::future::Future<Output = Result<Option<crate::gateway::models::ModelPrice>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let model_name = model_name.to_string();
    db
        .call_read_traced(None, __db_caller, move |conn| {
            // 优先取 manual 记录
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE model_name = ?1 AND source = 'manual' AND deleted_at = 0")
            )?;
            if let Some(mp) = stmt.query_row(params![model_name], row_to_model_price).optional()? {
                return Ok(Some(mp));
            }
            // 回退到 github（同步源）
            let mut stmt2 = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE model_name = ?1 AND source = 'github' AND deleted_at = 0")
            )?;
            Ok(stmt2.query_row(params![model_name], row_to_model_price).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// Upsert a model price record (INSERT OR REPLACE by model_name + source)
#[track_caller]
pub fn upsert_model_price<'a>(
    db: &'a Db,
    model_name: &'a str,
    source: &'a str,
    price_data: &'a str,
    max_input_tokens: Option<i64>,
    max_output_tokens: Option<i64>,
    context_window: Option<i64>,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    let model_name = model_name.to_string();
    let source = source.to_string();
    let price_data = price_data.to_string();
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO model_price (model_name, source, price_data, max_input_tokens, max_output_tokens, context_window, created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0)
                 ON CONFLICT(model_name, source) DO UPDATE SET
                   price_data = ?3,
                   max_input_tokens = ?4,
                   max_output_tokens = ?5,
                   context_window = ?6,
                   updated_at = ?7,
                   deleted_at = 0",
                params![model_name, source, price_data, max_input_tokens, max_output_tokens, context_window, ts],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert model price: {e}"))
    }
}

/// 取模型最大输出 token（出站裁剪用）。列优先，NULL 时回退 price_data JSON。
/// 返回 None = 未知/无限制（不裁剪）。
pub async fn get_model_max_output_tokens(db: &Db, model_name: &str) -> Result<Option<i64>, String> {
    let mp = get_model_price(db, model_name).await?;
    if let Some(m) = mp {
        if let Some(v) = m.max_output_tokens {
            return Ok(Some(v));
        }
        // 回退 price_data JSON（旧库 / 手动录入仅写 JSON 的兼容路径）
        let pd: serde_json::Value = serde_json::from_str(&m.price_data).unwrap_or_default();
        return Ok(pd.get("max_output_tokens").and_then(|v| v.as_i64()));
    }
    Ok(None)
}

/// 解析价格：model_name + platform_type → ResolvedPrice
/// 优先级: pricing[platform_type] > top_level > default_platform pricing > fallback settings
pub async fn resolve_price(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    fallback_input: f64,
    fallback_output: f64,
    input_tokens: i64,
) -> Result<crate::gateway::models::ResolvedPrice, String> {
    let mp = get_model_price(db, model_name).await?;
    let pd: serde_json::Value = match &mp {
        Some(m) => serde_json::from_str(&m.price_data).unwrap_or_default(),
        None => serde_json::Value::Null,
    };

    // 1. Try pricing[platform_type]
    if let Some(pricing_node) = pd.get("pricing").and_then(|p| p.get(platform_type)) {
        let input = pricing_node.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let output = pricing_node.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let cache = pricing_node.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if input > 0.0 || output > 0.0 {
            return Ok(apply_context_tier(
                crate::gateway::models::ResolvedPrice {
                    input_cost_per_token: input,
                    output_cost_per_token: output,
                    cache_read_input_token_cost: cache,
                    source: "platform_override".to_string(),
                },
                &pd,
                input_tokens,
            ));
        }
    }

    // 2. Try top-level price
    let top_input = pd.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let top_output = pd.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let top_cache = pd.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if top_input > 0.0 || top_output > 0.0 {
        return Ok(apply_context_tier(
            crate::gateway::models::ResolvedPrice {
                input_cost_per_token: top_input,
                output_cost_per_token: top_output,
                cache_read_input_token_cost: top_cache,
                source: "top_level".to_string(),
            },
            &pd,
            input_tokens,
        ));
    }

    // 3. Try default_platform pricing
    if let Some(dp) = pd.get("default_platform").and_then(|v| v.as_str()) {
        if let Some(pricing_node) = pd.get("pricing").and_then(|p| p.get(dp)) {
            let input = pricing_node.get("input_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let output = pricing_node.get("output_cost_per_token").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let cache = pricing_node.get("cache_read_input_token_cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if input > 0.0 || output > 0.0 {
                return Ok(apply_context_tier(
                    crate::gateway::models::ResolvedPrice {
                        input_cost_per_token: input,
                        output_cost_per_token: output,
                        cache_read_input_token_cost: cache,
                        source: "default_platform".to_string(),
                    },
                    &pd,
                    input_tokens,
                ));
            }
        }
    }

    // 4. Fallback
    Ok(crate::gateway::models::ResolvedPrice {
        input_cost_per_token: fallback_input / 1_000_000.0,
        output_cost_per_token: fallback_output / 1_000_000.0,
        cache_read_input_token_cost: 0.0,
        source: "fallback".to_string(),
    })
}

/// 上下文阶梯选档：取 `context_tiers` 中 `min_tokens <= input_tokens` 的最大档，
/// 非 null 字段覆盖 base 价（null 字段继承 base，如某些模型长档无 cache 价）。
/// `context_tiers` 缺失/非数组/无命中档 → 返回 base 不变（向后兼容旧 price_data）。
pub(crate) fn apply_context_tier(
    mut base: crate::gateway::models::ResolvedPrice,
    pd: &serde_json::Value,
    input_tokens: i64,
) -> crate::gateway::models::ResolvedPrice {
    let Some(tiers) = pd.get("context_tiers").and_then(|v| v.as_array()) else {
        return base;
    };
    // 选 min_tokens <= input_tokens 中阈值最大的档（最高适用档）
    let best = tiers
        .iter()
        .filter_map(|t| {
            let min_tokens = t.get("min_tokens").and_then(|v| v.as_i64())?;
            (min_tokens <= input_tokens).then_some((min_tokens, t))
        })
        .max_by_key(|(min_tokens, _)| *min_tokens);
    let Some((_, tier)) = best else {
        return base;
    };
    if let Some(v) = tier.get("input_cost_per_token").and_then(|v| v.as_f64()) {
        base.input_cost_per_token = v;
    }
    if let Some(v) = tier.get("output_cost_per_token").and_then(|v| v.as_f64()) {
        base.output_cost_per_token = v;
    }
    if let Some(v) = tier.get("cache_read_input_token_cost").and_then(|v| v.as_f64()) {
        base.cache_read_input_token_cost = v;
    }
    base.source.push_str("+tier");
    base
}

/// 搜索模型价格
#[track_caller]
pub fn search_model_prices<'a>(db: &'a Db, query: &'a str, limit: u32) -> impl std::future::Future<Output = Result<Vec<crate::gateway::models::ModelPriceSummary>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let pattern = format!("%{query}%");
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                &format!("SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE deleted_at = 0 AND model_name LIKE ?1 ORDER BY model_name LIMIT ?2")
            )?;
            let rows = stmt.query_map(params![pattern, limit], row_to_model_price)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(price_data_to_summary(&r?));
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// Filtered list: optional query (LIKE model_name), optional source, limit, offset.
#[track_caller]
pub fn filtered_list_model_prices<'a>(
    db: &'a Db,
    query: Option<&'a str>,
    source: Option<&'a str>,
    limit: u32,
    offset: u32,
) -> impl std::future::Future<Output = Result<Vec<crate::gateway::models::ModelPriceSummary>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let query = query.map(|s| s.to_string());
    let source = source.map(|s| s.to_string());
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let query = query.as_deref();
            let source = source.as_deref();
    let mut where_parts = vec!["deleted_at = 0".to_string()];
    let mut param_idx = 1;
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(q) = query {
        if !q.is_empty() {
            where_parts.push(format!("model_name LIKE ?{param_idx}"));
            params.push(Box::new(format!("%{q}%")));
            param_idx += 1;
        }
    }
    if let Some(s) = source {
        if !s.is_empty() {
            where_parts.push(format!("source = ?{param_idx}"));
            params.push(Box::new(s.to_string()));
            param_idx += 1;
        }
    }

    let where_sql = where_parts.join(" AND ");
    let sql = format!(
        "SELECT {MODEL_PRICE_COLUMNS} FROM model_price WHERE {where_sql} ORDER BY model_name LIMIT ?{param_idx} OFFSET ?{}",
        param_idx + 1
    );
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), row_to_model_price)?;
    let mut result = Vec::new();
    for r in rows {
        result.push(price_data_to_summary(&r?));
    }
    Ok(result)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// Count matching model prices with optional filters.
#[track_caller]
pub fn filtered_count_model_prices<'a>(
    db: &'a Db,
    query: Option<&'a str>,
    source: Option<&'a str>,
) -> impl std::future::Future<Output = Result<u32, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let query = query.map(|s| s.to_string());
    let source = source.map(|s| s.to_string());
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let query = query.as_deref();
            let source = source.as_deref();
    let mut where_parts = vec!["deleted_at = 0".to_string()];
    let mut param_idx = 1;
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(q) = query {
        if !q.is_empty() {
            where_parts.push(format!("model_name LIKE ?{param_idx}"));
            params.push(Box::new(format!("%{q}%")));
            param_idx += 1;
        }
    }
    if let Some(s) = source {
        if !s.is_empty() {
            where_parts.push(format!("source = ?{param_idx}"));
            params.push(Box::new(s.to_string()));
        }
    }

    let where_sql = where_parts.join(" AND ");
    let sql = format!("SELECT COUNT(*) FROM model_price WHERE {where_sql}");
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    Ok(conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

// ─── MCP server CRUD ───────────────────────────────────────
// 集中存 MCP server 配置（migration 020）。行结构见 crate::gateway::mcp::McpServerRow。
// env_json/headers_json 含原始敏感值，调用方负责脱敏后再返前端。

