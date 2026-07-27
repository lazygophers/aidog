# 架构深化第二轮: 10 组摩擦点全量修复 — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json):

## 0. 证据表 (4 路 Explore 产出, 全部带 file:line)

| # | 摩擦点 | 证据 | deletion test | 归组 |
|---|---|---|---|---|
| E1-1 | 协议身份两套表示: 客户端侧 `&str` / 上游侧 `Protocol` enum | `adapter/converter/request.rs:12,72`, `response.rs:49-70`, `proxy/forward.rs:169-172`, `proxy/finish.rs:136` | 浓缩 | c4 |
| E1-2 | **真 bug** SSE 分帧知识割裂: `finish.rs:286-287` 硬编码 `data: ` + `[DONE]`; `adapter/gemini.rs:429` `to_gemini_sse` 不带前缀; 全 crate grep `alt=sse` 零命中 (`converter/request.rs:22,62`) → Gemini 流式跨协议转换永远落空 | 同左 | 浓缩 | c5 |
| E1-3 | 4 个 `#[allow(dead_code)]` 纯委托 SSE 函数, 无生产调用点, 注释断言错误 | `adapter/openai_responses.rs:328-338`, `adapter/openai_completions.rs:196-206` | 浓缩(弱) | c7 |
| E1-4 | `same_protocol_passthrough` 布尔穿 4 处远距离消费点, `finish_nonstream`/`finish_stream` 各 15+ 参数 | `forward.rs:169-172,288,551,607` → `finish.rs:17,167,187` | 浓缩(需连 path 一起收) | c4 |
| E2-1 | **真 bug** `resolve_effective_models_cached`(生产) 与 `resolve_effective_models`(dead) 逐字节相同, 唯一测试打在死分支 | `router/candidates.rs:555,596`, `test_candidates.rs:740` | 浓缩 | c7 |
| E2-2 | `Platform.extra: String` 无类型 JSON, 12+ ad-hoc 解析器 | `peak_hours.rs:85,255`, `platform.rs:217,231`, `candidates.rs:47,64`, `quota/newapi.rs:32`, `quota/devin.rs:44`, `time_models.rs:14`, `http_client.rs:72`, `adapter/mock/config.rs:48`, `proxy/devin.rs:131`, `db/ui_extra.rs:15` | 浓缩 | c2 |
| E2-2b | `serde_json::to_string(&platform_type).trim_matches('"')` 裸名 hack ×4 | `router/mod.rs:83`, `candidates.rs:565,606`, `proxy/log.rs:147` | 浓缩 | c2 |
| E2-2c | `candidates.rs:47` "简化版"注释自陈猜协议名, 与 `models/platform.rs` 两套 platform_type 口径 | 同左 | 浓缩 | c2 |
| E2-3 | `calc_est_cost`(8 参) 住在"今日统计"模块; `platform_peak_hours` 与 `peak_hours.rs:75 peak_hours_for` 同一回退链两份实现 (注释自认重复); 计费核心零测试 | `db/stats_today.rs:203,245`, `gateway/peak_hours.rs:75` | 浓缩 | c6 |
| E2-4 | `router/selection.rs` 整 125 行死模块 + 136 行陪葬测试; 用旧 `platform.enabled` 布尔而非现行 `PlatformStatus` 三态, 随机种子不可测 | `router/selection.rs`, `router/mod.rs:21` | 浓缩, 净删 ~265 行 | c7 |
| E2-5 | (排除) `quota/mod.rs:203-250` `base_url.contains()` provider 分发 | — | **不通过** (只挪走) | 范围外 |
| E3-1 | **真 bug** 6 对 Rust↔TS 字段集漂移: `ProxyTimeoutSettings` 多 `source_protocol` 且每次保存都发被静默丢弃 / `Platform.sort_order` 缺 / `Group.sort_order` 缺 / `GroupPlatform` 缺 3 时间戳 | `models/settings.rs:54-61` ↔ `types/part2.ts:91-95` + `useSystemSettings.ts:233`; `models/platform.rs:186` ↔ `types/part1.ts:175-219`; `models/group.rs:31,111-125` ↔ `types/part1.ts:242,303` | 浓缩 | c1 / c1b |
| E3-2 | 双层零深度 adapter: TS 203 invoke 中 196 (96.5%) 1 行透传; Rust 194 command 中 87 (44.8%) body ≤2 行 | `startup.rs:41-286` (246 行注册表) | TS 层删=只挪走(不动); **Rust 转发层删=浓缩** | c3 |
| E3-3 | tracing 样板 191 份手写, `command = "xxx"` 字面量重复 191 次, `tracing::error!` 仅覆盖 49/194 (25%) | `commands_platform/src/platform.rs:31-38`, `commands_proxy/src/proxy_timeout.rs:19-28` | 浓缩 (attribute macro 单点生成) | c3 |
| E3-5 | **真 bug** `Vec<serde_json::Value>` 无类型 seam: TS 手拼 `Record<string,unknown>` 冒充 `Platform`, 写 `sort_order: 0`(接口无此字段) + `manual_budgets: ""`(Rust 是 `Vec<ManualBudget>`, 空串对 Vec 是硬错) | `backup.rs:173-187,213-227`, `ccswitchMatch.ts:242-270,269` | 浓缩 | c1 |
| E3-6 | `.github/workflows/` 无任何 CI 跑测试, release.yml 唯一检查是 `sync-version.mjs --check` | — | — | c1 (比对脚本填补) |
| E4-1 | `CliProxy.tsx` 936 行唯一未拆 god page: 20 个 useState, 批量操作/选中集/3 modal 全内联, `quotaTypeOf()` 重复 JSON.parse | `CliProxy.tsx:60,171-199` | 浓缩 | c10 |
| E4-2 | state-bag hook ×4: `useLogsData.ts:233-247` (40+ 字段全量透出) / `useSkillsData.ts:493-525` (50+) / `useSystemSettings.ts:254-279` (40+) / `useMcpData.ts`; 对比已 reducer 化的 `usePlatformsState.test.ts` (199 行, 同类唯一有测试者) 反证 bag 不可测 | 同左 + `ListView.tsx:25`, `DetailPanel.tsx:26` | 浓缩 (按关注点切) | c8 |
| E4-3 | `Settings.tsx` `Record<string, any>` 配置总线: `materializeStatuslineFields()` 混纯推导+副作用+吞错; `applyImport` 内联手写 dot-path 深合并; `updateField` 用重建对象实现删除语义 | `Settings.tsx:51-116,136,196-208,268-300+` | 浓缩 (`applyImport` 抽纯函数) | c8 |
| E4-4 | statusline 双份推导: `statusline-gen.ts:250-285 materializeStatusline()` (注释自陈 "mirrors") vs `useStatusLinePanel.ts:34,42,94-95` 独立实现; "byte-for-byte 一致"仅注释级契约 | 同左 | 浓缩 | c9 |
| E4-5 | `Groups.tsx`/`PlatformListView.tsx` ref 回调总线 (`openCreateGroupRef`/`reloadRef` 反向命令通道) | 同左 | 成本非平凡 → 仅摘 `editReducer.ts` 补测试 | c8(小项) |

