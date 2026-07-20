# 深度审计发现汇总 — deep-audit-optimize

4 维度并行只读审计,共 **41 条发现**。详情见 `research/<dim>.md`。

## 严重度分布

| 维度 | high | medium | low | 小计 |
|---|---|---|---|---|
| 代码质量 | 2 | 8 | 4 | 14 |
| 性能 | 2 | 5 | 5 | 12 |
| 边界契约 | 0 | 0 | 5 | 5 |
| 架构 | 1 | 4 | 5 | 10 |

边界维度健康度极高(零 high/medium,174 cmd 字符串全对齐,Protocol 67 变体一致)。

## 背景发现(非审计目标,但影响全局)

仓库正处于 **workspace 拆分重构期**:Rust 已从 `src-tauri/src/gateway/` 迁入 `src-tauri/crates/{aidog_core,commands_*,proxy,tray,config,ai_tools,platform}/`。`CLAUDE.md` 项目结构章节路径全过期(lib.rs 73 commands 已拆多 crate;11 页面实为 18;services/api.ts 实为目录)。此为 [arch F1],建议首先修——它污染所有维度的 file:line 定位准确性。

---

## 分批(按可执行性 + 风险,非按维度)

### Batch A — 高收益快修(low effort / high value,全向后兼容)

| ID | 来源 | 位置 | 动作 |
|---|---|---|---|
| A1 | perf F1 | http_client.rs:33 / forward.rs:267 | 每请求重建 reqwest::Client → 按 use_proxy 维度缓存共享 client(复用 TLS/连接池) |
| A2 | perf F2 | log.rs:164 / app_setup.rs:395 | tray-refresh 每请求同步触发菜单重建 4-6 次 → 监听端 trailing 防抖或仅终态 emit |
| A3 | quality F2 | AppSettings/LogSettingsSection.tsx:33,36 | 清 `console.log` debug 残留 |
| A4 | quality F1 | 7 crate 179 处 | 清 `#[allow(unused_imports)]` 掩盖的死 import(workspace 拆分遗留),逐 crate `cargo check` 验 |
| A5 | arch F1 | CLAUDE.md | 更新「项目结构」章节对齐实际 crate 布局 + 页面数 + api.ts 目录 |
| A6 | arch F5 | 15 处 .toLocaleString() | 统一走 `formatters.ts::formatDateTime`(含 i18n locale) |
| A7 | quality F14 | 13 处 commands_*/src/*.rs | 去重文件头 import 模板 |

### Batch B — 重复抽取(机械重构,低风险)

| ID | 来源 | 位置 | 动作 |
|---|---|---|---|
| B1 | quality F6 | formSections.tsx:26,416 / ScheduledBackupSection.tsx:25 | `pad` 抽 `formatters.ts` |
| B2 | quality F7 | popover.tsx:76 / PlatformCard.tsx:790 / shared/usageColor.ts:56 | `clamp` 抽公共(签名各异,先归一) |
| B3 | quality F8 | catalog.rs:90,232 | ANSI_RE 正则 OnceLock 去重 |
| B4 | arch F4 | 22 处 createPortal | 抽 `components/shared/Modal.tsx` 基元(9 个 *Modal 收敛) |
| B5 | arch F8 | PlatformCard/Groups ≥3 处 | `$${formatCost(x)}` → `formatCostUsd` |
| B6 | quality F11 | PlatformCard.tsx:774 | `relativeTime` 合并入 `formatRelativeTime`(去英文硬编码) |

### Batch C — 复杂度拆分(高风险高收益,需 grill 强化)

| ID | 来源 | 位置 | 动作 |
|---|---|---|---|
| C1 | quality F5 | PlatformCard.tsx:77(914 行,~20 props) | 拆 god component,拆前先抽测试锚 |
| C2 | quality F9 | router/candidates.rs:46(232 行/19 分支) | `select_candidates_ctx` 拆分 + 表驱动 |
| C3 | quality F10 | commands_ai_tools/model_test.rs:18(272 行) | 线性长函数拆阶段 |
| C4 | perf F5+F7 | candidates.rs:88,257 / handler.rs:216,305 | 路由层 JSON 重复解析(2-3 遍/平台)→ 一次解析传递 |
| C5 | perf F4 | log.rs:39-55,102-118 | upsert_log 内 est_cost + get_platform 重复计算两次 → 合并 |
| C6 | perf F3 | headers.rs:445 / forward.rs:291 | 上游请求体 pretty 二次序列化 → 仅 log 开启时付 |

### Batch D — 类型/契约收紧(防御性,向后兼容)

| ID | 来源 | 位置 | 动作 |
|---|---|---|---|
| D1 | quality F3 | editors/* 10+ 处 | `updateField: (field: string, value: any)` → 泛型/字面量 union |
| D2 | quality F4 | src/ 32 处 | `catch (e: any)` → `unknown` + narrowing |
| D3 | quality F12 | src/ 26 处 | 审 eslint-disable exhaustive-deps 是否掩盖真实依赖 |
| D4 | quality F13 | proxy/log.rs / mitm/cert_signer.rs 5 处 | Mutex `.lock().unwrap()` → `map_err` 防 poison panic |
| D5 | boundary F1-F5 | api.ts 类型 | ProxyLogDetail 补 blocked_by/blocked_reason;all_platform_usage_stats 改 Record<string>;Group 补 sort_order;RoutingMode 归一;settings any→unknown |
| D6 | perf F6 | stream.rs:63-90 | 流式 usage 聚合每 chunk 全量 split → 增量解析 |

### Batch E — 架构定调(需讨论后定方向,非纯执行)

| ID | 来源 | 位置 | 动作 |
|---|---|---|---|
| E1 | arch F2+F9 | domains/ vs pages+components | feature-sliced 定调 + 文档化目录职责规则 |
| E2 | arch F3 | gateway/ 15 子目录 vs 16 扁平 .rs | 定义升级子目录的门槛 |
| E3 | arch F6 | services/api/types/part1-4.ts | 按领域切分(非按尺寸) |
| E4 | arch F7 | peak_hours / tray_render | Rust↔TS 跨层对称逻辑加契约测试防漂移 |
| E5 | arch F10 | popover.tsx / PopoverConfigTab / PopoverCards | 三层横跨整理 |

---

## 未覆盖项(各 research 文件末尾标注,非阻塞)

- code-quality:schema_late.rs(1121 行)/ ca.rs(1223 行)/ forward.rs(973 行)/ logging.rs(909 行)未深扫
- perf:connect.rs / mitm CA(P1 隧道非默认热路径)/ quota 外部查询 / 前端 bundle 体积(无 build-analyzer 脚本)
- boundary:tray.ts / notification.ts / exchange.ts ~20 command 抽检未深核;startup.rs generate_handler! 注册完整性(需跑 dev 验,潜在 high)
- architecture:navGuard 注册表边界冲突(归 code-quality);Rust crate 间依赖方向(建议画依赖图)

## 跨维度协同(挑批次时注意)

- C1(PlatformCard 拆)↔ perf F11(PlatformListView 拖拽重渲)↔ D1(editors updateField 类型)— 同一组件群,建议同批或定先后
- A6(formatters 统一)↔ B1/B2/B5/B6(工具抽取)— B 批依赖 A6 先把 formatters 收齐,或合并执行
- E4(契约测试)↔ D5(边界类型补)— E4 若做则 D5 自然覆盖部分
