# Rust↔TS 边界契约一致性审计（deep-audit-optimize / boundary 维度）

> 审计范围：Tauri command 三层契约对齐（Rust `#[tauri::command]` 签名 ↔ TS `invoke` cmd 字符串 + args ↔ 返回 struct serde ↔ 前端 TS 类型断言）。
> 模式：只读，不改码。严重度按「能否致运行时失败」分 high/medium/low。
> 结论先行：**本维度整体健康度极高**。174 个 TS invoke cmd 字符串全部命中 Rust fn（零拼写错/零幽灵 cmd）；Protocol 枚举 67 变体与 TS union 100% 对齐；所有顶层参数名经 Tauri v2 camelCase↔snake_case 自动转换对齐；camelCase serde struct（mcp / defaults_sync）与 TS 完全一致。仅 5 条 low 级类型不严，均无运行时炸点。

## 正向结论（已逐点核对）

- **cmd 字符串对齐**：174 个 TS invoke cmd 串 ⊆ 184 个 Rust `#[tauri::command]` fn 名；零 typo / 零已删 cmd 残留（`comm -23` 空集）。核对方法：`grep` 提取双侧 + `comm` 集合差。
- **Protocol 枚举对齐**：Rust `Protocol`（`src-tauri/crates/aidog_core/src/gateway/models/protocol.rs:7-158`）67 变体 serde rename ⊿ TS `Protocol` union（`src/services/api/types/part1.ts:10-32`）67 字面量 = 空。前端 PROTOCOLS 数组已改 JSON 运行时派生（`buildProtocolsFromPresets`），无静态数组漂移风险。
- **顶层参数对齐**：全量脚本化核对（TS invoke args camelCase key ↔ Rust fn snake_case param），零 missing。Tauri v2 内建 `rename_args = camelCase` 生效（CLAUDE.md `mitm.rs:370` 注释亦印证「无全局 rename_all」即靠 Tauri 默认转换）。
- **camelCase serde struct 对齐**：`McpServerInfo`/`McpScanItem`/`McpImportPayload`/`McpUpdatePayload`/`ImportReport`（`mcp/types.rs:126-227`）⊥ TS `mcp.ts` + `types/part3.ts:255-317` 全字段 camelCase 一致；`DefaultsSyncResult`/`ClientTypesSyncResult`（`defaults_sync.rs:39` / `client_types_sync.rs:40`）⊥ TS `platforms.ts:386-421` 一致。
- **核心返回 struct 对齐**：`Platform` / `PlatformModels` / `PlatformEndpoint` / `Group`（主体）/ `ProxyLogSummary` / `ProxyLogFilter` / `ModelTestRequest`/`Result` / `PlatformQuota` / `BalanceInfo` / `CodingPlanInfo` / `StatsQuery`/`StatsResult` / `SkillInfo` / `CachedSkills` / `CliToolStatus` / `CliConflict` / `CliInstallation` / `AboutInfo` / `MitmStatus` / `WhitelistEntryDto` / `MatchedRuleDto` 均逐字段核对一致。

## 发现清单

### F1: ProxyLogDetail（TS）缺 blocked_by / blocked_reason 字段
- 严重度: low（当前无运行时炸点；潜在类型不完整）
- Rust 侧: `src-tauri/crates/commands_proxy/src/proxy_log.rs:49` `proxy_log_get(id: String, ...) -> Result<Option<ProxyLog>, String>`，返回**完整** `ProxyLog`；该 struct 含 `blocked_by: String`（`proxy_log.rs:67`）+ `blocked_reason: String`（`proxy_log.rs:70`），均 `#[serde(default)]`。
- TS 侧: `src/services/api/types/part2.ts:40-73` `ProxyLogDetail` 接口**未声明** `blocked_by` / `blocked_reason`；消费处 `src/pages/Logs/useLogsData.ts:31` `useState<ProxyLogDetail | null>`。
- 不一致点: Rust 序列化的 JSON 实际带 `blocked_by` / `blocked_reason` 两键，TS 类型断言截断；前端 `grep -rn 'blocked_by' src/`（排除 types）= 0 命中，即当前无消费点，故 latent。
- 修复方向: 前端补类型（向后兼容，禁改后端）：`ProxyLogDetail` 加 `blocked_by?: string; blocked_reason?: string;`。未来 Logs 详情页要展示「被中间件拦截」徽标时可直接消费，无需再查类型缺口。
- 跨维度: 无

