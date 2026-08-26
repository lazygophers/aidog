use super::*;

/// 单次候选 forward 尝试的控制结果：Respond=已确定响应直接返回客户端；Next=换下个候选重试。
pub(crate) enum AttemptOutcome {
    Respond(axum::response::Response),
    Next,
}

/// 单次候选 forward 尝试：构建上游请求、发送、按状态码分类处理。
/// 返回 AttemptOutcome::Respond 直接回客户端，Next 表示换下个候选重试。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn forward_attempt(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    route: RouteResult,
    is_last_candidate: bool,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    log_settings: &ProxyLogSettings,
    lang: Lang,
    group: &Group,
    chat_req: &mut ChatRequest,
    req_value: &Value,
    source_protocol: &Protocol,
    requested_model: &str,
    is_stream: bool,
    orig_headers: &axum::http::HeaderMap,
    sched_settings: &super::models::SchedulingBreakerSettings,
    start: std::time::Instant,
) -> AttemptOutcome {
    let actual_model = route.target_model.clone();

    // OpenCode Zen：api_key 留空 → 注入匿名免费 key（$opencode）；用户填了用用户的。
    let eff_api_key = resolve_opencode_zen_key(&route.platform);

    // 尝试匹配端点：按 source_protocol 查找平台是否支持对应协议的端点。
    // 先精确匹配；openai_responses 源（Codex）若无 Responses 端点，回退到 openai 端点
    // （普通 chat/completions 平台），出站经 to_openai 转换。
    let matched_ep = select_endpoint_for_protocol(&route.platform.endpoints, source_protocol);

    // ── UA 透传分支（[protocol-same-proto-passthrough] 扩展，PRD §5 级别 1）──
    // 仅当 path 推断的入站协议在平台无任何对应 endpoint（matched_ep == None，
    // 现状会落入 platform_type + ClientType::Default 有损兜底）时尝试：
    // 按入站 User-Agent 推断客户端原生协议（claude-cli→anthropic / codex→openai_responses），
    // 若平台确有该协议的 endpoint → matched_ep 改指向该 UA-endpoint，并以该协议为透传 wire 协议。
    // UA 不识别 / 平台无该协议 endpoint → matched_ep 保持 None，回退现有兜底（零行为变更）。
    // matched_ep 命中（path 已支持）时不介入。
    let (matched_ep, passthrough_proto) = if matched_ep.is_none() {
        let ua_proto = orig_headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .and_then(infer_passthrough_protocol_from_ua);
        match ua_proto {
            Some(p) => match route.platform.endpoints.iter().find(|ep| ep.protocol == p) {
                Some(ep) => {
                    tracing::info!(
                        platform = %route.platform.name, platform_id = route.platform.id,
                        source_protocol = ?source_protocol, ua_protocol = ?p,
                        "ua-passthrough: path protocol unsupported by platform, routing to UA-inferred endpoint"
                    );
                    (Some(ep), Some(p))
                }
                // UA 命中但平台无该协议 endpoint（级别 2）→ 回退现有兜底
                None => (matched_ep, None),
            },
            // UA 不识别（级别 3）→ 回退现有兜底
            None => (matched_ep, None),
        }
    } else {
        (matched_ep, None)
    };

    let (target_protocol_enum, target_base_url, client_type, coding_plan) = matched_ep
        .map(|ep| (&ep.protocol, ep.base_url.clone(), ep.client_type.clone(), ep.coding_plan))
        .unwrap_or((&route.platform.platform_type, route.platform.base_url.clone(), "default".to_string(), false));

    // ── target_protocol 合法性 guard（bugfix: s2-bug1-target-protocol）──
    // matched_ep=None 时 fallback 到 platform_type，但 platform_type 可能是平台别名(sensenova/glm等)
    // 而非 5 个有效协议之一(anthropic/openai/openai_responses/openai_completions/gemini)。
    // 这种情况下 target_protocol 会落库为平台名，导致后续统计/审计出错。
    //
    // 验收：endpoint 匹配失败时 target_protocol 必须落 5 协议之一；否则 route fail。
    // ponytail: 仅检测 5 协议，未来扩展协议需同步更新此列表。
    let is_valid_wire_protocol = |p: &Protocol| -> bool {
        matches!(p, Protocol::Anthropic | Protocol::OpenAI | Protocol::OpenAIResponses | Protocol::OpenAICompletions | Protocol::Gemini)
    };
    if !is_valid_wire_protocol(target_protocol_enum) {
        tracing::error!(
            platform = %route.platform.name, platform_id = route.platform.id,
            source_protocol = ?source_protocol, target_protocol = ?target_protocol_enum,
            endpoints_len = route.platform.endpoints.len(),
            "target_protocol is not a valid wire protocol, endpoint selection failed"
        );
        // endpoint 选择失败且无有效兜底 → 记录 error 并 route fail
        if !is_last_candidate {
            // 非 last candidate：记录 attempt 并换下一个候选
            attempts.push(ProxyAttempt {
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 0,
                error: format!("invalid target protocol: {:?}", target_protocol_enum),
                duration_ms: attempt_start.elapsed().as_millis() as i64,
                ts: attempt_ts,
            });
            let _ = aidog_db::set_platform_last_error(
                &state.db, route.platform.id, Some(format!("invalid target protocol: {:?}", target_protocol_enum)),
            ).await;
            return AttemptOutcome::Next;
        }
        // last candidate：返回 502 + 审计落库
        let msg = format!("{}: endpoint selection failed (no valid wire protocol)", i18n::t(lang, ErrorKey::Upstream));
        return AttemptOutcome::Respond(
            finalize_proxy_502(
                state, log, attempts, route.platform.id,
                format!("invalid target protocol: {:?}", target_protocol_enum),
                msg, start, log_settings,
            ).await,
        );
    }

    // ── base_url 缺失 guard ──
    // endpoints/base_url 均空（OAuth 未回填 / 用户手建平台漏配）→ 友好错误替代 reqwest builder error。
    // 空 base_url 拼 api_path 得无 host 相对 URL，reqwest builder 直接 error → 502「upstream error」无诊断价值。
    // 不发上游：记录平台 last_error + attempts；非末位候选 → Next 换下个；末位候选 → 502 + 审计落库。
    // ponytail: 对称防护——forward.rs 单一 URL 构造点覆盖流式 + 非流式两分支（URL 在分支前已定）。
    if target_base_url.trim().is_empty() {
        tracing::warn!(
            platform = %route.platform.name, platform_id = route.platform.id,
            "upstream base_url empty, skipping platform"
        );
        attempts.push(ProxyAttempt {
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: 0,
            error: "base_url missing".to_string(),
            duration_ms: attempt_start.elapsed().as_millis() as i64,
            ts: attempt_ts,
        });
        let _ = aidog_db::set_platform_last_error(
            &state.db, route.platform.id, Some("base_url missing".to_string()),
        ).await;
        if !is_last_candidate {
            return AttemptOutcome::Next;
        }
        let msg = format!("{}: base_url 缺失", i18n::t(lang, ErrorKey::Upstream));
        return AttemptOutcome::Respond(
            finalize_proxy_502(
                state, log, attempts, route.platform.id,
                "base_url missing".to_string(), msg, start, log_settings,
            ).await,
        );
    }

    let target_protocol = target_protocol_enum.wire_str();
    let needs_model_remap = actual_model != requested_model;

    // ── 同协议透传判定 ──
    // 平台**显式声明**了与入站协议精确相同的端点 → 逻辑透传：跳过 convert_request 有损格式转换，
    // 用客户端原始请求体（仅 patch model 字段）出站；响应侧同样跳过 parse_sse→to_client_sse 格式转换。
    // 鉴权 / URL / coding_plan / usage 提取等旁路改写仍全部保留。
    // 注意：openai_responses→openai 的跨协议回退命中时 target_protocol != source_protocol，
    // 不算透传，仍走 convert_request（必须真转换）。
    // 透传判定：
    // - 级别 0（现状）：端点协议精确等于 path 推断的 source_protocol。
    // - 级别 1（UA 透传）：passthrough_proto == Some(p) 且端点协议等于 UA 推断协议 p
    //   → 端点协议 == source_protocol 不成立（否则 matched_ep 在级别 0 已命中），故单独判定。
    let same_protocol_passthrough = match passthrough_proto {
        Some(p) => matched_ep.map(|ep| ep.protocol == p).unwrap_or(false),
        None => matched_ep.map(|ep| ep.protocol == *source_protocol).unwrap_or(false),
    };

    // Upsert #3: route resolved
    log.actual_model = actual_model.clone();
    log.target_protocol = target_protocol.clone();
    log.platform_id = route.platform.id;
    tracing::info!(
        platform = %route.platform.name, platform_id = route.platform.id,
        requested_model = %requested_model, actual_model = %actual_model,
        source_protocol = ?source_protocol, target_protocol = %target_protocol,
        coding_plan, stream = is_stream, remap = needs_model_remap,
        "request routed to upstream"
    );
    upsert_log(state, log, log_settings).await;

    // 替换模型名
    chat_req.model = actual_model.clone();

    // ── max_tokens 出站裁剪（convert_request 前）──
    // 客户端 max_tokens 超过选定模型上限时裁剪到上限；未传 / 模型无上限则不动（Q3 保守）。
    // 此处裁 chat_req（转换分支的入参，同时是 token 估算口径）；出站 body 上的同上限复裁
    // 见下方 `cap_body_max_tokens` 调用点（透传分支唯一生效处）。
    let model_max = aidog_db::get_model_max_output_tokens(&state.db, &actual_model)
        .await
        .ok()
        .flatten();
    {
        let (capped, did_cap) = super::router::cap_max_tokens(chat_req.max_tokens, model_max);
        if did_cap {
            tracing::info!(
                model = %actual_model,
                requested = ?chat_req.max_tokens, capped_to = ?capped,
                "max_tokens exceeds model limit, capping"
            );
            chat_req.max_tokens = capped;
        }
    }

    // ── 中间件入站规则（platform 层，候选选定后、convert_request 前）──
    // 仅应用 platform 作用域规则（global/group 已在路由前应用，避免重复）。
    // block 在 forward 前返回，对透传/转换分支均生效；mask/inject 改写 chat_req，
    // 转换分支(convert_request 读 chat_req)由此生效；同协议透传分支用 req_value 原体，
    // 由下方 `apply_middleware_body` 在出站 body 上补齐（票 02）。
    {
        let mw_settings = state.settings_cache.read().await.middleware_settings.clone();
        if let InboundOutcome::Blocked { blocked_by, blocked_reason } =
            state.middleware.apply_inbound_platform(&mw_settings, chat_req, route.platform.id as i64)
        {
            log.platform_id = route.platform.id;
            return AttemptOutcome::Respond(
                block_inbound(state, log.clone(), log_settings, lang, blocked_by, blocked_reason, start).await,
            );
        }
    }

    // ── 手动预算耗尽阻断（mock / 上游平台均适用，转发前惰性只读判定，不写库）──
    // 任一 enabled 限额剩余 ≤ 0（含窗口惰性重置后）→ 不发上游/不出 mock，返回 402。
    // 平台保持启用，窗口/次日恢复后自动放行。无 manual_budgets（含透传）→ 跳过。
    if let Some(info) = super::manual_budget::evaluate_depletion(&route.platform.manual_budgets, aidog_db::now()) {
        let recover_hint = match info.kind.as_str() {
            "daily" => i18n::t(lang, ErrorKey::BudgetResetDaily),
            "rolling" => i18n::t(lang, ErrorKey::BudgetResetRolling),
            "fixed" => i18n::t(lang, ErrorKey::BudgetResetFixed),
            _ => i18n::t(lang, ErrorKey::BudgetResetTotal),
        };
        let body = serde_json::json!({
            "error": {
                "type": "manual_budget_exhausted",
                "message": format!(
                    "{} (kind={}, unit={}, amount={}). {}",
                    i18n::t(lang, ErrorKey::BudgetExhausted),
                    info.kind, info.unit, info.amount, recover_hint
                ),
                "budget_kind": info.kind,
                "budget_unit": info.unit,
                "budget_amount": info.amount,
            }
        })
        .to_string();
        tracing::warn!(
            platform = %route.platform.name, kind = %info.kind, unit = %info.unit, amount = info.amount,
            "manual budget exhausted, blocking request (402)"
        );
        log.status_code = 402;
        log.done = true;
        log.platform_id = route.platform.id;
        log.response_body = body.clone();
        log.user_response_body = body.clone();
        log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
        log.duration_ms = start.elapsed().as_millis() as i32;
        attempts.push(ProxyAttempt {
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: 402,
            error: "manual budget exhausted".to_string(),
            duration_ms: attempt_start.elapsed().as_millis() as i64,
            ts: attempt_ts,
        });
        log.retry_count = (attempts.len() as i32 - 1).max(0);
        log.attempts = std::mem::take(attempts);
        upsert_log(state, log, log_settings).await;
        return AttemptOutcome::Respond({
            let mut r = (
                StatusCode::PAYMENT_REQUIRED,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                body,
            )
                .into_response();
            inject_trace_header(&mut r);
            r
        });
    }

    // 协议转换 / 同协议透传：
    // - 透传分支（同协议）：用客户端原始请求体，仅 patch model 字段，跳过 messages/tools 结构转换；
    //   path 由 wire 协议决定（passthrough_api_path，与 convert_request 一致但不转 body）。
    // - 转换分支：wire format 由 endpoint 协议决定，API path 由平台类型决定。
    let platform_protocol = &route.platform.platform_type;
    let (mut req_body, mut api_path) = if same_protocol_passthrough {
        let mut body = req_value.clone();
        // model remap：透传下仍必须替换路由模型名（请求体 model 字段）
        if let Some(obj) = body.as_object_mut() {
            obj.insert("model".to_string(), Value::String(actual_model.clone()));
        }
        let path = adapter::passthrough_api_path(target_protocol_enum, &actual_model, platform_protocol);
        tracing::debug!(protocol = %target_protocol, "same-protocol passthrough: skip request format conversion");
        (body, path)
    } else {
        adapter::convert_request(chat_req, target_protocol_enum, platform_protocol)
    };

    // Coding Plan 特殊处理：注入平台特有字段 + 覆盖 API 路径
    if coding_plan {
        inject_coding_plan_fields(&mut req_body, platform_protocol);
        override_coding_plan_path(&mut api_path, platform_protocol);
    }

    // disable_thinking：aidog 本地扩展字段（客户端请求禁用思考）。非标字段任何上游不认 →
    // 识别后必剥除。语义 = 剔掉开启型思考参数后，按目标 wire 协议写入显式禁用参数
    // （用户决策 2026-08-26；只剔不写会让上游按自身默认开启思考，见 apply_disable_thinking 注释）。
    // MiniMax-M2 等内置思考模型上游仍无法真正禁用，响应不剥离。
    let disable_thinking = req_value
        .get("disable_thinking")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    apply_disable_thinking(&mut req_body, disable_thinking, target_protocol_enum, &target_base_url);

    // builtin-tool-compat：per-model 内置工具兼容（platform.extra.builtin_tool_compat，
    // 默认关闭零进入）。两级 AND：全局总开关（ProxySettingsCache）× 平台级 enabled。
    // 透传与转换两分支共用本 seam（见 builtin_tools.rs 模块注释）。
    let btc_global = state.settings_cache.read().await.builtin_tool_compat.enabled;
    builtin_tools::apply_builtin_tool_compat(&mut req_body, &route.platform.extra, &actual_model, btc_global);

    // ── max_completion_tokens 归一（必须排在下方裁剪之前）──
    // 透传分支的 body 是客户端原体：新版 OpenAI SDK 只发 `max_completion_tokens`，
    // 不折成 `max_tokens` 的话下方 cap 找不到槽位 → 超模型上限的值原样上送（票 05）。
    fold_openai_max_completion_tokens(&mut req_body, target_protocol_enum);

    // ── max_tokens 出站裁剪（body 层，透传与转换两分支共用本 seam）──
    // 上限口径与上方 chat_req 侧同源（同一个 `model_max`，不会出现「裁两次不同上限」）：
    // 转换分支此处为幂等复裁（chat_req 已裁到同值，不再命中）；透传分支此处是唯一生效点，
    // 修掉「客户端 max_tokens 超模型上限 → 上游 400」（票 02）。
    // 留痕：proxy_log 同时存客户端原始 body（request_body）与上游实际 body
    // （upstream_request_body），两者一比即知裁到多少。
    if let Some((requested, capped)) = cap_body_max_tokens(&mut req_body, model_max, target_protocol_enum) {
        tracing::info!(
            model = %actual_model, requested, capped_to = capped,
            passthrough = same_protocol_passthrough,
            "max_tokens exceeds model limit, capping outbound body"
        );
    }

    // ── 中间件入站改写（Value 层，仅透传分支）──
    // 转换分支的 mask/override/inject 已在分叉前作用于 chat_req，两处都跑会把 system_append
    // 注入两遍，故此处只补透传分支（脱敏规则不再因为「恰好同协议」被绕过，票 02）。
    if same_protocol_passthrough {
        let mw_settings = state.settings_cache.read().await.middleware_settings.clone();
        let changed = middleware_body::apply_middleware_body(
            &state.middleware, &mw_settings, &mut req_body,
            target_protocol_enum, &actual_model, route.platform.id as i64,
        );
        if changed {
            tracing::info!(
                platform_id = route.platform.id, model = %actual_model,
                "middleware inbound rules rewrote passthrough body"
            );
        }
    }

    // ── 未建模顶层字段兜底透传（票 01，透传与转换两分支共用本 seam）──
    // 客户端设的采样参数（stop / top_k / seed / response_format / …）在 ChatRequest 强类型模型里
    // 没有对应字段，转换分支经 wire struct 序列化后静默消失。本 seam 从客户端原体按**目标 wire
    // 协议的允许集合**补齐（键名按协议换名），允许集合外的字段一律不写出。
    apply_field_passthrough(&mut req_body, req_value, target_protocol_enum, &target_base_url);

    // ── 官方 OpenAI 输出长度键改写（票 05）──
    // 排在裁剪、中间件与兜底透传之后：前几步都按 `max_tokens` 认字段，改名放最后才不会漏。
    if rename_openai_max_tokens_key(&mut req_body, target_protocol_enum, &target_base_url) {
        tracing::debug!(model = %actual_model, "official OpenAI host: max_tokens → max_completion_tokens");
    }

    // 构建目标 URL
    let base_url = target_base_url.trim_end_matches('/');
    let mut url = format!("{}{}", base_url, api_path);
    // Gemini streamGenerateContent 不带 alt=sse 时上游返回单个 JSON 数组（非 SSE），流式解析全部落空。
    if matches!(target_protocol_enum, Protocol::Gemini) && is_stream {
        url.push_str("?alt=sse");
    }
    log.upstream_request_url = url.clone();

    // ── 第三方 anthropic 端点不支持字段剔除 / 非标结构规整 ──
    // host-gated（仅 !is_official_anthropic_host）：
    //   - context_management：thinking 开启即无条件剔（第三方不认该协商字段；首轮 GLM 1210 + 有历史 DeepSeek 400）
    //   - thinking：仅历史 assistant 轮缺 thinking block（必 400 的不匹配）才剔，齐全直传
    //   - messages 内 role=system 非标位置规整：非流式多轮（有 assistant 历史）+ messages 内含 role=system
    //     时，GLM/DeepSeek 等 anthropic-compat 端点拒绝 → 400 code 1210 "API 调用参数有误"
    //     （DB 全样本交叉验证：9/9 失败均为 no_stream+assistant+messages 内 role=system；
    //      官方 Anthropic 接受该 CC 注入的非标位置，第三方严格）。规整=将 messages 内 role=system
    //     合并到顶层 system 数组（语义等价、Anthropic 规范形式），messages 数组移除该消息。
    //     仅非流式触发：流式 + 同结构当前工作正常（9279 PASS），不动避免回归。
    if matches!(target_protocol_enum, Protocol::Anthropic) && !is_official_anthropic_host(&url) {
        strip_thinking_if_unmatched(&mut req_body);
        // 无条件剥离 redacted_thinking content block：第三方 anthropic 端点（火山 doubao coding、
        // deepseek 等）不认该 Claude 4.x extended thinking 加密块 → 400 InvalidParameter
        // "invalid value: `redacted_thinking`"。同协议 passthrough 不走 to_anthropic 转换
        // （后者已 filter Unknown 含 redacted_thinking），content 原样透传即触发。redacted 内容
        // 加密 opaque 不可回放，剥离安全。trace 81dc4466 / 87e3c500 实证。
        strip_redacted_thinking_blocks(&mut req_body);
        if !is_stream {
            hoist_mid_messages_system(&mut req_body);
        }
    }

    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();

    // ── 解析超时：模型 > 分组 > 系统 ──（system_timeout + proxy_client 一次缓存借齐）
    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let (req_timeout, conn_timeout) = resolve_timeout(&route.mapping, group, &system_timeout);
    // 流式响应 body 读取不计入总超时：reqwest .timeout 覆盖「连接→响应头→body 全部读完」，
    // 会砍断长 thinking/tool_use 流（body 读取 > request_timeout_secs）致无 message_stop → 客户端
    // JSON Parse error / 内容残缺。流式禁总超时（传 0），connect_timeout 仍保护连接期，客户端自有超时兜底。
    let req_timeout = if is_stream { 0 } else { req_timeout };
    let client = super::http_client::build_http_client(
        &proxy_client, req_timeout, conn_timeout,
        Some(&route.platform.extra), None,
    ).await;

    // ── 构建上游请求头 ──
    // convert 路径：先铺底透传入站头（anthropic-* / x-stainless-* / x-app / session-id 等，
    // 跨协议也带，上游忽略未知头不报错），再由 apply_client_headers 覆盖 UA + auth + CT。
    // passthrough_convert_headers 已剔 hop-by-hop + auth/UA/CT（由下方覆盖），无同名多值。
    let upstream_headers = build_upstream_headers(&client_type, target_protocol_enum, &eff_api_key, orig_headers, &url);

    let mut req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .headers(passthrough_convert_headers(orig_headers, &url))
        .body(req_body_str.clone());

    // ── 覆盖 UA + auth（平台 api_key）──
    req_builder = apply_client_headers(req_builder, &client_type, target_protocol_enum, &eff_api_key);

    // ── 记录上游实际请求 ──
    log.upstream_request_headers = serde_json::Value::Object(
        upstream_headers.into_iter().map(|(k, v)| (k, Value::String(v))).collect()
    ).to_string();
    // ponytail: pretty 序列化仅当 log_upstream_request 开启时执行，关日志零开销
    log.upstream_request_body = if log_settings.log_upstream_request {
        format_pretty_json(&req_body_str)
    } else {
        String::new()
    };
    tracing::info!(method = "POST", url = %url, "upstream request");
    tracing::debug!(method = "POST", url = %url, body = %super::log_util::log_body_preview(&req_body_str), "upstream request body");

    // ── 熔断指标：本次 forward 尝试前在途 +1；解析本平台有效阈值 ──
    let breaker_th = {
        let (ft, os, hom) = sched_settings.effective_thresholds(&route.platform);
        super::scheduling::BreakerThresholds { failure_threshold: ft, open_secs: os, half_open_max: hom }
    };
    state.scheduler.inc_inflight(route.platform.id);

    // ── 发上游 + 同平台瞬时重试 ──
    // transport 错误（上游中途掐连接 / 连不上）先在同一平台原地重试 TRANSPORT_RETRY_MAX 次再
    // 换候选：单平台组没有 failover 候选，不原地重试则任何瞬断都直接 502 到客户端。
    // 重试期间不 record_failure（in-flight 仍 +1，本次尝试尚未定终态），仅最终失败时计一次。
    // try_clone 对本路径恒 Some（body 是 String，非 stream body）；None 时退化为不重试。
    let mut pending = Some(req_builder);
    let mut transport_retried = 0u32;
    let resp = loop {
        let builder = pending.take().expect("pending builder always set at loop head");
        let next_builder = if transport_retried < TRANSPORT_RETRY_MAX {
            builder.try_clone()
        } else {
            None
        };
        // 本轮尝试自身的耗时（非累计）：慢失败不重试的判据，见 is_transport_retryable。
        let send_start = std::time::Instant::now();
        match builder.send().await {
            Ok(r) => break r,
            Err(e) if is_transport_retryable(&e, send_start.elapsed()) && next_builder.is_some() => {
                let backoff = transport_retry_backoff(transport_retried);
                tracing::warn!(
                    url = %url, platform = %route.platform.name, error = %err_chain(&e),
                    retry = transport_retried + 1, backoff_ms = backoff.as_millis() as u64,
                    "upstream transport error, retrying same platform"
                );
                attempts.push(ProxyAttempt {
                    platform_id: route.platform.id,
                    platform_name: route.platform.name.clone(),
                    status_code: 0,
                    error: format!("upstream error (retrying): {}", err_chain(&e)),
                    duration_ms: attempt_start.elapsed().as_millis() as i64,
                    ts: attempt_ts,
                });
                transport_retried += 1;
                tokio::time::sleep(backoff).await;
                pending = next_builder;
                continue;
            }
            Err(e) => {
                // 同平台重试已用尽 / 错误不宜重试 → 换下个候选；候选耗尽则返回 502。
                // 熔断：连接失败 / 超时计一次失败（in-flight -1 + breaker fail 计数）。
                state.scheduler.record_failure(route.platform.id, &breaker_th, aidog_db::now());
                let detail = err_chain(&e);
                tracing::error!(url = %url, platform = %route.platform.name, error = %detail, duration_ms = start.elapsed().as_millis() as i64, "upstream request failed (502)");
                let upstream_err = format!("upstream error: {detail}");
                attempts.push(ProxyAttempt {
                    platform_id: route.platform.id,
                    platform_name: route.platform.name.clone(),
                    status_code: 0,
                    error: upstream_err.clone(),
                    duration_ms: attempt_start.elapsed().as_millis() as i64,
                    ts: attempt_ts,
                });
                let _ = aidog_db::set_platform_last_error(
                    &state.db, route.platform.id, Some(upstream_err.clone()),
                ).await;
                if !is_last_candidate {
                    return AttemptOutcome::Next;
                }
                let msg = format!("{}: {detail}", i18n::t(lang, ErrorKey::Upstream));
                return AttemptOutcome::Respond(
                    finalize_proxy_502(
                        state, log, attempts, route.platform.id,
                        upstream_err, msg, start, log_settings,
                    ).await,
                );
            }
        }
    };

    // ── 捕获上游响应 headers + status ──
    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    // builtin-tool-compat 运行时审计：4xx + 出站含 tools → blocked_by=upstream/tools_4xx
    builtin_tools::mark_tools_4xx(log, &req_body, status.as_u16());
    // clone 上游响应头，供回包前透传筛选用（resp 后续被 bytes()/bytes_stream() 消费）
    let upstream_resp_headers = resp.headers().clone();
    {
        let mut h = serde_json::Map::new();
        for (k, v) in resp.headers() {
            if let Ok(s) = v.to_str() {
                h.insert(k.to_string(), Value::String(s.to_string()));
            }
        }
        log.upstream_response_headers = Value::Object(h).to_string();
    }

    // ── 流式判定以实际上游响应为准：请求 body 的 stream 字段与上游响应 content-type 取并。
    //   中转站常对未声明 stream 的请求强制以 text/event-stream 响应；若仅凭请求字段会误判为
    //   非流式，进而用 JSON 解析 SSE 文本拿不到 usage → token/est_cost 全为 0。此处纠偏，
    //   使任何 SSE 响应都走流式 token 聚合路径。OR 语义保证既有正常流式路径不回归。──
    let upstream_ct = upstream_resp_headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_stream = resolve_is_stream(is_stream, upstream_ct);
    log.is_stream = is_stream;

    if !status.is_success() {
        return handle_non_success(
            resp, status, state, log, attempts, &route, group, &breaker_th, &url, start,
            attempt_start, attempt_ts, is_last_candidate, log_settings,
        )
        .await;
    }

    // ── 2xx：状态码成功，但「200 + 空/无效响应」按决策 B 仍当作失败重试。──
    // 成功记账（record_success / 恢复 auto_disabled / 清 strike / attempts.push 成功 / log.attempts）
    // 推迟到「确认非空有效响应」之后，由 commit_2xx_success! 宏统一执行（避免重复且保证仅真成功才记账）。
    let attempt_latency_ms = attempt_start.elapsed().as_millis() as i64;

    // 决策 B 失败（200 空响应）时记一次失败 attempt 并 failover；候选耗尽则返回 502。
    // 与连接错误/超时同语义：熔断计一次失败（record_failure），但不 auto_disable（非鉴权/死端点信号）。
    macro_rules! retry_on_empty_2xx {
        ($reason:expr, $upstream_text:expr) => {{
            state.scheduler.record_failure(route.platform.id, &breaker_th, aidog_db::now());
            tracing::warn!(
                platform = %route.platform.name, platform_id = route.platform.id,
                reason = $reason, "decision-B: upstream 200 but empty/invalid response, failover next platform"
            );
            attempts.push(ProxyAttempt {
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 200,
                error: $reason.to_string(),
                duration_ms: attempt_latency_ms,
                ts: attempt_ts,
            });
            let _ = aidog_db::set_platform_last_error(
                &state.db, route.platform.id, Some(format!("HTTP 200: {}", $reason)),
            ).await;
            if !is_last_candidate {
                return AttemptOutcome::Next;
            }
            // 候选耗尽：返回 502 + 已记录的 attempts（此时尚未向客户端发任何字节，安全）。
            log.platform_id = route.platform.id;
            log.status_code = 502;
            log.done = true;
            log.upstream_status_code = status.as_u16() as i32;
            let err_body = format!("{}: 200 but empty/invalid response", i18n::t(lang, ErrorKey::Upstream));
            // 取证：把上游真实首块原文截断（≤4KB + truncated 标记）落 response_body，替代占位文案；
            // upstream_text 为空时回退占位兜底。下次 GLM 间歇空流复现自动留 DB 证据。
            let captured = truncate_peek_text($upstream_text);
            log.response_body = if captured.is_empty() { $reason.to_string() } else { captured };
            log.user_response_body = err_body.clone();
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.retry_count = (attempts.len() as i32 - 1).max(0);
            log.attempts = std::mem::take(attempts);
            upsert_log(state, log, log_settings).await;
            return AttemptOutcome::Respond({
                let mut r = (StatusCode::BAD_GATEWAY, err_body).into_response();
                inject_trace_header(&mut r);
                r
            });
        }};
    }

    // 真成功记账：熔断成功 + 恢复 auto_disabled + attempts.push 成功 + 填 log.attempts。
    macro_rules! commit_2xx_success {
        () => {{
            // 熔断指标：成功 → 更新延迟 EMA + breaker Closed/HalfOpen→Closed + inflight-1。
            // 注意流式此处为「首个有效内容」延迟（peek 已收到内容）；作为延迟近似用于 LeastLatency。
            state.scheduler.record_success(route.platform.id, attempt_latency_ms);
            // 最近一次成功 → 清本平台 last_error。仅在原有 last_error 非空时写，避免成功热路径空写。
            if !route.platform.last_error.is_empty() {
                let _ = aidog_db::set_platform_last_error(&state.db, route.platform.id, None).await;
            }
            attempts.push(ProxyAttempt {
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: status.as_u16() as i32,
                error: String::new(),
                duration_ms: attempt_latency_ms,
                ts: attempt_ts,
            });
            if route.platform.status == super::models::PlatformStatus::AutoDisabled {
                if let Err(e) = aidog_db::recover_platform_auto_disabled(&state.db, route.platform.id).await {
                    tracing::error!(platform_id = route.platform.id, error = %e, "recover auto-disabled platform failed");
                } else {
                    tracing::info!(platform = %route.platform.name, platform_id = route.platform.id, "platform recovered from auto-disabled (2xx)");
                }
            }
            log.platform_id = route.platform.id;
            log.retry_count = (attempts.len() as i32 - 1).max(0);
            log.attempts = std::mem::take(attempts);
        }};
    }

    // forward_attempt 本次路由决策的协议/模型元数据，finish_nonstream / finish_stream 共用（见 finish.rs::AttemptCtx）。
    let attempt_ctx = AttemptCtx {
        source_protocol: source_protocol.clone(),
        target_protocol: target_protocol_enum.clone(),
        same_protocol_passthrough,
        coding_plan,
        requested_model: requested_model.to_string(),
        actual_model: actual_model.clone(),
        eff_api_key: eff_api_key.clone(),
        quota_base_url: target_base_url.clone(),
    };

    // 非流式：直接透传 JSON
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        // usage 借用：lossy 不经 to_string 中转（extract_usage 在 finish_nonstream 内）
        let lossy = String::from_utf8_lossy(&body);
        let resp_str: &str = &lossy;

        // ── 决策 B（非流式）：200 但空 body / error 结构 / 无有效 choices/content → 失败重试。──
        if !is_nonstream_body_valid(resp_str) {
            retry_on_empty_2xx!("200 but empty/invalid body", resp_str);
        }
        commit_2xx_success!();

        return AttemptOutcome::Respond(
            finish_nonstream(
                state, log, log_settings, group, &route, &attempt_ctx, &upstream_resp_headers, start, body,
            )
            .await,
        );
    }

    // ── 决策 B（流式）：提交转发前缓冲(peek)上游首个「有效内容」chunk 再决定。──
    // 在向客户端发任何字节前，先从上游 SSE 流拉取若干 chunk，扫描累积原文：
    //   - Meaningful（真实内容事件）→ 提交：把已缓冲的 chunk 原样 prepend 回流，继续既有 relay。
    //   - EmptyOrError（立即 [DONE] / 立即 error / 流秒断无内容 / 空 body）→ 当作失败 failover（header 未发，安全）。
    // 仅 peek 到「判定够了」即停（收到首个有效内容立即提交），不缓冲整条流（接受首字节延迟）。
    // 缓冲上限兜底：累计字节 / chunk 数到上限仍未判定 → 视为已产出内容，提交（避免饿死长 keepalive 流）。
    const PEEK_MAX_BYTES: usize = 64 * 1024;
    const PEEK_MAX_CHUNKS: usize = 64;
    let mut upstream_stream = resp.bytes_stream();
    let mut peek_buf: Vec<Bytes> = Vec::new();
    let mut peek_text = String::new();
    let mut peek_bytes = 0usize;
    let peek_decision = loop {
        match upstream_stream.next().await {
            Some(Ok(chunk)) => {
                peek_bytes += chunk.len();
                peek_text.push_str(&String::from_utf8_lossy(&chunk));
                peek_buf.push(chunk);
                match classify_stream_first(&peek_text, false) {
                    StreamPeek::Meaningful => break StreamPeek::Meaningful,
                    StreamPeek::EmptyOrError => break StreamPeek::EmptyOrError,
                    StreamPeek::NeedMore => {
                        if peek_bytes >= PEEK_MAX_BYTES || peek_buf.len() >= PEEK_MAX_CHUNKS {
                            // 上限兜底：已收到字节但未见明确内容/错误标记 → 保守提交，避免误杀长流。
                            break StreamPeek::Meaningful;
                        }
                    }
                }
            }
            // 上游流秒断（peek 期间出错）→ 与连接错误同语义，failover。
            Some(Err(e)) => {
                tracing::warn!(error = %e, "decision-B: upstream stream error during first-chunk peek");
                break StreamPeek::EmptyOrError;
            }
            // 流结束：用 stream_ended=true 收敛判定（无内容 → EmptyOrError）。
            None => break classify_stream_first(&peek_text, true),
        }
    };

    if peek_decision == StreamPeek::EmptyOrError {
        retry_on_empty_2xx!("200 but empty/invalid stream", &peek_text);
    }
    // Meaningful：确认上游真实产出 → 提交成功记账（在构建 guard 前，使 guard 的 log 快照含正确 attempts）。
    commit_2xx_success!();

    // 决策 B：把 peek 阶段已缓冲的首批 chunk 原样 prepend 回流（不能吞首块），再接上游剩余流；
    // finish_stream 内对缓冲块与后续块一视同仁（token 聚合 / 转换 / finalize 不受影响）。
    let buffered_head = futures::stream::iter(peek_buf.into_iter().map(Ok::<Bytes, reqwest::Error>));
    let full_stream = buffered_head.chain(upstream_stream);

    AttemptOutcome::Respond(
        finish_stream(
            full_stream, state, log, log_settings, group, &route, &attempt_ctx, &upstream_resp_headers, start,
        )
        .await,
    )
}

