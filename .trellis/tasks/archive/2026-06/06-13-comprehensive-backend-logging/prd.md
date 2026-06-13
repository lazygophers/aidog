# PRD: 后端全链路日志覆盖（零遗漏）

## 背景

前序已补请求生命周期日志（router + proxy 主路径）。但后端仍有大面积盲区，排障时无迹可循：

| 文件 | tracing 点 | 盲区 |
|---|---|---|
| db.rs | 0 | 60 处 map_err + 10 处 `let _ =`/`.ok()` 静默吞错 |
| lib.rs | 17 | 69 个 tauri 命令大多无入口/错误日志；21 处 `let _ =` |
| price_sync.rs | 0 | 后台价格同步 4 处 Err 全静默 |
| codex.rs | 0 | TOML 读写失败无日志 |
| manual_budget.rs | 0 | 预算重置/耗尽判定无 debug |
| estimate.rs | 1 | calibrate 失败静默 early-return |
| quota.rs | 2(出站) | 错误路径 err_quota 不记平台/错误 |

目标：**系统性补齐所有盲区**，统一分级，全部输出终端。

## 统一分级策略（MUST 遵守，避免过度日志）

| 级别 | 用途 | 例 |
|---|---|---|
| TRACE | 高频内循环细节（默认不用） | — |
| DEBUG | 命令入口（命令名 + 脱敏关键参数）、请求/响应 body、DB 写操作影响行数 | `command=update_platform id=3` |
| INFO | 请求生命周期 path/路由/完成、后台任务启停、设置变更、server bind | `price sync started` |
| WARN | 可恢复异常、降级回退、非成功上游状态、配额耗尽、未命中、静默吞错点 | `db settings read failed, using default` |
| ERROR | 不可恢复失败、上游连接失败、DB 写失败致数据丢失、panic 边界 | `failed to persist platform: {e}` |

**脱敏硬规**：`api_key` / `authorization` / `token` / `Bearer` 值一律 `[REDACTED]`，沿用现有 redact 模式。日志中禁出现明文密钥。

**反过度日志**：不在 hot path 每次循环打日志；DB 读成功不打（只打写/失败）；命令入口 debug 级而非 info。

## 零遗漏清单（按文件，sub-agent 实施范围）

### 组 A — lib.rs（69 命令）
- 每个 `#[tauri::command]` 入口加 `tracing::debug!(command="<name>", <脱敏关键参数>, "command invoked")`
- 每个 `return Err(...)` / `Err(e) =>` 终止点加 `tracing::warn!` 或 `error!`（含命令名 + 错误）
- 审查 21 处 `let _ =`：丢弃 Result 若代表失败语义 → 加 warn；纯 fire-and-forget emit 可不加
- 已有 17 点不重复/不降级

### 组 B — db.rs
- 10 处 `let _ =` / `.ok()` 静默吞错 → 评估补 `tracing::warn!`（尤其 settings 写、cleanup、stats 更新）
- settings 读取失败回退默认值处 → `tracing::warn!`
- 60 处 map_err 本身上抛 String 给调用边界，**不在 db 层重复 log**（避免双重日志）；仅静默处补

### 组 C — proxy.rs + quota.rs
- proxy.rs：`resolve_group` 失败细节、`handle_mock` 错误路径、server 启动 `bind` 成功/失败、SSE chunk error 升级为 warn
- quota.rs：`err_quota` 辅助补 `tracing::warn!(platform, error)`；各配额函数失败路径

### 组 D — 后台/杂项（price_sync.rs + codex.rs + manual_budget.rs + estimate.rs + logging.rs）
- price_sync.rs：同步启停 info、4 处 Err → warn/error
- codex.rs：TOML 读/写/parse 失败 → warn/error
- manual_budget.rs：耗尽/重置判定 → debug（含 kind/unit/amount）
- estimate.rs：calibrate_from_quota 失败 early-return 处 → warn
- logging.rs：确认 console layer 永远输出（dev 强制 debug 已实现）；确认无 log 仅落文件不进终端

### 组 E — 追踪 id（trace-id，横切，4 组落地后单独 pass）
- **目标**: 每个代理请求生命周期内所有日志携带同一 `req_id`，可串联 inbound→route→upstream→completed 全链路。
- **机制**: tracing span，非手动每宏加字段。proxy.rs 入站处理器开头 `let req_id = uuid::Uuid::new_v4().simple()` 取前 8 位 hex；建 `tracing::info_span!("req", id = %req_id)`；对 async handler body `.instrument(span)`（跨 await 用 Instrument 而非 enter guard，避免 hold-across-await）。tracing_subscriber fmt 默认渲染当前 span 上下文 → 该请求所有 event 自动前缀 `req{id=xxxxxxxx}:`，无需改 logging.rs。
- **uuid 依赖已存在**（Cargo.toml `uuid = {version="1", features=["v4"]}`），零新依赖。
- **范围**: 主代理 handler + passthrough handler + mock handler。出站 quota/命令暂不强制（可后续）。
- **冲突约束**: 仅 proxy.rs；与组 C 同文件，必须组 C 完成后再做。

## 非目标
- 不改日志框架（继续 tracing + tracing_subscriber）
- 不加结构化 JSON 输出格式
- 不改前端

## 验收标准
1. `cargo check` 0 warning 0 error（warnings-are-issues）
2. 每文件 tracing 点数显著上升：db.rs > 8、price_sync > 4、codex > 4、lib.rs 覆盖全部 Err 终止点
3. `grep -rn "api_key\|Bearer " src/` 在新增日志行中无明文（只允许 [REDACTED]）
4. 手动跑一次代理请求 + 一次配额查询，终端可见完整链路无断点
5. 分级正确：body 在 debug、path/生命周期在 info、失败在 warn/error

## 失败处理
- 任一组 cargo check 失败 → 该组 agent 自修复后再交
- 双重日志（同一错误 db 层 + 命令层都打）→ 删 db 层保命令层
- 不确定某 `let _ =` 是否该记 → 默认记 warn（排障优先）