### F2: all_platform_usage_stats 返回 HashMap<u64,…> 序列化键为字符串，TS 断言 Record<number,…>
- 严重度: low（JS 数字键 coerce 到字符串，运行时巧合可用；类型不精确）
- Rust 侧: `src-tauri/crates/commands_proxy/src/proxy_log.rs:91` `all_platform_usage_stats(...) -> Result<HashMap<u64, PlatformUsageStats>, String>`。serde 序列化 `HashMap<u64,_>` 时键按 JSON 规范 stringify（`{"1":{...},"2":{...}}`）。
- TS 侧: `src/services/api/platforms.ts:326` `invoke<Record<number, PlatformUsageStats>>("all_platform_usage_stats")`；消费处 `src/pages/platforms/usePlatformsState.ts:184` `useState<Record<number, PlatformUsageStats>>`，`PlatformListView.tsx:143` `usageMap[p.id]`（`p.id: number`）。
- 不一致点: 实际 JSON 键是字符串 `"1"`，TS 类型声明数字键。JS 属性访问 `obj[1]` 自动 coerce 成 `obj["1"]`，故**运行时命中**；但 `Object.keys(usageMap)` 返字符串数组，TS 却认为 `number[]`，任何 `Number()` / 算术消费会出类型谎言。
- 修复方向: 前端对齐 wire 真相（向后兼容）：`usageStatsAll` 返回类型改 `Record<string, PlatformUsageStats>`，消费处 `usageMap[String(p.id)]` 或 `Number(k)`。对照 `all_group_usage_stats`（`groups.ts:12`）已正确用 `Record<string,…>`——同模式两处不一致即是佐证。
- 跨维度: 无

### F3: Group（TS）缺 sort_order 字段
- 严重度: low（当前无消费点；潜在排序功能类型缺口）
- Rust 侧: `src-tauri/crates/aidog_core/src/gateway/models/group.rs:31` `sort_order: i64`（`#[serde(default)]`）。
- TS 侧: `src/services/api/types/part1.ts:227-253` `Group` 接口未列 `sort_order`；`grep -rn '\.sort_order' src/` = 0 命中。
- 不一致点: Rust 序列化带 `sort_order`，TS 截断；前端如要做分组拖拽排序展示（平台已有 `group_reorder` / `platform.sort_order`），分组维度缺类型支撑。
- 修复方向: 前端补 `sort_order?: number`（与 `Platform.sort_order` 对齐，后者 TS 已有）。非阻塞。
- 跨维度: 无

### F4: SchedulingBreakerSettings.default_routing_mode Rust=String / TS=RoutingMode union
- 严重度: low（wire 串值落在 union 成员内，功能正常；类型松紧不一致）
- Rust 侧: `src-tauri/crates/aidog_core/src/gateway/models/settings.rs:142` `default_routing_mode: String`（serde 透传任意字符串，未知值由 `RoutingMode::from_str_or_default` 回退 `load_balance`）。
- TS 侧: `src/services/api/types/part2.ts:229` `default_routing_mode: RoutingMode`（5 值 union）。
- 不一致点: Rust 真值源是裸 String（DB 可存历史脏值），TS 断言为闭合 union。若 DB 存了非 5 值的脏字符串，TS 类型不设防（消费方以为必属 union）。
- 修复方向: 后端读取时归一化（`from_str_or_default` 已有，确认 settings_get 路径调用即可），或前端类型放宽为 `string`（向后兼容，禁删字段）。当前不致炸（前端 match 缺省分支兜底），归档待验。
- 跨维度: 无

### F5: settingsApi.get / read_claude_code_settings 返回 Record<string, any>
- 严重度: low（边界类型逃逸；`any` 违反项目「无 any」硬规，但属 code-quality 维度，此处仅记边界松散）
- Rust 侧: `src-tauri/crates/commands_config/src/settings.rs` `settings_get(...) -> Result<Option<serde_json::Value>>`；`read_claude_code_settings() -> Result<serde_json::Value>`。
- TS 侧: `src/services/api/settings.ts:43` `invoke<Record<string, any> | null>("settings_get")`；`settings.ts:99` `invoke<Record<string, any>>("read_claude_code_settings")`。
- 不一致点: 返回是任意 JSON（`serde_json::Value`），前端用 `any` 接——绕过 TS 类型防护。项目 `Verification` 规则要求 `grep 'any' src/` = 0，这两处是已知泄漏点。
- 修复方向: 前端改 `Record<string, unknown>`（向后兼容，消费处再 narrow）。属类型收紧，禁改后端。
- 跨维度: 与 code-quality 维度（any 清理）协同

## 未覆盖

- **tray.ts / notification.ts / exchange.ts 内部分 command**：抽检 `popover_data`（`popover.rs:45` → `PopoverData`）、`group_usage_stats`、`export_to_file` 均对齐，未逐 command 全核。这 3 个文件共约 20 个 command，按「核心路径优先」原则未深入，风险面低（均为 settings KV 透传或简单 struct）。
- **adapter/converter 内部 serde**（`gemini.rs` 等 camelCase struct）：属上游协议转换层，不经 Tauri command 边界，非本维度范围。
- **import_export 子系统**（sub2api / ccswitch）：导入导出内部 serde，前端经 `platform_share_export`/`platform_share_parse`（`SharePlatform` 已核对）消费，未直传这些 struct，非边界。
- **handler 注册完整性**：未核 `startup.rs` `generate_handler!` 是否注册了全部 184 个 Rust command（若有 fn 未注册，invoke 会运行时报「command not found」——这是潜在 high，但需跑 dev 才能验，归为未覆盖）。