/// 候选耗尽时统一终态：填 log 502 字段 + upsert 落库 + 构造 502 Response（含 trace 头）。
///
/// 抽自 3 处同构 502 终态（invalid protocol / base_url missing / upstream send error），
/// 共享 8 个 log 字段 + upsert + (StatusCode::BAD_GATEWAY, msg) 响应。
/// 签名预留 `response_body` 与 `user_response_body` 分离：前者落库审计取证（上游原文/原因），
/// 后者返回客户端（i18n 友好文案）。`platform_id` 显式传入便于后续 devin/handler/passthrough 复用。
///
/// ponytail: 禁动内嵌宏 retry_on_empty_2xx! —— 其 502 分支含额外 upstream_status_code +
/// truncate_peek_text 取证逻辑，非完全同构；后续若需统一，扩参 `Option<(i32, String)>` 取证元组即可。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn finalize_proxy_502(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    platform_id: u64,
    response_body: String,
    user_response_body: String,
    start: std::time::Instant,
    log_settings: &ProxyLogSettings,
) -> axum::response::Response {
    log.platform_id = platform_id;
    log.response_body = response_body;
    log.status_code = 502;
    log.done = true;
    log.user_response_body = user_response_body.clone();
    log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.retry_count = (attempts.len() as i32 - 1).max(0);
    log.attempts = std::mem::take(attempts);
    upsert_log(state, log, log_settings).await;
    let mut r = (StatusCode::BAD_GATEWAY, user_response_body).into_response();
    inject_trace_header(&mut r);
    r
}