## 1. 各组设计

### c1 类型契约止血
- 手修 6 对漂移。`sort_order`/时间戳方向定为 **TS 补字段**（Rust 是真值源）。
- `source_protocol` 方向 **2026-07-27 修订**：原定"Rust 补字段"，理由是"前端已在用"。exec 期查证推翻 —— `ProxyTimeoutSettings` 是系统级全局单例（`gateway/proxy/timeout.rs::get_system_timeout` / `resolve_timeout` 整体消费，无 per-protocol 分支），`useSystemSettings.ts:233` 硬编码传 `"anthropic"` 且从不随状态变化，Rust 补字段 = 补死字段。**改定为删前端字段**（零行为变化的死码清理），用户已拍板。按 protocol 覆盖超时若要做，另立 feature task。
- `scripts/check-types.mjs`：解析 `src-tauri/**/models/*.rs` 的 `#[derive(Serialize)]` struct 字段集 ↔ `src/services/api/types/*.ts` interface 字段集，报差集。接 `package.json` 的 `check:types`。
- **c1 是 c1b 的止血前置**：c1b 落地后 check-types.mjs 退化为冗余，届时由 c1b 删除（在 c1b 验收里显式声明）。

### c1b ts-rs codegen
- `ts-rs` 加到 `aidog_core` 的 dev/正常依赖，`#[derive(TS)] #[ts(export, export_to = "...")]`。
- **只对前端实际消费的 struct 加**（非 183 个全量），由 `types/part1.ts`/`part2.ts` 现有 interface 清单反查。
- 生成物落 `src/services/api/types/generated/`，`index.ts` re-export；手写 part*.ts 删净。
- 约束：`#[ts(rename_all)]` 禁 camelCase（spec `cross-layer/trellis-20.md`）。

