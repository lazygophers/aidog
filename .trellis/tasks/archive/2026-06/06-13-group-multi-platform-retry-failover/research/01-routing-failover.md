# Research: 路由 / failover / 重试现状

- **Query**: router.rs 分组匹配 + 平台选择逻辑; proxy.rs 是否已有失败重试 / 换平台?
- **Scope**: internal
- **Date**: 2026-06-13

## 结论速览

- **单次请求只选 1 个平台**，`select_platform` 调用一次返回单一 `Platform`，无循环、无候选列表。
- **现状无任何失败重试 / 换平台逻辑**。上游 send 失败（502）或非 2xx 状态，proxy.rs 直接把错误透传回用户并 `return`，不会换平台重试。
- 现有 `RoutingMode::Failover` 是**选平台时的"优先级选择"**（按 priority 升序选第一个 enabled），不是"请求失败后切换平台"的运行时 failover。语义与新需求的"重试 failover"不同，**不能直接复用，但可作为候选平台排序的基础**。

## Findings

### 路由选择逻辑（router.rs）

| File:Line | 说明 |
|---|---|
| `router.rs:13-80` | `select_platform()` 主入口：①找 model_mapping ②取 group_platforms ③显式 mapping 优先 ④按 routing_mode 选平台 ⑤自动匹配模型。**返回单个 `RouteResult { platform, target_model, mapping }`** |
| `router.rs:55-58` | `match group.routing_mode { Failover => select_failover, LoadBalance => select_load_balance }` |
| `router.rs:111-120` | `select_failover`：按 `gp.priority` 升序排序，`.find(\|gp\| gp.platform.enabled)` 选第一个启用的。**只返回 1 个**，无候选列表 |
| `router.rs:123-149` | `select_load_balance`：在 enabled 平台中加权随机选 1 个 |

关键代码（`router.rs:110-120`）：
```rust
/// 故障转移：按 priority 升序选第一个 enabled 的
fn select_failover(platforms: &[GroupPlatformDetail]) -> Result<Platform, String> {
    let mut sorted: Vec<_> = platforms.iter().collect();
    sorted.sort_by_key(|gp| gp.priority);
    sorted
        .into_iter()
        .find(|gp| gp.platform.enabled)   // ← 只过 enabled bool
        .map(|gp| gp.platform.clone())
        .ok_or_else(|| "no enabled platform for failover".to_string())
}
```

### 请求转发流程（proxy.rs）

| File:Line | 说明 |
|---|---|
| `proxy.rs:644` | `let route = select_platform(&state.db, &group, &chat_req.model).await;` — **整个请求只调用一次** |
| `proxy.rs:645-655` | route 失败 → 400 直接返回 |
| `proxy.rs:842-854` | 上游 `send()` 失败 → 502，写日志后 **直接 return**，无换平台 |
| `proxy.rs:869-885` | 上游返回非 2xx（含 401/403）→ 原样 `status` + body 透传给用户，写日志后 **直接 return**。**无 401/403 特殊检测，无平台状态联动** |
| `proxy.rs:888-914` | 非流式 2xx → 透传 |
| `proxy.rs:917+` | `spawn_estimate` 后台预估，之后是流式分支（同样无重试） |

非成功状态处理（`proxy.rs:869-884`）—— 注意这里就是新需求 401/403 自动禁用 + 重试要插入的位置：
```rust
if !status.is_success() {
    let body = resp.text().await.unwrap_or_default();
    ...
    log.status_code = status.as_u16() as i32;
    ...
    upsert_log(&state, &log, &log_settings).await;
    return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY), body)
        .into_response();   // ← 直接返回, 无重试/换平台
}
```

### 单条请求路径中的多个提前 return（都无重试）

`proxy.rs` 在路由后还有若干提前返回分支，重试循环若包裹必须避开/兼容它们：
- `proxy.rs:709-746` manual_budget 耗尽 → 402 return
- `proxy.rs:749-764` Mock 平台 → `handle_mock` return
- `proxy.rs:767-782` ClaudeCode 透传 → `handle_passthrough` return

## Caveats / Not Found

- **未找到任何重试 / 候选轮询 / max_retries 配置**。grep 关键词 `failover/retry/load_balance/select_platform` 仅命中上述选择逻辑（router.rs/proxy.rs/models.rs/db.rs），确认现状无运行时 failover。
- 流式分支（`proxy.rs:920+` 之后）未逐行读，但其入口同样在单次 `route` 之后，结构上无第二次选平台调用 → 同样无重试（如需 100% 确认可补读 920-1100）。