/// 第三方 anthropic 端点不支持字段剔除（仅在已判定为非官方 anthropic 端点时调用）。
///
/// **`context_management`（无条件剔）**：thinking 开启（`thinking.type != "disabled"`）即剔，
/// 独立于 assistant 历史是否齐全。`context_management` 是官方 Anthropic 服务端协商特性
/// （Claude Code adaptive/summarized 模式 `clear_thinking_20251015`，让官方服务端自动清历史 thinking），
/// 第三方 anthropic-compat 端点普遍不实现该协商，保留该字段对第三方无益仅风险。两类复现：
/// 首轮请求（messages 仅 user，无 assistant 历史）GLM 直拒字段 → 400 code 1210 "API 调用参数有误"
/// （旧逻辑 `has_unmatched_assistant`=false 漏剔 → 本次修复根因）；有 assistant 历史时 DeepSeek 认字段
/// 判 thinking mode → 400 "thinking must be passed back"。函数名沿用 `strip_thinking_if_unmatched`
/// （单调用点 forward.rs，最小 diff；context_management 已超越 thinking unmatched 语义，注释说明）。
///
/// **`thinking`（仅 unmatched 时剔）**：thinking 开启且历史任一 assistant 轮缺 thinking block 时剔。
/// 第三方端点严格要求 thinking 模式下每 assistant 轮回传 thinking block，缺失即 400
/// `content[].thinking must be passed back`；官方 Anthropic 的 summarized/adaptive 模式无此约束，
/// Claude Code 故不回传，跨路由到第三方即触发该 400。thinking block 齐全（正常情况）保留直传，
/// 第三方能正常处理。
fn strip_thinking_if_unmatched(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else { return };
    let thinking_on = obj
        .get("thinking")
        .map(|t| t.get("type").and_then(|v| v.as_str()) != Some("disabled"))
        .unwrap_or(false);
    if !thinking_on {
        return;
    }
    // context_management 无条件剔：第三方端点不认该协商字段（首轮 GLM 1210 + 有历史 DeepSeek 400）
    obj.remove("context_management");
    let has_unmatched_assistant = obj
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|msgs| {
            msgs.iter().any(|m| {
                if m.get("role").and_then(|r| r.as_str()) != Some("assistant") {
                    return false;
                }
                match m.get("content") {
                    // 块数组：非空且无 thinking block → 不匹配
                    Some(Value::Array(blocks)) => {
                        !blocks.is_empty()
                            && !blocks
                                .iter()
                                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("thinking"))
                    }
                    // 纯文本 assistant 轮：thinking 模式下也应携带 thinking，缺失即不匹配
                    Some(Value::String(s)) => !s.is_empty(),
                    _ => false,
                }
            })
        })
        .unwrap_or(false);
    if has_unmatched_assistant {
        obj.remove("thinking");
    }
}