### c2 PlatformExtra struct
```rust
#[derive(Serialize, Deserialize, Default)]
pub struct PlatformExtra {
    pub peak_hours: Option<Vec<PeakWindow>>,
    pub time_models: Option<...>,
    pub disable_during_peak: Option<bool>,
    pub quota: Option<QuotaExtra>,
    pub http: Option<HttpExtra>,
    pub mock: Option<MockConfig>,
    pub devin: Option<DevinExtra>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, Value>,   // 保 _ui_* unknown key 往返
}
```
- `Platform::extra_parsed(&self) -> PlatformExtra` 单点解析（带 `OnceCell` 或直接 parse — 现状每处都在 parse，不会更慢）。
- 4 份 `to_string(&platform_type).trim_matches('"')` → `Protocol::as_str()` 单点。
- 硬约束：`db/test_ui_extra.rs` 必须原样通过（unknown-key 往返）。

### c3 删 commands_* 转发层
- `#[tauri::command]` 直接标在 `aidog_core` 的实现函数上，6 个 `commands_*` crate 从 workspace 移除。
- `aidog_core/Cargo.toml`: `tauri = { optional = true }` + `[features] tauri = ["dep:tauri"]`，command fn 上 `#[cfg(feature = "tauri")]`。
- tracing 样板 → `#[traced_command]` proc-macro（新建 `aidog_macros` crate）或 `macro_rules!` 包装；`command = ` 名由 `stringify!(fn名)` 生成，错误分支统一 `tracing::error!`。
- **风险最高的一组**：194 个 command 迁移，`startup.rs` 注册表全改。**必须单独串行，禁与其他 subtask 并行**（其他组都会改 aidog_core）。

### c4 Protocol enum 统一
- `source_protocol: &str` → `Protocol`；新增 `Protocol::same_wire_family(&self, other) -> bool` 收 3 处同协议判定。
- `same_protocol_passthrough: bool` + 分散 path 参数 → `PassthroughDecision` 小结构体，`finish_*` 参数从 15+ 降到个位数。

### c5 SSE 分帧收进 adapter
- 约定：**所有 `to_*_sse` 自带完整 wire 帧**（含 `data: ` 前缀与终止帧）。`to_gemini_sse` 补齐（Gemini SSE 也用 `data: ` 前缀，实测 `alt=sse` 返回格式）。
- `finish.rs:286-287` 的 `strip_prefix("data: ")` 解析循环删除，改为 adapter 侧 `parse_upstream_sse(protocol, chunk)`。
- 出站请求 URL 补 `alt=sse`（`converter/request.rs:22,62`）— 仅 gemini 且 stream=true。
- 新增 ≥2 个 gemini 流式端到端测试（anthropic→gemini、openai→gemini）。

### c6 计费 locality
- 新 `gateway/billing.rs`：`pub fn est_cost(input: EstCostInput) -> f64` 纯函数 + `pub async fn calc_est_cost(...)` 取数壳。
- `db/stats_today.rs::platform_peak_hours` 删除，回退链只留 `peak_hours.rs::peak_hours_for`。
- 纯函数 ≥3 测试：base、peak multiplier、cache_read 折扣。

