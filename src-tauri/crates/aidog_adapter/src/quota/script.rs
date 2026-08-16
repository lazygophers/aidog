//! JS 脚本自定义 quota 查询（三函数模式之三）。
//!
//! 脚本环境内置：
//!   - `ctx`：注入的查询上下文（`ctx.baseUrl` / `ctx.apiKey` / `ctx.extra`）
//!   - `http.get(url, headers?)` / `http.post(url, body, headers?)`：出站请求，
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
//! };
//! ```
// ponytail: http.* 在 spawn_blocking 线程内同步执行；脚本查询频率低（手动/定时余额拉取），
// 阻塞代价可忽略；脚本出站请求不落 proxy_log（平台侧 PlatformQuota 结果仍走上层落库），
// 需要逐请求审计时再接 context data 通道。

use boa_engine::{
    Context, JsArgs, JsNativeError, JsResult, JsValue, NativeFunction, Source,
    js_string,
    object::ObjectInitializer,
    property::Attribute,
};

use super::http::{err_quota, now_millis, PlatformQuota, QUOTA_PLATFORM_ID};
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
/// platform_id 透传落库日志（与 query_quota 同 idiom）。
pub async fn run_custom_query(
    _db: Option<&Arc<Db>>,
    ctx: CustomQueryCtx,
    script: &str,
    platform_id: i64,
) -> PlatformQuota {
    QUOTA_PLATFORM_ID.scope(platform_id, {
        let script = script.to_string();
        async move {
            // JS 引擎 Context 非 Send，spawn_blocking 线程内独占跑
            let joined = tokio::task::spawn_blocking(move || eval_script(&ctx, &script))
                .await;
            match joined {
                Ok(Ok(quota)) => quota,
                Ok(Err(msg)) => err_quota(&msg),
                Err(e) => err_quota(&format!("script task failed: {e}")),
            }
        }
    })
    .await
}

fn eval_script(qctx: &CustomQueryCtx, script: &str) -> Result<PlatformQuota, String> {
    let mut ctx = Context::default();

    // ── 注入 ctx 对象 ──
    let ctx_obj = ObjectInitializer::new(&mut ctx)
        .property(js_string!("baseUrl"), js_string!(qctx.base_url.as_str()), Attribute::all())
        .property(js_string!("apiKey"), js_string!(qctx.api_key.as_str()), Attribute::all())
        .property(js_string!("extra"), js_string!(qctx.extra.as_str()), Attribute::all())
        .build();
    ctx.register_global_property(js_string!("ctx"), ctx_obj, Attribute::all())
        .map_err(|e| e.to_string())?;

    // ── 注入 http 对象 ──
    let http_obj = ObjectInitializer::new(&mut ctx)
        .function(NativeFunction::from_fn_ptr(http_get), js_string!("get"), 1)
        .function(NativeFunction::from_fn_ptr(http_post), js_string!("post"), 2)
        .build();
    ctx.register_global_property(js_string!("http"), http_obj, Attribute::all())
        .map_err(|e| e.to_string())?;

    // ── eval ──
    // 脚本按函数体语义执行（顶层 return 合法）：自动包裹 IIFE
    let wrapped = format!("(function() {{
{script}
}})()");
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
    let success = obj.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if !success {
        let msg = obj.get("error").and_then(|v| v.as_str()).unwrap_or("script returned success=false");
        return err_quota(msg);
    }
    let balance = obj.get("balance").and_then(|b| serde_json::from_value(b.clone()).ok());
    let coding_plan = obj.get("coding_plan").and_then(|c| serde_json::from_value(c.clone()).ok());
    PlatformQuota {
        success: true,
        error: None,
        queried_at: now_millis(),
        balance,
        coding_plan,
        newapi_user_id: None,
    }
}

// ── native: http.get / http.post ────────────────────────
// spawn_blocking 线程内同步执行 reqwest。

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
    ctx: &mut Context,
    method: reqwest::Method,
    url: String,
    body: Option<String>,
    headers: Vec<(String, String)>,
) -> JsResult<JsValue> {
    let result: Result<serde_json::Value, String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let client = reqwest::Client::new();
            let mut req = client.request(method, &url);
            for (k, v) in &headers {
                req = req.header(k, v);
            }
            if let Some(b) = &body {
                req = req.body(b.clone());
            }
            let resp = req.send().await.map_err(|e| e.to_string())?;
            let status = resp.status().as_u16();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !(200..300).contains(&status) {
                return Err(format!("HTTP {status}: {}", text.chars().take(500).collect::<String>()));
            }
            serde_json::from_str::<serde_json::Value>(&text).map_err(|e| format!("JSON parse: {e}"))
        })
    });
    match result {
        Ok(json) => JsValue::from_json(&json, ctx),
        Err(msg) => Err(JsNativeError::error().with_message(msg).into()),
    }
}

fn http_get(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let url = args.get_or_undefined(0).to_string(ctx)?.to_std_string_escaped();
    let headers = js_headers_to_vec(args.get_or_undefined(1), ctx)?;
    fetch_and_to_json(ctx, reqwest::Method::GET, url, None, headers)
}

fn http_post(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let url = args.get_or_undefined(0).to_string(ctx)?.to_std_string_escaped();
    let body = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
    let headers = js_headers_to_vec(args.get_or_undefined(2), ctx)?;
    fetch_and_to_json(ctx, reqwest::Method::POST, url, Some(body), headers)
}

#[cfg(test)]
#[path = "test_script.rs"]
mod test_script;