/// 出站 body 上的 `max_tokens` 裁剪（透传与转换两分支共用）。
///
/// **为什么要在 body 上再裁一次**：`chat_req` 侧的裁剪只喂给 `convert_request`，同协议透传
/// 分支的 body 是客户端原体 `Value`，超限值原样上送 → 上游 400（票 02）。转换分支上此处
/// 幂等（chat_req 已裁到同一上限，不会二次收缩），故不需要按分支分叉。
///
/// 键名按目标 wire 协议分叉：
/// - anthropic / openai / openai_completions → 顶层 `max_tokens`
/// - openai_responses → 顶层 `max_output_tokens`
/// - gemini → `generationConfig.maxOutputTokens`
///
/// 上限口径由调用方给（`get_model_max_output_tokens(actual_model)`），保守语义同
/// [`super::router::cap_max_tokens`]：未传 / 模型无上限 / 未超限一律不动。
/// 返回 `Some((原值, 裁剪后值))` 表示发生了裁剪，`None` 表示 body 未被改动。
fn cap_body_max_tokens(body: &mut Value, model_max: Option<i64>, wire: &Protocol) -> Option<(u32, u32)> {
    let obj = body.as_object_mut()?;
    let slot = match wire {
        Protocol::Anthropic | Protocol::OpenAI | Protocol::OpenAICompletions => obj.get_mut("max_tokens"),
        Protocol::OpenAIResponses => obj.get_mut("max_output_tokens"),
        Protocol::Gemini => obj
            .get_mut("generationConfig")
            .and_then(|gc| gc.as_object_mut())
            .and_then(|gc| gc.get_mut("maxOutputTokens")),
        // 其余枚举值不是 wire 协议（平台类型），不会作为 endpoint 协议出现
        _ => None,
    }?;
    // 超 u32 的病态值按 u32::MAX 处理（照样会被上限裁下来）。
    let requested = u32::try_from(slot.as_u64()?).unwrap_or(u32::MAX);
    let (capped, did_cap) = super::router::cap_max_tokens(Some(requested), model_max);
    let capped = capped.filter(|_| did_cap)?;
    *slot = Value::from(capped);
    Some((requested, capped))
}

/// OpenAI wire 出站 body 上把 `max_completion_tokens` 折进 `max_tokens`（票 05）。
///
/// 转换分支上是 no-op（`to_openai` 只写 `max_tokens`）；生效点是同协议透传——新版 OpenAI
/// SDK 与 o 系列模型只发 `max_completion_tokens`，不折就绕过下游的模型上限裁剪。
///
/// 取值规则与入站 `from_openai` 同源：**两键同时存在时取 `max_completion_tokens`**
/// （新键是客户端有意设置的那个，旧键多是 SDK 为兼容老服务端保留的镜像值）。
/// 折完 body 上只剩 `max_tokens`，官方 host 需要的键名由
/// [`rename_openai_max_tokens_key`] 在链路末端改回。
fn fold_openai_max_completion_tokens(body: &mut Value, wire: &Protocol) {
    if !matches!(wire, Protocol::OpenAI) {
        return;
    }
    let Some(obj) = body.as_object_mut() else { return };
    if let Some(v) = obj.remove("max_completion_tokens")
        && !v.is_null()
    {
        obj.insert("max_tokens".to_string(), v);
    }
}