### c7 死代码清扫
- 删 `router/selection.rs` + `router/test_selection.rs` + `router/mod.rs:21` 重导出。
- 删 `resolve_effective_models`（dead 孪生），`test_candidates.rs:740` 改指 `_cached`（同时把 `_cached` 后缀去掉，只剩一个函数）。
- 删 `openai_responses.rs:328-338` / `openai_completions.rs:196-206` 4 个别名 + 错误注释。
- **与 c5 冲突面**：都动 adapter 的 SSE 函数 → c7 依赖 c5 完成。

### c8 前端 state-bag
- `useLogsData` → `useLogsFilters` / `useLogsList` / `useLogsDetail` 三段，**只做 Logs（立范式）**，Skills/Mcp/Settings 的 bag 本轮不切（范围控制，范式立住后续可复制）。
- `Settings.tsx::applyImport` 的 dot-path 深合并 → `domains/settings/applySelectedPaths.ts` 纯函数 + 测试。
- `domains/groups/editReducer.ts` 补测试（E4-5 摘出的小项）。

### c9 statusline 单份推导
- `useStatusLinePanel` 删自带推导，改调 `materializeStatusline`。
- 2 个脚本生成 snapshot test 锁住输出。

### c10 CliProxy 拆分
- `useCliProxySelection`（选中集 + 7 个布尔碎片）抽出；3 个批量 modal 外迁 `components/cliproxy/`。
- `quotaTypeOf` → 纯函数 + memo，消除重复 JSON.parse。

## 2. DAG 与并行约束

```
c1 ──┬─> c1b
     │
c7 <── c5 ──> (独立)
c4 ──> (独立, 与 c5 同文件 finish.rs → 串行: c5 先, c4 后)
c2 ──> (独立)
c6 ──> (独立)
c3 ──> 必须最后, 且独占 (改全部 Rust crate 布局)

前端 c8 / c9 / c10 完全独立于 Rust 侧, 可与 Rust 组并行
```

排期（max_parallel=2，Rust 侧串行度高）：
1. c1 ‖ c9
2. c1b ‖ c8
3. c5 ‖ c10
4. c7 ‖ c4
5. c2 ‖ —
6. c6 ‖ —
7. c3 独占（最后）

## 3. 关键取舍

| 取舍 | 决定 | 理由 |
|---|---|---|
| c1 与 c1b 是否合并 | **分开** | c1 是止血（几小时），c1b 是根治（引入构建期 codegen，风险高）。分开后 c1b 失败也不回退已修的 6 对漂移 |
| c3 打破 crate 分层 | **接受**，feature gate 缓解 | 用户明确拍板。`cargo test -p aidog_core --no-default-features` 作为分层未彻底腐化的哨兵 |
| TS 侧 extra parser | **不动** | deletion test 通过但收益低于回归风险；c2 只收 Rust 侧 |
| `quota/mod.rs` contains() 分发 | **不动** | deletion test 不通过 |
| 4 个 state-bag hook | **只切 Logs** | 范式验证优先，避免一轮内前端改动面失控 |
| Gemini SSE 前缀格式 | 需实测确认 | c5 subtask 首要动作：查 Gemini `alt=sse` 实际 wire 格式，勿凭猜 |

## 4. 风险与缓解

| 风险 | 缓解 |
|---|---|
| c3 迁移 194 command 出错，编译地狱 | 独占执行；分批（按原 crate 6 批）迁移，每批 `cargo build` 通过再下一批 |
| c1b ts-rs 生成物与手写差异导致 tsc 大面积红 | 先生成到 `generated/` 并行存在，逐文件切换 import，最后删手写 |
| c2 `#[serde(flatten)]` 与 `Option` 字段组合的已知 serde 坑（flatten 下数字反序列化走 `Value`） | 依赖 `db/test_ui_extra.rs` 往返测试 + 新增 peak_hours 往返测试 |
| c5 改 wire 格式引入线上流式回归 | 新增端到端测试为验收硬条件；同协议 passthrough 路径不得受影响（回归测试锁） |
| 多组同改 `aidog_core`，auto_commit 下冲突 | max_parallel=2 且 DAG 已按文件冲突面排序；c3 独占 |
