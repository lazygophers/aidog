//! JS 脚本自定义 quota 查询（三函数模式之三）。
//!
//! 脚本环境内置：
//!   - `ctx`：注入的查询上下文（`ctx.baseUrl` / `ctx.apiKey` / `ctx.extra`）
//!   - `http.get(url, headers?)` / `http.post(url, body, headers?)`：出站请求，
//!     走系统代理感知 client（CLIENT_BUILDER 注入，缺省直连 + 超时），每次出站落
//!     proxy_log（source_protocol="quota"，group_key="[quota:script]"），
//!     返回解析后的 JSON 对象（非 2xx / 解析失败 throw，脚本可 try/catch）
//!   - `JSON.parse` / `JSON.stringify`：boa 内置原生实现
//!
//! 脚本 `return` 固定格式（缺省字段自动补全为 None）：
//! ```js
//! return {
//!   success: true,
//!   error: null,
//!   balance: { remaining: 1.5, total: 10, used: 8.5, currency: "CNY", is_valid: true },
//!   coding_plan: { level: "pro", tiers: [
//!     { name: "five_hour", utilization: 42.0, resets_at: "2026-08-16T00:00:00Z",
//!       limit: 100, remaining: 58 },
//!   ]},
//!   newapi_user_id: "42", // 可选：New API 系从 /api/user/self 取回的用户 ID，前端回填用
//! };
//! ```
// ponytail: http.* 在 spawn_blocking 线程内同步执行；脚本查询频率低（手动/定时余额拉取），
// 阻塞代价可忽略；脚本出站经 http.rs::quota_script_request 单点落 proxy_log（[quota:script]）。
// CLIENT_BUILDER 是 async Fn，eval 在 spawn_blocking 内拿不到——出站 client 在 eval 前
// （异步侧）build 好，连同 db 经 Context::insert_data 注入，native 闭包内取用。

use boa_engine::{
    Context, JsArgs, JsNativeError, JsResult, JsValue, NativeFunction, Source,
    gc::{Finalize, Trace, empty_trace},
    js_string,
    object::{JsData, ObjectInitializer},
    property::Attribute,
};

use super::http::{
    PlatformQuota, QUOTA_PLATFORM_ID, err_quota, http_client, now_millis, quota_script_request,
};
use std::sync::Arc;

use aidog_db::Db;

/// 自定义查询脚本入参上下文（注入 JS 全局 `ctx` 对象）。
pub struct CustomQueryCtx {
    pub base_url: String,
    pub api_key: String,
    /// platform.extra 原文（JSON 字符串，脚本内 JSON.parse 使用）
    pub extra: String,
}

/// 执行 JS 自定义查询脚本，返回固定格式 PlatformQuota。
/// platform_id 透传落库日志（与 query_quota 同 idiom）；db 供脚本出站 client
/// 构建与 proxy_log 落库。platform_id=0 表示调用方没有平台行；此时保留
/// 外层 task-local 平台归属，避免启发式脚本把真实平台 ID 覆盖成 0。
pub async fn run_custom_query(
    db: Option<&Arc<Db>>,
    ctx: CustomQueryCtx,
    script: &str,
    platform_id: i64,
) -> PlatformQuota {
    let script = script.to_string();
    let effective_platform_id = if platform_id > 0 {
        platform_id
    } else {
        QUOTA_PLATFORM_ID.try_get().unwrap_or(0)
    };
    let outbound = Outbound {
        client: http_client(db).await,
        db: db.cloned(),
        platform_id: effective_platform_id,
    };
    let run = async move {
        // JS 引擎 Context 非 Send，spawn_blocking 线程内独占跑
        let joined = tokio::task::spawn_blocking(move || eval_script(&ctx, &outbound, &script)).await;
        match joined {
            Ok(Ok(quota)) => quota,
            Ok(Err(msg)) => err_quota(&msg),
            Err(e) => err_quota(&format!("script task failed: {e}")),
        }
    };
    if platform_id > 0 {
        QUOTA_PLATFORM_ID.scope(platform_id, run).await
    } else {
        run.await
    }
}

/// 脚本出站通道：eval 前（异步侧）build 好的 client + 落库用 db + 平台归属，
/// 经 `Context::insert_data` 注入，http.get/post native 函数内取用。
#[derive(Clone)]
struct Outbound {
    client: reqwest::Client,
    db: Option<Arc<Db>>,
    platform_id: i64,
}
impl Finalize for Outbound {}
// SAFETY: 字段（reqwest::Client / Option<Arc<Db>>）均非 boa Gc 可追踪类型。
unsafe impl Trace for Outbound {
    empty_trace!();
}
impl JsData for Outbound {}