/// 官方 OpenAI Chat Completions 的输出长度键改写（票 05）。
///
/// 官方端点已把 `max_tokens` 标为 deprecated，o 系列等新模型直接拒绝该参数，只认
/// `max_completion_tokens`；第三方 OpenAI 兼容端点与 legacy `/v1/completions`
/// 反过来只认 `max_tokens`。故仅 `wire == OpenAI` 且 host 为 `api.openai.com` 时改键，
/// 值不动（此时值已是裁剪后的最终值）。host 判定复用 [`is_official_openai_host`]。
///
/// 返回是否改写过。
fn rename_openai_max_tokens_key(body: &mut Value, wire: &Protocol, upstream_url: &str) -> bool {
    if !matches!(wire, Protocol::OpenAI) || !is_official_openai_host(upstream_url) {
        return false;
    }
    let Some(obj) = body.as_object_mut() else { return false };
    let Some(v) = obj.remove("max_tokens") else { return false };
    obj.insert("max_completion_tokens".to_string(), v);
    true
}

/// `disable_thinking` 请求字段处理（aidog 本地扩展）。字段存在即剥（非标，透传恐 400）；
/// 值为 true 时先剔掉客户端带来的开启型思考参数，再按目标 wire 协议写入**显式禁用参数**。
///
/// **为什么不只剔参数**（用户决策 2026-08-26，request d1c87c9c 实证）：旧实现只剔不写，
/// 上游收不到任何禁用指令 → 按自身默认（思考开启）执行。实测 MiniMax-M2 把 300 max_tokens
/// 全烧在 thinking block 上，正文一个字没输出。语义因此改为「按协议显式告知上游关闭思考」。
///
/// 协议映射：
/// - anthropic → `thinking: {"type": "disabled"}`
/// - gemini → `generationConfig.thinkingConfig.thinkingBudget = 0`
/// - openai_responses → `reasoning: {"effort": "none"}`
/// - openai / openai_completions → 官方 host 用 `reasoning_effort: "none"`（官方拒未知顶层字段），
///   第三方 OpenAI 兼容端点用 `chat_template_kwargs.enable_thinking = false`（vLLM/SGLang/GLM/Qwen 通行写法）
///
/// 上游硬限制不在本函数职责内：MiniMax M2.x 接受 `thinking.type=disabled` 但模型照样思考，
/// 响应侧不做剥离（用户决策：剥了也救不回被思考烧光的 max_tokens）。
///
/// **`disable` 由调用方从客户端原体读**：转换分支的 `req_body` 由强类型 struct 序列化而来
/// （`adapter::convert_request`），`disable_thinking` 这种未建模的 aidog 私有字段在那一步就被丢掉，
/// 从 `body` 读 key 在转换分支恒为 false（审计实证：本仓 707/1016 请求走 anthropic→openai 转换）。
/// 函数内仍剥 `body` 上残留的同名 key，透传分支靠它把非标字段挡在上游之外。
fn apply_disable_thinking(body: &mut Value, disable: bool, wire: &Protocol, upstream_url: &str) {
    let Some(obj) = body.as_object_mut() else { return };
    obj.remove("disable_thinking");
    if !disable {
        return;
    }
    for k in ["thinking", "context_management", "reasoning_effort", "reasoning", "enable_thinking"] {
        obj.remove(k);
    }
    if let Some(ctk) = obj.get_mut("chat_template_kwargs").and_then(|o| o.as_object_mut()) {
        ctk.remove("enable_thinking");
    }
    if let Some(gc) = obj.get_mut("generationConfig").and_then(|o| o.as_object_mut()) {
        gc.remove("thinkingConfig");
    }

    match wire {
        Protocol::Anthropic => {
            obj.insert("thinking".to_string(), serde_json::json!({"type": "disabled"}));
        }
        Protocol::Gemini => {
            let gc = obj
                .entry("generationConfig".to_string())
                .or_insert_with(|| serde_json::json!({}));
            if let Some(gc) = gc.as_object_mut() {
                gc.insert("thinkingConfig".to_string(), serde_json::json!({"thinkingBudget": 0}));
            }
        }
        Protocol::OpenAIResponses => {
            obj.insert("reasoning".to_string(), serde_json::json!({"effort": "none"}));
        }
        Protocol::OpenAI | Protocol::OpenAICompletions => {
            if is_official_openai_host(upstream_url) {
                obj.insert("reasoning_effort".to_string(), Value::String("none".to_string()));
            } else {
                let ctk = obj
                    .entry("chat_template_kwargs".to_string())
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(ctk) = ctk.as_object_mut() {
                    ctk.insert("enable_thinking".to_string(), Value::Bool(false));
                }
            }
        }
        // 其余枚举值不是 wire 协议（平台类型），不会作为 endpoint 协议出现
        _ => {}
    }
}

/// 官方 OpenAI 端点判定（host == api.openai.com）。官方 Chat Completions 对未知顶层字段
/// 返回 400 `unknown_parameter`，因此禁思考只能走官方认的 `reasoning_effort`。
/// host 提取与 `is_official_anthropic_host` 同 idiom。
fn is_official_openai_host(upstream_url: &str) -> bool {
    let after_scheme = upstream_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(upstream_url);
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority)
        .split(':')
        .next()
        .unwrap_or("");
    host.eq_ignore_ascii_case("api.openai.com")
}

/// 未建模顶层字段兜底透传：按**目标 wire 协议的允许集合**从客户端原体补齐出站 body（票 01）。
///
/// **为什么需要**：`ChatRequest` 是白名单强类型模型，`stop` / `top_k` / `seed` /
/// `response_format` / `stream_options` / `presence_penalty` / `frequency_penalty` / `n` /
/// `user` 全无对应字段，五个 wire struct 也都是封闭 struct（无 flatten 出口），
/// 转换分支上这些参数在 `parse_incoming_request` → `convert_request` 之间静默消失
/// （全库 grep 实测零命中）。
///
/// **为什么是白名单而不是全量倒出**：官方 OpenAI Chat Completions 对未知顶层字段返回 400
/// `unknown_parameter`（同 `apply_disable_thinking` 的 host gate 理由），全量倒出会把现在
/// 能用的链路打坏。
///
/// 逐协议允许集合：
/// - anthropic → `stop_sequences`（客户端写 `stop` 时换名 + 字符串包成数组）、`top_k`
/// - openai / openai_completions → `stop`（客户端写 `stop_sequences` 时换名）、`seed`、
///   `stream_options`、`presence_penalty`、`frequency_penalty`、`n`、`user`；
///   `response_format` 仅 chat completions（legacy completions 无此参数）；
///   `top_k` 仅第三方 OpenAI 兼容端点（vLLM/SGLang/GLM/Qwen 通行，官方不认）
/// - openai_responses → `user`（Responses API 不收 stop / 惩罚项 / n / seed / response_format）
/// - gemini → 顶层无本组字段（对应键全在 `generationConfig` 下，归票 09，本函数不碰）
///
/// **只补不覆盖、只加不删**：出站 body 已有该键就不动（强类型字段优先）；
/// 透传分支客户端原体的其它字段原样保留（本函数不做剔除，避免打坏现有透传链路）。
///
/// **`src` 是客户端原体**（`req_value`），不是 `req_body`——同 `apply_disable_thinking` 的
/// 调用方约束：转换分支的 `req_body` 由 wire struct 序列化而来，未建模字段在那里已经没了。
fn apply_field_passthrough(body: &mut Value, src: &Value, wire: &Protocol, upstream_url: &str) {
    let Some(src_obj) = src.as_object() else { return };
    let Some(obj) = body.as_object_mut() else { return };

    // ① 停止序列：anthropic 用 stop_sequences（只收数组），openai 族用 stop（字符串或数组）
    let stop_key = match wire {
        Protocol::Anthropic => Some("stop_sequences"),
        Protocol::OpenAI | Protocol::OpenAICompletions => Some("stop"),
        _ => None,
    };
    if let Some(key) = stop_key.filter(|k| !obj.contains_key(*k)) {
        // 取值时先认目标协议的写法，再回退另一族的写法
        let raw = if key == "stop_sequences" {
            src_obj.get("stop_sequences").or_else(|| src_obj.get("stop"))
        } else {
            src_obj.get("stop").or_else(|| src_obj.get("stop_sequences"))
        };
        let normalized = match raw {
            Some(Value::String(s)) if key == "stop_sequences" => {
                Some(Value::Array(vec![Value::String(s.clone())]))
            }
            Some(v @ (Value::String(_) | Value::Array(_))) => Some(v.clone()),
            // 其它形态（数字 / 对象 / null）不是任何一家的合法值，不写出
            _ => None,
        };
        if let Some(v) = normalized {
            obj.insert(key.to_string(), v);
        }
    }

    // ② 其余顶层键：同名补齐
    const OPENAI_COMMON: [&str; 6] = [
        "seed",
        "stream_options",
        "presence_penalty",
        "frequency_penalty",
        "n",
        "user",
    ];
    let third_party = !is_official_openai_host(upstream_url);
    let allow: Vec<&str> = match wire {
        Protocol::Anthropic => vec!["top_k"],
        Protocol::OpenAI => {
            let mut v = OPENAI_COMMON.to_vec();
            v.push("response_format");
            if third_party {
                v.push("top_k");
            }
            v
        }
        Protocol::OpenAICompletions => {
            let mut v = OPENAI_COMMON.to_vec();
            if third_party {
                v.push("top_k");
            }
            v
        }
        Protocol::OpenAIResponses => vec!["user"],
        // gemini 顶层无本组字段；其余枚举值不是 wire 协议（平台类型），不会作为 endpoint 协议出现
        _ => vec![],
    };
    for k in allow {
        if obj.contains_key(k) {
            continue;
        }
        if let Some(v) = src_obj.get(k) {
            obj.insert(k.to_string(), v.clone());
        }
    }
}

