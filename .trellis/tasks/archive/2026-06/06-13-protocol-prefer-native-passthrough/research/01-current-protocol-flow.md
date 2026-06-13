# Research: 当前协议判定与出站协议决策完整链路

- **Query**: 入站协议如何判定？选定平台后如何决定用哪个协议发出？是否无条件转成平台默认协议？
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### 完整链路 (file:line)

入站请求进入 `proxy.rs` 主转发函数后：

1. **入站协议判定 — 基于请求路径 (不读 body)**
   - `proxy.rs:578` `let source_protocol = detect_source_protocol(&path);`
   - `detect_source_protocol` 实现 `proxy.rs:1487-1516`：
     - `/v1/messages` → `"anthropic"`
     - `/v1/responses` → `"openai_responses"` (Codex)
     - `/v1/chat/completions` `/v1/completions` `/v1/embeddings` `/v1/images` `/v1/audio` `/v1/models` → `"openai"`
     - `/v1beta/` → `"gemini"`
     - 兜底 → `"anthropic"`
   - 注释 `proxy.rs:577`：「group no longer restricts inbound protocol」— 分组不再限制入站协议，纯按 path 自动识别。

2. **按入站协议解析 body 为内部 ChatRequest**
   - `proxy.rs:594` `adapter::parse_incoming_request(&log.source_protocol, &req_value)`
   - `converter.rs:60-69`：按 `source_protocol` 字符串分派到 `from_openai` / `from_responses` / `from_completions` / `from_gemini`，默认 (anthropic) 直接 `serde_json::from_value`。
   - 结论：入站请求被**统一归一化为内部 `ChatRequest`**，丢失原始 wire 细节（除 ClaudeCode 透传外，下文 03）。

3. **路由选平台 (不决定协议)**
   - `proxy.rs:610` `select_platform(&state.db, &group, &chat_req.model)`
   - `router.rs:13-80`：`select_platform` 只产出 `RouteResult { platform, target_model, mapping }`。**router.rs 全程不涉及任何协议判定/选择**——协议决策完全在 proxy.rs。

4. **出站协议决策 — 端点匹配 (关键!)**
   - `proxy.rs:628-641`：
     ```rust
     let ep_proto = |ep: &PlatformEndpoint| format!("{:?}", ep.protocol).to_lowercase();
     let matched_ep = route.platform.endpoints
         .iter()
         .find(|ep| ep_proto(ep) == source_protocol)        // ① 精确匹配入站协议
         .or_else(|| {
             if source_protocol == "openai_responses" {       // ② Codex 特例回退
                 route.platform.endpoints.iter().find(|ep| ep_proto(ep) == "openai")
             } else { None }
         });
     let (target_protocol_enum, target_base_url, client_type, coding_plan) = matched_ep
         .map(|ep| (&ep.protocol, ep.base_url.clone(), ep.client_type.clone(), ep.coding_plan))
         .unwrap_or((&route.platform.platform_type, route.platform.base_url.clone(),
                     ClientType::Default, false));            // ③ 无匹配端点 → 回退平台主协议
     ```
   - **`target_protocol_enum` = wire protocol = 实际出站协议**。
   - **结论：不是无条件转成平台默认协议**。已有「按 source_protocol 在 platform.endpoints 里找同协议端点」逻辑——若平台声明了与入站协议相同的端点，就用该端点的协议/base_url 出站（即近似透传该协议）。

5. **协议转换执行**
   - `proxy.rs:740-742`：
     ```rust
     let platform_protocol = &route.platform.platform_type;
     let (mut req_body, mut api_path) = adapter::convert_request(&chat_req, target_protocol_enum, platform_protocol);
     ```
   - `convert_request` 第二参 = `wire_protocol`(出站请求体格式)，第三参 = `platform_protocol`(决定 OpenAI 兼容平台 path)。详见 03。

6. **URL 构造**
   - `proxy.rs:753-754` `base_url(端点或平台).trim_end_matches('/') + api_path`。

### 当前是否「无条件转成平台默认协议」？

**否，但有重要 caveat（见下）**。已有端点匹配机制：入站协议 → 优先用平台同协议端点出站。但：
- 端点匹配成功时，`target_protocol_enum` = 端点协议（= 入站协议），`convert_request` 仍会跑 `to_anthropic/to_openai/to_gemini` 把内部 `ChatRequest` **重新序列化**为该协议 —— 即使入站协议与出站协议相同，**仍走一次 归一化→重序列化 的有损往返**，不是字节级透传（唯一字节级透传是 ClaudeCode，见 03）。
- 端点**匹配失败**（平台没声明对应协议端点）→ 回退 `platform_type` + `ClientType::Default`，按平台主协议转换出站。这才是「转成平台默认协议」的发生点。

## Caveats / Not Found

- 端点匹配是**精确字符串相等**（`openai_responses` 仅对 `openai` 有一条回退），不存在「anthropic 入站可回退到 openai 端点」之类的跨协议兼容回退。
- wiki `.wiki/architecture/protocol-adapter.md` 的「入站 openai_chat → 出站 anthropic 自动协议转换」描述（line 47-53）**已过时/不完整**：它没提 endpoints 端点匹配优先逻辑，给人「无条件转平台主协议」的错觉。代码实际先做端点匹配。需复核更新该 wiki。
