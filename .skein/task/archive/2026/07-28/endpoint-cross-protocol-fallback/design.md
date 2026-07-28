# endpoint 跨协议回退 — 详细设计

## 现状（根因）

请求 `39534a37`：source=anthropic（path `/proxy/chat/compate` 无 `/v1/` 回退 anthropic）+ platform 244 商汤 SenseNova（endpoints 仅 `[openai, client_type=codex_tui]`）。

路由链：
1. `select_endpoint_for_protocol(eps, "anthropic")` → None（`endpoint.rs:130` 普通平台步骤 3 无 anthropic endpoint，步骤 4 仅 `openai_responses` 源才回退 openai）
2. UA `Python-urllib` 非 codex/claude-cli → UA 透传不救（`forward.rs:51`）
3. matched_ep=None → fallback `platform_type=sensenova` → `is_valid_wire_protocol` gate → 502 + `"invalid target protocol: SenseNova"`

**矛盾**：上轮 converter-reasoning-content task 已交付 5×5 互转矩阵（source→wire 请求 + wire→source 响应双向），但 endpoint 选择层未开回退路径，converter 能力被卡死。

## 架构决策：普通平台开跨协议回退

用户语义（明确）：
1. 识别入站协议
2. 选择平台
3. 平台有相同入站协议 → 直接使用（同协议直发，零转换）
4. 没有 → 协议转换，按目标协议请求，响应按入参协议返回

= 释放 converter 5×5 能力。endpoint 层只管「选哪个 endpoint」，converter 自动介入双向转换。

## 数据流

`forward.rs:44` `select_endpoint_for_protocol(&route.platform.endpoints, source_protocol)` 返回 endpoint 后，下游 `convert_request(chat_req, &wire_protocol, &platform_protocol)` + `convert_response(body, wire_protocol, client_protocol, model)` 自动按 wire/client 协议对转换。**endpoint 层改动即释放全链路，converter 无新开发。**

## 改动：select_endpoint_for_protocol 普通平台步骤 4 扩展

现状（`endpoint.rs:128-137`）：
```rust
} else {
    // 普通平台：步骤 3 同协议直发；步骤 4 openai_responses 回退 openai。
    endpoints
        .iter()
        .find(|ep| ep_proto(ep) == source_protocol)
        .or_else(|| {
            if source_protocol == "openai_responses" {
                endpoints.iter().find(|ep| ep_proto(ep) == "openai")
            } else {
                None
            }
        })
}
```

改为（步骤 4 泛化）：
```rust
} else {
    // 普通平台：步骤 3 同协议直发；步骤 4 跨协议回退（释放 converter 5×5）。
    // 优先 openai（最稳 converter 路径，平台最常见），若无 openai 取 endpoints 首个非 source 可用 endpoint。
    endpoints
        .iter()
        .find(|ep| ep_proto(ep) == source_protocol)
        .or_else(|| endpoints.iter().find(|ep| ep_proto(ep) == "openai"))
        .or_else(|| endpoints.iter().find(|ep| ep_proto(ep) != source_protocol))
}
```

**优先级链**：同协议直发 > openai（converter 主路径）> 任意非 source endpoint（兜底）。

coding 平台分支（`has_coding_ep=true`）**不动** — 步骤 1/2 限定 coding 端点防 401，回退不介入。

## 不变量

- coding 平台（has_coding_ep）禁落非 coding 端点（401 防护，`endpoint.rs:65-70` 注释明说）— 回退仅在普通平台分支生效。
- 回退后 `target_protocol = ep.protocol`（endpoint 声明的真值），必落 5 wire 协议之一 → `is_valid_wire_protocol` gate 不再触发。
- UA 透传分支（`forward.rs:48-68`）保留 — 它是「path 不支持但 UA 明确」的精确路由，回退是更后置的兜底，两层不冲突。UA 命中优先于回退（UA 透传在 select 之后、fallback 之前）。

## 取舍

- **openai 优先非全协议平铺**：99% 平台 openai 为主，openai converter 路径最稳（上轮 test 覆盖最全）。全协议平铺优先级无数据支撑（platform endpoint 协议分布无统计），YAGNI。
- **不引入「回退开关」配置项**：用户语义明确「没有则转换」，平台配 endpoint 即意图声明。加开关 = 过度配置（YAGNI）。
- **converter 不改**：5×5 已覆盖，endpoint 层改动即释放。若 converter 某路径有 bug（如 gemini→anthropic 丢字段），属 converter 自身问题，本 task 不裹挟。

## 可能性分支（研究期留痕，不进正文/subtask）

- 若未来要支持「平台级禁跨协议转换」（用户想限制某平台仅同协议）：per-platform `extra.no_cross_protocol: bool`，true 时 select 无同协议 endpoint 直接 None（回退现状 fail）。触发条件：用户反馈强制转换丢字段且想保留协议隔离。
- 若 converter 某 wire→client 路径质量不足（如 gemini→openai_completions legacy 格式）：单独 converter task 补，不阻塞本 task 回退链路。