/// 第三方 anthropic 端点：无条件剥离 messages[].content 内 `redacted_thinking` block。
///
/// **根因（DB 响应体实证）**：Claude 4.x extended thinking 多轮请求含 `redacted_thinking`
/// content block（Claude Code 回传上轮 protected thinking 的加密关联块）。同协议 passthrough
/// （anthropic→anthropic, remap=true）不经 `to_anthropic` 转换（adapter/anthropic.rs 已 filter
/// Unknown 含 redacted_thinking），content 原样透传 → 第三方端点不认该 type → 400 InvalidParameter
/// `"invalid value: 'redacted_thinking', supported values: 'text','thinking','image','tool_use','tool_result'"`。
///
/// **trace 实证**：81dc4466（火山 doubao coding endpoint）+ 87e3c500（deepseek-v4-pro-260425）
/// 同根因 400。所有第三方 anthropic-compat 端点共性。
///
/// **剥离语义**：redacted_thinking 内容为客户端不可解读的加密 opaque blob（仅官方 Anthropic
/// 能关联上轮 protected thinking），第三方必无法处理，无条件剥离安全。仅遍历数组形态 content
/// （字符串形态无 block 可剔）。content 变空数组时保留 message 结构（剥离顺序敏感，下游规整
/// 依赖 message 序列完整）。
fn strip_redacted_thinking_blocks(body: &mut Value) {
    let Some(msgs) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
        return;
    };
    for m in msgs.iter_mut() {
        let Some(blocks) = m.get_mut("content").and_then(|c| c.as_array_mut()) else {
            continue;
        };
        blocks.retain(|b| b.get("type").and_then(|t| t.as_str()) != Some("redacted_thinking"));
    }
}