fn eval_script(
    qctx: &CustomQueryCtx,
    outbound: &Outbound,
    script: &str,
) -> Result<PlatformQuota, String> {
    let mut ctx = Context::default();
    ctx.insert_data(outbound.clone());

    // ── 注入 ctx 对象 ──
    let ctx_obj = ObjectInitializer::new(&mut ctx)
        .property(
            js_string!("baseUrl"),
            js_string!(qctx.base_url.as_str()),
            Attribute::all(),
        )
        .property(
            js_string!("apiKey"),
            js_string!(qctx.api_key.as_str()),
            Attribute::all(),
        )
        .property(
            js_string!("extra"),
            js_string!(qctx.extra.as_str()),
            Attribute::all(),
        )
        .build();
    ctx.register_global_property(js_string!("ctx"), ctx_obj, Attribute::all())
        .map_err(|e| e.to_string())?;

    // ── 注入 http 对象 ──
    let http_obj = ObjectInitializer::new(&mut ctx)
        .function(NativeFunction::from_fn_ptr(http_get), js_string!("get"), 1)
        .function(
            NativeFunction::from_fn_ptr(http_post),
            js_string!("post"),
            2,
        )
        .build();
    ctx.register_global_property(js_string!("http"), http_obj, Attribute::all())
        .map_err(|e| e.to_string())?;

    // ── eval ──
    // 脚本按函数体语义执行（顶层 return 合法）：自动包裹 IIFE
    let wrapped = format!(
        "(function() {{
{script}
}})()"
    );
    let result = ctx
        .eval(Source::from_bytes(wrapped.as_bytes()))
        .map_err(|e| format!("script error: {e}"))?;

    // ── JsValue → JSON → PlatformQuota ──
    let json = result
        .to_json(&mut ctx)
        .map_err(|e| format!("script result not JSON-serializable: {e}"))?
        .unwrap_or(serde_json::Value::Null);
    Ok(parse_result(json))
}

/// 固定格式结果解析（宽松：缺省字段补全，balance/coding_plan 子对象可选）。
fn parse_result(json: serde_json::Value) -> PlatformQuota {
    let obj = match json {
        serde_json::Value::Object(o) => o,
        other => return err_quota(&format!("script must return an object, got: {other}")),
    };
    let success = obj
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !success {
        let msg = obj
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("script returned success=false");
        return err_quota(msg);
    }
    let balance = obj
        .get("balance")
        .and_then(|b| serde_json::from_value(b.clone()).ok());
    let coding_plan = obj
        .get("coding_plan")
        .and_then(|c| serde_json::from_value(c.clone()).ok());
    let newapi_user_id = obj
        .get("newapi_user_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    PlatformQuota {
        success: true,
        error: None,
        queried_at: now_millis(),
        balance,
        coding_plan,
        newapi_user_id,
    }
}

// ── native: http.get / http.post ────────────────────────
// spawn_blocking 线程内同步执行 reqwest；出站通道（client + db）从 Context data 取。

fn outbound_from_ctx(ctx: &Context) -> JsResult<Outbound> {
    ctx.get_data::<Outbound>().cloned().ok_or_else(|| {
        JsNativeError::error()
            .with_message("script outbound client missing")
            .into()
    })
}

fn js_headers_to_vec(headers: &JsValue, ctx: &mut Context) -> JsResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    if headers.is_undefined() || headers.is_null() {
        return Ok(out);
    }
    let obj = headers
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("headers must be an object"))?;
    for key in obj.own_property_keys(ctx)? {
        let val = obj.get(key.clone(), ctx)?;
        out.push((key.to_string(), val.to_string(ctx)?.to_std_string_escaped()));
    }
    Ok(out)
}

fn fetch_and_to_json(
    outbound: &Outbound,
    ctx: &mut Context,
    method: reqwest::Method,
    url: String,
    body: Option<String>,
    headers: Vec<(String, String)>,
) -> JsResult<JsValue> {
    let result: Result<serde_json::Value, String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(quota_script_request(
            outbound.db.as_ref(),
            outbound.client.clone(),
            method,
            &url,
            body,
            headers,
            outbound.platform_id,
        ))
    });
    match result {
        Ok(json) => JsValue::from_json(&json, ctx),
        Err(msg) => Err(JsNativeError::error().with_message(msg).into()),
    }
}

fn http_get(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let outbound = outbound_from_ctx(ctx)?;
    let url = args
        .get_or_undefined(0)
        .to_string(ctx)?
        .to_std_string_escaped();
    let headers = js_headers_to_vec(args.get_or_undefined(1), ctx)?;
    fetch_and_to_json(&outbound, ctx, reqwest::Method::GET, url, None, headers)
}

fn http_post(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let outbound = outbound_from_ctx(ctx)?;
    let url = args
        .get_or_undefined(0)
        .to_string(ctx)?
        .to_std_string_escaped();
    let body = args
        .get_or_undefined(1)
        .to_string(ctx)?
        .to_std_string_escaped();
    let headers = js_headers_to_vec(args.get_or_undefined(2), ctx)?;
    fetch_and_to_json(
        &outbound,
        ctx,
        reqwest::Method::POST,
        url,
        Some(body),
        headers,
    )
}

#[cfg(test)]
#[path = "test_script.rs"]
mod test_script;