/// 第三方 anthropic 端点：messages 内非首位的 role=system 规整到顶层 system 数组。
///
/// **根因（DB 全样本取证）**：Claude Code 把 SessionStart/UserPromptSubmit hook 注入的上下文
/// 以 `role=system` 消息插入 messages 数组中段/末尾（官方 Anthropic 接受该非标位置作为客户端
/// 约定）。GLM / DeepSeek 等第三方 anthropic-compat 端点严格执行规范（role=system 仅顶层 system
/// 字段或 messages[0]），多轮 + 非流式场景下拒绝 → 400 code 1210 "API 调用参数有误"。
///
/// **DB 交叉验证**（GLM `open.bigmodel.cn/api/anthropic`，10552 条 200 + 9 条 400 全样本）：
/// 失败全集 = `{no_stream, has_assistant, messages 含 role=system（含中段+末段）}` —— 9/9 命中；
/// 同结构流式 PASS=1166，非流式 PASS=3（GLM 间歇性接受，3 异常样本均为 14-112 msgs 长上下文）。
/// 故仅非流式触发规整：流式同结构当前工作正常（host-gated 但 is_stream=true 不动），避免回归。
///
/// **规整方式**：messages 内 role=system 消息按出现顺序，content 合并到顶层 `system` 数组
/// （顶层 system 已是数组则追加 text block；字符串则升级为数组；缺失则新建）。
/// messages 数组移除该消息，剩余 user/assistant 交替保持原序。仅多轮（含 assistant）才触发：
/// 首轮无 assistant 时 messages 内 role=system 多为客户端约首约定（DeepSeek/GLM 首轮接受），不动。
fn hoist_mid_messages_system(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else { return };
    let Some(msgs) = obj.get_mut("messages").and_then(|m| m.as_array_mut()) else { return };
    // 仅多轮（有 assistant 历史）触发：首轮无 assistant 不动（首轮 role=system 第三方接受）。
    let has_assistant = msgs.iter().any(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"));
    if !has_assistant {
        return;
    }
    // 收集 messages 内 role=system 的 content（保持出现顺序），同时保留非 system 消息原序。
    let mut hoisted_blocks: Vec<Value> = Vec::new();
    let mut kept: Vec<Value> = Vec::with_capacity(msgs.len());
    for m in msgs.drain(..) {
        if m.get("role").and_then(|r| r.as_str()) == Some("system") {
            // system message content：字符串 → text block；数组（blocks） → 逐项取
            match m.get("content") {
                Some(Value::String(s)) => {
                    hoisted_blocks.push(serde_json::json!({"type": "text", "text": s}));
                }
                Some(Value::Array(blocks)) => {
                    for b in blocks {
                        if b.is_object() {
                            hoisted_blocks.push(b.clone());
                        }
                    }
                }
                _ => {}
            }
        } else {
            kept.push(m);
        }
    }
    if hoisted_blocks.is_empty() {
        // 无 system 可规整：还原原 msgs（drain 清空了）
        *msgs = kept;
        return;
    }
    *msgs = kept;
    // 合并到顶层 system 数组：现有数组追加；字符串升级；缺失新建。
    match obj.get_mut("system") {
        Some(Value::Array(arr)) => arr.extend(hoisted_blocks),
        Some(Value::String(s)) => {
            let mut arr = vec![serde_json::json!({"type": "text", "text": s})];
            arr.extend(hoisted_blocks);
            obj.insert("system".to_string(), Value::Array(arr));
        }
        _ => {
            obj.insert("system".to_string(), Value::Array(hoisted_blocks));
        }
    }
}

#[cfg(test)]
mod test_cap_body_max_tokens {
    use super::{cap_body_max_tokens, Protocol};
    use serde_json::json;

    /// anthropic 透传 body：超模型上限的 max_tokens 被裁到上限（票 02 的 400 根因）。
    #[test]
    fn anthropic_caps_top_level_max_tokens() {
        let mut b = json!({"model": "m", "max_tokens": 200_000, "messages": []});
        assert_eq!(cap_body_max_tokens(&mut b, Some(8192), &Protocol::Anthropic), Some((200_000, 8192)));
        assert_eq!(b["max_tokens"], json!(8192));
        assert_eq!(b["messages"], json!([]), "同级其它字段不动");
    }

    /// openai / completions 同样是顶层 `max_tokens`。
    #[test]
    fn openai_and_completions_cap_top_level_max_tokens() {
        for wire in [Protocol::OpenAI, Protocol::OpenAICompletions] {
            let mut b = json!({"model": "m", "max_tokens": 9999});
            assert_eq!(cap_body_max_tokens(&mut b, Some(4096), &wire), Some((9999, 4096)));
            assert_eq!(b["max_tokens"], json!(4096), "{wire:?}");
        }
    }

    /// openai_responses 的键名是 `max_output_tokens`。
    #[test]
    fn responses_caps_max_output_tokens() {
        let mut b = json!({"model": "m", "max_output_tokens": 100_000, "max_tokens": 100_000});
        assert_eq!(cap_body_max_tokens(&mut b, Some(16384), &Protocol::OpenAIResponses), Some((100_000, 16384)));
        assert_eq!(b["max_output_tokens"], json!(16384));
        assert_eq!(b["max_tokens"], json!(100_000), "responses 不碰顶层 max_tokens");
    }

    /// gemini 的键名嵌在 `generationConfig.maxOutputTokens`。
    #[test]
    fn gemini_caps_generation_config_max_output_tokens() {
        let mut b = json!({"generationConfig": {"maxOutputTokens": 65536, "temperature": 0.5}});
        assert_eq!(cap_body_max_tokens(&mut b, Some(8192), &Protocol::Gemini), Some((65536, 8192)));
        assert_eq!(b["generationConfig"]["maxOutputTokens"], json!(8192));
        assert_eq!(b["generationConfig"]["temperature"], json!(0.5), "同级其它字段不动");
    }

    /// 未超限 / 模型无上限 / 未传 max_tokens：body 整体不动（保守语义，与 chat_req 侧一致）。
    #[test]
    fn under_limit_or_no_limit_or_absent_is_noop() {
        let cases = [
            (json!({"model": "m", "max_tokens": 100}), Some(8192)),
            (json!({"model": "m", "max_tokens": 100}), None),
            (json!({"model": "m"}), Some(8192)),
            (json!({"model": "m", "max_tokens": null}), Some(8192)),
        ];
        for (original, model_max) in cases {
            let mut b = original.clone();
            assert_eq!(cap_body_max_tokens(&mut b, model_max, &Protocol::Anthropic), None);
            assert_eq!(b, original, "未超限/无上限/未传时 body 必须逐字节不变");
        }
    }

    /// 非 wire 协议（平台类型枚举）不改写。
    #[test]
    fn non_wire_protocol_is_noop() {
        let mut b = json!({"max_tokens": 999_999});
        assert_eq!(cap_body_max_tokens(&mut b, Some(8), &Protocol::Mock), None);
        assert_eq!(b["max_tokens"], json!(999_999));
    }
}

#[cfg(test)]
mod test_field_passthrough {
    use super::{apply_field_passthrough, Protocol};
    use serde_json::json;

    /// 客户端原体：一份同时带 openai 族与 anthropic 族写法的采样参数。
    fn client_body() -> serde_json::Value {
        json!({
            "model": "m",
            "messages": [],
            "stop": ["\n\n"],
            "top_k": 40,
            "seed": 7,
            "response_format": {"type": "json_object"},
            "stream_options": {"include_usage": true},
            "presence_penalty": 0.5,
            "frequency_penalty": -0.25,
            "n": 2,
            "user": "u-1",
            "wild_unknown_field": "should-never-be-forwarded"
        })
    }

    /// anthropic 目标：`stop` 换名 `stop_sequences`，`top_k` 原名补齐；
    /// 允许集合外的 openai 专属参数（seed / n / 惩罚项 / response_format）一个都不出现。
    #[test]
    fn anthropic_renames_stop_and_keeps_only_its_allow_set() {
        let src = client_body();
        let mut b = json!({"model": "m", "messages": [], "max_tokens": 100});
        apply_field_passthrough(&mut b, &src, &Protocol::Anthropic, "https://api.anthropic.com/v1/messages");
        assert_eq!(b["stop_sequences"], json!(["\n\n"]));
        assert_eq!(b["top_k"], json!(40));
        for k in ["stop", "seed", "response_format", "stream_options", "presence_penalty", "frequency_penalty", "n", "user", "wild_unknown_field"] {
            assert!(b.get(k).is_none(), "anthropic 允许集合外字段 {k} 不应出现");
        }
    }

    /// anthropic 只收数组形态的 stop_sequences：客户端写字符串要包成单元素数组。
    #[test]
    fn anthropic_wraps_string_stop_into_array() {
        let src = json!({"stop": "END"});
        let mut b = json!({"model": "m"});
        apply_field_passthrough(&mut b, &src, &Protocol::Anthropic, "https://api.anthropic.com/v1/messages");
        assert_eq!(b["stop_sequences"], json!(["END"]));
    }

    /// 第三方 OpenAI 兼容端点：openai 族允许集合全量补齐，`top_k` 也放行（vLLM/GLM 通行）。
    #[test]
    fn third_party_openai_fills_full_allow_set() {
        let src = client_body();
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(&mut b, &src, &Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(b["stop"], json!(["\n\n"]));
        assert_eq!(b["top_k"], json!(40));
        assert_eq!(b["seed"], json!(7));
        assert_eq!(b["response_format"], json!({"type": "json_object"}));
        assert_eq!(b["stream_options"], json!({"include_usage": true}));
        assert_eq!(b["presence_penalty"], json!(0.5));
        assert_eq!(b["frequency_penalty"], json!(-0.25));
        assert_eq!(b["n"], json!(2));
        assert_eq!(b["user"], json!("u-1"));
        assert!(b.get("wild_unknown_field").is_none(), "允许集合外字段不透传");
    }

    /// anthropic 客户端 → openai 上游：`stop_sequences` 换名成 `stop`。
    #[test]
    fn openai_renames_stop_sequences_to_stop() {
        let src = json!({"stop_sequences": ["\n\nHuman:"]});
        let mut b = json!({"model": "m"});
        apply_field_passthrough(&mut b, &src, &Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(b["stop"], json!(["\n\nHuman:"]));
        assert!(b.get("stop_sequences").is_none());
    }

    /// 官方 OpenAI host 回归防线：`top_k` 不是官方参数（未知顶层字段 → 400），必须挡住。
    #[test]
    fn official_openai_excludes_top_k() {
        let src = client_body();
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(&mut b, &src, &Protocol::OpenAI, "https://api.openai.com/v1/chat/completions");
        assert!(b.get("top_k").is_none(), "官方 OpenAI 允许集合不含 top_k");
        assert!(b.get("wild_unknown_field").is_none());
        assert_eq!(b["seed"], json!(7), "官方文档有的参数照常补齐");
        assert_eq!(b["user"], json!("u-1"));
    }

    /// legacy completions 无 `response_format` 参数，不写出。
    #[test]
    fn completions_excludes_response_format() {
        let src = client_body();
        let mut b = json!({"model": "m", "prompt": "x"});
        apply_field_passthrough(&mut b, &src, &Protocol::OpenAICompletions, "https://api.openai.com/v1/completions");
        assert!(b.get("response_format").is_none());
        assert_eq!(b["stop"], json!(["\n\n"]));
    }

    /// Responses API 只认 `user`，stop / 惩罚项 / n / seed / response_format 全挡。
    #[test]
    fn responses_allows_only_user() {
        let src = client_body();
        let mut b = json!({"model": "m", "input": []});
        apply_field_passthrough(&mut b, &src, &Protocol::OpenAIResponses, "https://api.openai.com/v1/responses");
        assert_eq!(b["user"], json!("u-1"));
        for k in ["stop", "stop_sequences", "top_k", "seed", "response_format", "stream_options", "presence_penalty", "frequency_penalty", "n"] {
            assert!(b.get(k).is_none(), "responses 允许集合外字段 {k} 不应出现");
        }
    }

    /// gemini 顶层无本组字段（对应键在 generationConfig 下，归票 09），本函数整体 no-op。
    #[test]
    fn gemini_top_level_is_noop() {
        let src = client_body();
        let mut b = json!({"contents": [], "generationConfig": {"temperature": 0.3}});
        let before = b.clone();
        apply_field_passthrough(&mut b, &src, &Protocol::Gemini, "https://generativelanguage.googleapis.com/v1beta");
        assert_eq!(b, before);
    }

    /// 已建模字段不被覆盖：出站 body 已有该键时保持强类型字段的值。
    #[test]
    fn modeled_fields_are_not_overwritten() {
        let src = json!({"stop": ["from-client"], "user": "from-client", "top_k": 40});
        let mut b = json!({"model": "m", "stop": ["already-modeled"], "user": "already-modeled", "top_k": 1});
        apply_field_passthrough(&mut b, &src, &Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(b["stop"], json!(["already-modeled"]));
        assert_eq!(b["user"], json!("already-modeled"));
        assert_eq!(b["top_k"], json!(1));
    }

    /// 透传分支：客户端原体即出站 body，本函数不得删掉任何既有字段（只补不删）。
    #[test]
    fn passthrough_body_keeps_its_own_unknown_fields() {
        let src = client_body();
        let mut b = src.clone();
        apply_field_passthrough(&mut b, &src, &Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(b["wild_unknown_field"], json!("should-never-be-forwarded"));
        assert_eq!(b, src, "透传分支上本函数是 no-op");
    }

    /// 客户端没设这些参数时不凭空造键。
    #[test]
    fn absent_client_fields_are_not_invented() {
        let src = json!({"model": "m", "messages": []});
        let mut b = json!({"model": "m", "messages": []});
        apply_field_passthrough(&mut b, &src, &Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(b, json!({"model": "m", "messages": []}));
    }
}

#[cfg(test)]
mod test_openai_max_completion_tokens {
    use super::{
        cap_body_max_tokens, fold_openai_max_completion_tokens, rename_openai_max_tokens_key,
        Protocol,
    };
    use aidog_adapter::converter::{convert_request, parse_incoming_request};
    use serde_json::json;

    const OFFICIAL: &str = "https://api.openai.com/v1";
    const THIRD_PARTY: &str = "https://open.bigmodel.cn/api/paas/v4";

    /// 透传 body 上两键同时存在 → 取 `max_completion_tokens`（与入站 `from_openai` 同规则）。
    #[test]
    fn both_keys_present_prefers_max_completion_tokens() {
        let mut b = json!({"model": "m", "max_tokens": 100, "max_completion_tokens": 9000});
        fold_openai_max_completion_tokens(&mut b, &Protocol::OpenAI);
        assert_eq!(b["max_tokens"], json!(9000));
        assert!(b.get("max_completion_tokens").is_none(), "折叠后旧键不残留: {b}");
    }

    /// 回归防线：只发 `max_tokens` 时 body 逐字节不变。
    #[test]
    fn max_tokens_only_body_unchanged() {
        let original = json!({"model": "m", "max_tokens": 777});
        let mut b = original.clone();
        fold_openai_max_completion_tokens(&mut b, &Protocol::OpenAI);
        assert_eq!(b, original, "无新键时 body 逐字节不变");
    }

    /// 与 cap 链路衔接：入站只发 `max_completion_tokens` 且超模型上限 →
    /// 裁剪作用在识别后的值上（转换分支 chat_req 侧口径由 body 侧幂等复裁锚住）。
    #[test]
    fn cap_applies_to_recognized_value() {
        let body = json!({
            "model": "gpt-5", "max_completion_tokens": 200_000,
            "messages": [{"role": "user", "content": "hi"}]
        });
        let req = parse_incoming_request(&Protocol::OpenAI, &body).expect("parse");
        let (mut out, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
        assert_eq!(
            cap_body_max_tokens(&mut out, Some(8192), &Protocol::OpenAI),
            Some((200_000, 8192)),
            "cap 没作用在归一后的值上: {out}"
        );
        assert_eq!(out["max_tokens"], json!(8192));

        // 透传分支：原体只有新键，折叠后同样被裁
        let mut b = json!({"model": "gpt-5", "max_completion_tokens": 200_000});
        fold_openai_max_completion_tokens(&mut b, &Protocol::OpenAI);
        assert_eq!(cap_body_max_tokens(&mut b, Some(8192), &Protocol::OpenAI), Some((200_000, 8192)));
        assert_eq!(b["max_tokens"], json!(8192));
    }

    /// 出站键名按目标分叉：官方 OpenAI host 改成 `max_completion_tokens`，第三方保持 `max_tokens`。
    #[test]
    fn official_host_renames_third_party_keeps_max_tokens() {
        let mut b = json!({"model": "m", "max_tokens": 8192, "temperature": 0.5});
        assert!(rename_openai_max_tokens_key(&mut b, &Protocol::OpenAI, OFFICIAL));
        assert_eq!(b["max_completion_tokens"], json!(8192));
        assert!(b.get("max_tokens").is_none(), "官方 host 不应残留旧键: {b}");
        assert_eq!(b["temperature"], json!(0.5), "同级其它字段不动");

        let original = json!({"model": "m", "max_tokens": 8192});
        let mut b = original.clone();
        assert!(!rename_openai_max_tokens_key(&mut b, &Protocol::OpenAI, THIRD_PARTY));
        assert_eq!(b, original, "第三方端点 body 逐字节不变");
    }

    /// 其它 wire 协议与「没有 max_tokens」时不动（anthropic 只认 max_tokens）。
    #[test]
    fn other_wires_and_absent_key_are_noop() {
        for wire in [Protocol::Anthropic, Protocol::OpenAICompletions, Protocol::Gemini] {
            let original = json!({"model": "m", "max_tokens": 8192});
            let mut b = original.clone();
            assert!(!rename_openai_max_tokens_key(&mut b, &wire, OFFICIAL));
            assert_eq!(b, original, "{wire:?}: 非 openai wire 不应改键");
            fold_openai_max_completion_tokens(&mut b, &wire);
            assert_eq!(b, original, "{wire:?}: 非 openai wire 不应折叠");
        }
        let mut b = json!({"model": "m"});
        assert!(!rename_openai_max_tokens_key(&mut b, &Protocol::OpenAI, OFFICIAL));
        assert_eq!(b, json!({"model": "m"}), "未传 max_tokens 时不产键");
    }
}

#[cfg(test)]
mod test_disable_thinking {
    use super::{apply_disable_thinking, is_official_openai_host, Protocol};
    use serde_json::json;

    /// anthropic：客户端开启型参数剔干净后，写入显式 `thinking.type=disabled`
    /// （request d1c87c9c 的 minimax anthropic 端点场景）。
    #[test]
    fn anthropic_emits_explicit_disabled() {
        let mut b = json!({
            "model": "m", "max_tokens": 300, "disable_thinking": true,
            "thinking": {"type": "enabled", "budget_tokens": 1024},
            "context_management": {"edits": []},
        });
        apply_disable_thinking(&mut b, true, &Protocol::Anthropic, "https://api.minimax.io/anthropic");
        assert!(b.get("disable_thinking").is_none());
        assert!(b.get("context_management").is_none());
        assert_eq!(b["thinking"], json!({"type": "disabled"}));
    }

    /// 第三方 OpenAI 兼容端点：`chat_template_kwargs.enable_thinking=false`，开启型参数不残留。
    #[test]
    fn third_party_openai_emits_chat_template_kwargs() {
        let mut b = json!({
            "model": "m", "disable_thinking": true, "reasoning_effort": "medium",
            "chat_template_kwargs": {"enable_thinking": true, "other": 1},
        });
        apply_disable_thinking(&mut b, true, &Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4");
        assert!(b.get("reasoning_effort").is_none());
        assert_eq!(b["chat_template_kwargs"]["enable_thinking"], json!(false));
        assert_eq!(b["chat_template_kwargs"]["other"], json!(1), "同级其它字段不动");
    }

    /// 官方 OpenAI：拒未知顶层字段，只能发 `reasoning_effort="none"`。
    #[test]
    fn official_openai_emits_reasoning_effort() {
        let mut b = json!({"model": "gpt-5", "disable_thinking": true});
        apply_disable_thinking(&mut b, true, &Protocol::OpenAI, "https://api.openai.com/v1");
        assert_eq!(b["reasoning_effort"], json!("none"));
        assert!(b.get("chat_template_kwargs").is_none());
    }

    /// openai_responses：`reasoning.effort="none"`。
    #[test]
    fn openai_responses_emits_reasoning_effort_object() {
        let mut b = json!({"model": "m", "disable_thinking": true, "reasoning": {"effort": "high"}});
        apply_disable_thinking(&mut b, true, &Protocol::OpenAIResponses, "https://api.openai.com/v1");
        assert_eq!(b["reasoning"], json!({"effort": "none"}));
    }

    /// gemini：`generationConfig.thinkingConfig.thinkingBudget=0`，generationConfig 缺省时补建。
    #[test]
    fn gemini_emits_zero_thinking_budget() {
        let mut b = json!({
            "disable_thinking": true,
            "generationConfig": {"thinkingConfig": {"thinkingBudget": 128}, "temperature": 0.5},
        });
        apply_disable_thinking(&mut b, true, &Protocol::Gemini, "https://generativelanguage.googleapis.com");
        assert_eq!(b["generationConfig"]["thinkingConfig"], json!({"thinkingBudget": 0}));
        assert_eq!(b["generationConfig"]["temperature"], json!(0.5));

        let mut b = json!({"disable_thinking": true});
        apply_disable_thinking(&mut b, true, &Protocol::Gemini, "https://generativelanguage.googleapis.com");
        assert_eq!(b["generationConfig"]["thinkingConfig"]["thinkingBudget"], json!(0));
    }

    #[test]
    fn false_strips_field_only_and_absent_is_noop() {
        let mut b = json!({"disable_thinking": false, "thinking": {"type": "enabled"}});
        apply_disable_thinking(&mut b, false, &Protocol::Anthropic, "https://example.com");
        assert!(b.get("disable_thinking").is_none(), "非标字段仍剥");
        assert_eq!(b["thinking"], json!({"type": "enabled"}), "false 不动思考参数");

        let mut b = json!({"model": "m"});
        apply_disable_thinking(&mut b, false, &Protocol::Anthropic, "https://example.com");
        assert_eq!(b, json!({"model": "m"}), "未请求禁用时整体不动");
    }

    /// 转换分支：`req_body` 由强类型 struct 序列化，body 里根本没有 `disable_thinking` key，
    /// 禁用意图只能靠调用方从客户端原体读出的 flag 传进来（审计发现的漏修路径）。
    #[test]
    fn converted_body_without_key_still_disables() {
        let mut b = json!({"model": "m", "messages": [], "reasoning_effort": "high"});
        apply_disable_thinking(&mut b, true, &Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(b["chat_template_kwargs"]["enable_thinking"], json!(false));
        assert!(b.get("reasoning_effort").is_none());
    }

    #[test]
    fn official_openai_host_matches_host_only() {
        assert!(is_official_openai_host("https://api.openai.com/v1/chat/completions"));
        assert!(is_official_openai_host("https://API.OpenAI.com:443/v1"));
        assert!(!is_official_openai_host("https://api.openai.com.evil.dev/v1"));
        assert!(!is_official_openai_host("https://open.bigmodel.cn/api/paas/v4"));
    }
}

#[cfg(test)]
mod test_strip_thinking {
    use super::{strip_redacted_thinking_blocks, strip_thinking_if_unmatched};
    use serde_json::json;

    #[test]
    fn strips_when_assistant_turn_lacks_thinking_block() {
        // 复现 0cea9d32 真因：thinking 开启 + assistant 轮仅 tool_use 无 thinking → 第三方 400
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "hi"}]},
                {"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "x", "input": {}}]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_none(), "应剔除 thinking");
        assert!(body.get("context_management").is_none(), "应剔除 context_management");
    }

    #[test]
    fn strips_context_management_with_adaptive_thinking_no_assistant_block() {
        // 复现 request_id=1658bb4b 真因：Claude Code adaptive/summarized 模式
        // (thinking adaptive + context_management clear_thinking_20251015) → assistant 轮不回传 thinking block
        // → 跨路由到第三方 anthropic 端点(DeepSeek)。单剔 thinking 不够，context_management 保留仍判 thinking mode → 400。
        // 修复：两字段皆剔。
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q1"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "a1"}]},
                {"role": "user", "content": [{"type": "text", "text": "q2"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "a2"}]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_none(), "应剔除 thinking");
        assert!(body.get("context_management").is_none(), "应剔除 context_management");
    }

    #[test]
    fn keeps_thinking_when_blocks_present() {
        let mut body = json!({
            "thinking": {"type": "adaptive"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "...", "signature": "s"},
                    {"type": "tool_use", "id": "t1", "name": "x", "input": {}},
                ]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_some(), "thinking 齐全应保留");
        // context_management 无条件剔（第三方不认该协商字段）
        assert!(body.get("context_management").is_none(), "thinking 开启即无条件剔 context_management（即使 thinking 齐全）");
    }

    #[test]
    fn noop_when_thinking_off() {
        let mut body = json!({
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [{"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "x", "input": {}}]}],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("messages").is_some());
        assert!(body.get("context_management").is_some(), "thinking off 不命中 unmatched，context_management 保留");
    }

    #[test]
    fn keeps_thinking_first_turn_no_assistant_but_strips_context_management() {
        // 复现 request_id=3a76c297 真因（GLM 1210）：首轮请求 messages 仅 user，无 assistant 历史，
        // thinking adaptive + context_management clear_thinking_20251015。
        // 旧逻辑 has_unmatched_assistant=false → 两字段皆保留 → GLM 不认 context_management 报 1210。
        // 修复：context_management 无条件剔（thinking_on 即剔，独立于 has_unmatched）；thinking 无 unmatched 故保留。
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_some(), "首轮无 assistant → has_unmatched=false，thinking 保留");
        assert!(body.get("context_management").is_none(), "thinking 开启即无条件剔 context_management（首轮 GLM 1210 根因）");
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_filters_only_redacted() {
        // 复现 trace 81dc4466（火山 doubao coding endpoint 400 InvalidParameter）
        // + 87e3c500（deepseek-v4-pro-260425 同根因）：redacted_thinking block 透传致第三方端点 400。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q"}]},
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "sig-data", "signature": "s"},
                    {"type": "redacted_thinking", "data": "encrypted-opaque-blob"},
                    {"type": "text", "text": "a"},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "t1", "content": "r"},
                    {"type": "redacted_thinking", "data": "encrypted-opaque-blob-2"},
                ]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        // assistant 轮：仅剩 thinking + text
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(asst.len(), 2, "应仅剥离 redacted_thinking，保留 thinking + text");
        assert!(
            asst.iter().all(|b| b.get("type").and_then(|t| t.as_str()) != Some("redacted_thinking")),
            "无残留 redacted_thinking"
        );
        // user 轮：仅剩 tool_result
        let u2 = msgs[2].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(u2.len(), 1, "tool_result 保留，仅剔 redacted_thinking");
        assert_eq!(u2[0].get("type").and_then(|t| t.as_str()), Some("tool_result"));
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_all_redacted_keeps_empty_message() {
        // 全 redacted_thinking 的 message → 剥离后 content 为空数组，但 message 结构保留
        // （剥离顺序敏感，禁删整条 message）。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q"}]},
                {"role": "assistant", "content": [
                    {"type": "redacted_thinking", "data": "blob-1"},
                    {"type": "redacted_thinking", "data": "blob-2"},
                ]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        assert_eq!(msgs.len(), 2, "message 数量不变（结构保留）");
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert!(asst.is_empty(), "全 redacted_thinking 剥离后 content 为空数组");
        // user 轮 text block 保留
        let u = msgs[0].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(u.len(), 1);
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_noop_on_string_content() {
        // 字符串形态 content 无 block 可剔，不动。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": "plain text"},
                {"role": "assistant", "content": [{"type": "redacted_thinking", "data": "x"}]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        // 字符串 content 原样
        assert_eq!(msgs[0].get("content").and_then(|c| c.as_str()), Some("plain text"));
        // assistant 数组内 redacted_thinking 已剔
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert!(asst.is_empty());
    }
}

#[cfg(test)]
mod test_hoist_mid_messages_system {
    use super::hoist_mid_messages_system;
    use serde_json::json;

    #[test]
    fn hoists_mid_system_to_top_level_when_multiturn() {
        // 复现 GLM 1210 真因（request_id=7c8629eadb074648a71858ae388ea550 等 9 例）：
        // CC 注入 role=system 进 messages 中段+末段，多轮 + 非流式下 GLM 拒绝 → 400 code 1210。
        // 规整：messages 内 role=system 合并到顶层 system 数组，messages 仅留 user/assistant。
        let mut body = json!({
            "system": [{"type": "text", "text": "base system"}],
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q1"}]},
                {"role": "system", "content": "mid reminder 1"},
                {"role": "assistant", "content": [{"type": "text", "text": "a1"}]},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": "mid reminder 2"},
                {"role": "assistant", "content": [{"type": "text", "text": "a2"}]},
                {"role": "user", "content": "q3"},
                {"role": "system", "content": "trailing reminder"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let msgs = body["messages"].as_array().unwrap();
        // messages 内不再有 role=system
        assert!(!msgs.iter().any(|m| m["role"] == "system"), "messages 内不应再有 role=system");
        // user/assistant 交替保留
        let roles: Vec<&str> = msgs.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["user", "assistant", "user", "assistant", "user"]);
        // 顶层 system 数组追加 3 个 text block（原 1 + 合并 3 = 4）
        let sys = body["system"].as_array().unwrap();
        assert_eq!(sys.len(), 4, "顶层 system 数组应含原 1 + 合并 3 = 4 块");
        assert_eq!(sys[0]["text"], "base system");
        assert_eq!(sys[1]["text"], "mid reminder 1");
        assert_eq!(sys[2]["text"], "mid reminder 2");
        assert_eq!(sys[3]["text"], "trailing reminder");
    }

    #[test]
    fn noop_when_no_assistant_first_turn() {
        // 首轮无 assistant 历史：messages 内 role=system 多为客户端首约定，第三方接受，不动。
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "system", "content": "ctx"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        // messages 保持原样（含 role=system）
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2, "首轮无 assistant 不应规整");
        assert_eq!(msgs[1]["role"], "system");
        assert_eq!(body["system"].as_array().unwrap().len(), 1, "顶层 system 不变");
    }

    #[test]
    fn noop_when_no_mid_system() {
        // 多轮但 messages 内无 role=system：无需规整
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        assert_eq!(body["messages"].as_array().unwrap().len(), 3, "无 mid-system 不动");
        assert_eq!(body["system"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn upgrades_top_system_str_to_array() {
        // 顶层 system 是字符串时：升级为数组并追加 mid-system
        let mut body = json!({
            "system": "base string",
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": "injected"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().expect("顶层 system 应升级为数组");
        assert_eq!(sys.len(), 2);
        assert_eq!(sys[0]["text"], "base string");
        assert_eq!(sys[1]["text"], "injected");
    }

    #[test]
    fn creates_top_system_when_absent() {
        // 顶层无 system 字段：mid-system 合并新建
        let mut body = json!({
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": [{"type": "text", "text": "block1"}]},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().expect("应新建顶层 system 数组");
        assert_eq!(sys.len(), 1);
        assert_eq!(sys[0]["text"], "block1");
    }

    #[test]
    fn preserves_array_block_content_from_mid_system() {
        // mid-system content 是数组（blocks）时：逐项合并到顶层 system 数组
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": [
                    {"type": "text", "text": "block a"},
                    {"type": "text", "text": "block b"},
                ]},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().unwrap();
        assert_eq!(sys.len(), 3, "原 1 + mid-system 2 block = 3");
        assert_eq!(sys[1]["text"], "block a");
        assert_eq!(sys[2]["text"], "block b");
    }
}
