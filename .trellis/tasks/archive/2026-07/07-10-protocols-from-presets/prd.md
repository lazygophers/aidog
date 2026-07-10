# PROTOCOLS/COLORS/LABELS 常量改由 presets JSON 派生

## Goal

消除 `src/domains/platforms/constants.ts` 三大硬编码常量（`PROTOCOLS` / `PROTOCOL_COLORS` / `PROTOCOL_LABELS`），全部改为从 `src-tauri/defaults/platform-presets.json` 派生。单一真值源 = presets JSON。同时拆 5 个 coding plan 独立协议 key（与 glm_coding 同模式），根治同 value 双显。

**为什么**：现 JSON 有 name/endpoints/is_coding_plan，TS 常量又抄一份 label/color/codingPlan/keywords → 双真值源，改一处忘另一处（protocol 展示 bug 反复）。用户多次报 glm_coding 显示问题根因即此。彻底统一到 JSON。

## 现状

### 三常量（constants.ts）
- `PROTOCOLS`（line 6，60+ 条）：value/label/codingPlan/keywords/hosts/codingKeyPrefixes。hosts 已运行时 injectProtocolHosts 派生注入（非硬编码），其余硬编码。
- `PROTOCOL_COLORS`（line 202，Record<Protocol,hex>）：每协议品牌色。
- `PROTOCOL_LABELS`（line 112，Record<Protocol,string>）：含 5 请求格式协议 + 61 platform 类型 label。与 `ENDPOINT_PROTOCOLS`（line 84，5 条请求格式）在 openai/anthropic/gemini 等重叠。
- `ENDPOINT_PROTOCOLS`（line 84，5 条 openai/anthropic/gemini/responses/completions）：**保留**（请求格式协议层，非 platform，不在 presets JSON）。

### 调用点
- PROTOCOLS: SearchableProtocolSelect / ccswitchMatch / Sub2ApiImport / PlatformEditForm / platformPaste.test / defaults.ts(injectProtocolHosts)
- PROTOCOL_COLORS: ProtocolLogo / PlatformCard / PlatformListView / PlatformEditForm
- PROTOCOL_LABELS: PlatformPicker / SearchableProtocolSelect / SmartPasteModal / PlatformCard / ModelTestPanel / usePlatformForm / MultiKeyPreview / PlatformListView / PlatformEditForm

### coding plan 双显
PROTOCOLS 6 个 codingPlan 条目：glm_coding（JSON 独立 key is_coding_plan=true）+ 5 共 value 双显（kimi/minimax/minimax_en/qianfan/xiaomi_mimo，JSON 无 cp 信息）。用户决定：5 协议拆独立 cp key（kimi_coding 等），与 glm_coding 同模式。

## Decision (ADR-lite)

**Context**：三常量与 JSON 双真值源致 protocol 展示 bug 反复。用户要彻底统一。

**Decision**：
1. **JSON schema 扩展**（每协议顶层加 3 字段）：
   - `keywords: [string]`（搜索词，从 PROTOCOLS 迁）
   - `codingKeyPrefixes: [string]`（仅 xiaomi_mimo 系 `tp-`，余 absent）
   - `color: "#hex"`（从 PROTOCOL_COLORS 迁）
   - name/is_coding_plan/endpoints 已有（label/codingPlan/hosts 源）
2. **JSON 加 5 独立 cp key**：`kimi_coding` / `minimax_coding` / `minimax_en_coding` / `qianfan_coding` / `xiaomi_mimo_coding`，每条完整（endpoints/models/name/desc/color/keywords/source_urls），与 glm_coding 同模式。原 5 协议（kimi 等）去掉双显 codingPlan 条目（JSON 本就单条，PROTOCOLS 派生时单条）。
3. **Rust Protocol 枚举加 5 变体** + serde rename_all + 路由/adapter/quota/estimate 支持（与 glm_coding 同路径）。
4. **删三常量**：PROTOCOLS / PROTOCOL_COLORS / PROTOCOL_LABELS。ENDPOINT_PROTOCOLS 保留。
5. **派生层**（defaults.ts，全 async）：
   - `buildProtocolsFromPresets(locale): Promise<ProtocolOption[]>`：loadDoc → 每 key 派生 {value, label:name[locale]||en-US, codingPlan:is_coding_plan||false, keywords, hosts:派生自endpoints, codingKeyPrefixes}
   - `getProtocolColorMap(): Promise<Record<Protocol,hex>>`：loadDoc → 每 key color 字段
   - labelMap（getProtocolLabelMap）已存在，覆盖 PROTOCOL_LABELS 删除后的 fallback
6. **调用点全 async 化**：现同步读常量 → useState + useEffect（locale key）加载派生 map。SearchableProtocolSelect/PlatformCard/PlatformEditForm/PlatformListView/ProtocolLogo/PlatformPicker/SmartPasteModal/ModelTestPanel/usePlatformForm/MultiKeyPreview/Sub2ApiImport/ccswitchMatch 全改。

**Consequences**：
- JSON 61 → 66 协议（+5 cp key），每条 +3 字段（keywords/color/codingKeyPrefixes）。
- Rust Protocol 枚举 +5 变体，serde/路由/adapter/quota 全链加支。
- 前端 ~13 文件改 async 加载（首帧 fallback 空/placeholder，加载后填充——与现 labelMap useEffect 同模式）。
- ENDPOINT_PROTOCOLS（5 请求格式协议）保留硬编码：非 platform 数据，概念独立（endpoint.protocol 字段值，非 platform_type）。
- platformPaste.test 需更新（PROTOCOLS async 化后测试 mock 调整）。

## Requirements

### R1 JSON schema 扩展
- R1.1 每协议顶层加 `keywords`（数组，搜索词）、`color`（hex 字符串）、`codingKeyPrefixes`（数组，可选，仅 xiaomi_mimo 系）。
- R1.2 字段从现 PROTOCOLS/PROTOCOL_COLORS 逐协议迁移（61 条），禁遗漏。
- R1.3 json.load 等价硬门禁（与 presets-compress 一致）。

### R2 5 独立 cp key
- R2.1 加 `kimi_coding`/`minimax_coding`/`minimax_en_coding`/`qianfan_coding`/`xiaomi_mimo_coding`，各含 is_coding_plan:true + 完整 endpoints（cp 真实 URL）/models（cp 独占模型）/name/desc/color/keywords/source_urls。
- R2.2 cp key endpoints 需查各平台官方 cp 文档填真实 base_url（与 glm_coding `/api/coding/paas/v4` 模式）。
- R2.3 原 5 协议（kimi 等）保持普通版单条，不再双显 cp。

### R3 Rust 枚举 + 全链
- R3.1 Protocol 枚举 +5 变体（KimiCoding/MiniMaxCoding/...），serde key 与 JSON 一致。
- R3.2 router.rs 路由 + adapter 转换 + quota.rs 配额查询 + estimate.rs 价格 + price_sync 加支（grep glm_coding 现路径复制）。
- R3.3 cargo test + clippy 全绿。

### R4 派生层（defaults.ts）
- R4.1 `buildProtocolsFromPresets(locale)` async，复用 loadDoc 单次 RPC。
- R4.2 `getProtocolColorMap()` async。
- R4.3 injectProtocolHosts 并入 buildProtocolsFromPresets（hosts 派生整合，不再单独注入 PROTOCOLS）。

### R5 调用点改造
- R5.1 ~13 文件改 useState+useEffect 加载派生 map（locale key，cancelled flag 防竞态）。
- R5.2 删 PROTOCOL_LABELS import + fallback，labelMap 单源。
- R5.3 删 PROTOCOL_COLORS import，getProtocolColorMap 单源。
- R5.4 SearchableProtocolSelect 键盘导航（line 67-71 PROTOCOLS.findIndex）改 async 加载后数组。
- R5.5 platformPaste.test mock 调整。

### R6 删除
- R6.1 删 PROTOCOLS / PROTOCOL_COLORS / PROTOCOL_LABELS 常量定义。
- R6.2 ENDPOINT_PROTOCOLS 保留（注释说明：请求格式协议层，非 platform）。
- R6.3 grep 残留 import 清零。

### R7 门禁
- R7.1 yarn build（tsc）过 — async 化 type 全对。
- R7.2 cargo test + clippy 过。
- R7.3 check:i18n 过（新 cp key 5 协议 name 多 locale 完整）。
- R7.4 主仓零改动（worktree 内）。

## Acceptance Criteria

- [ ] JSON 66 协议各含 keywords/color/codingKeyPrefixes（按需）字段
- [ ] 5 独立 cp key 完整（endpoints/models/name 全填）
- [ ] Rust Protocol +5 变体全链路支持（路由/转换/配额/估价）
- [ ] 三常量删，ENDPOINT_PROTOCOLS 保留
- [ ] buildProtocolsFromPresets + getProtocolColorMap 派生层落地
- [ ] ~13 调用点 async 化，首帧 fallback 不崩
- [ ] yarn build + cargo test/clippy + check:i18n 全绿
- [ ] 主仓零改动

## Out of Scope

- ENDPOINT_PROTOCOLS（5 请求格式协议）保留硬编码
- HEALTH_COLORS（用户未提，PlatformCard 健康状态色，非协议色）
- CLIENT_TYPES（客户端模拟选项，非协议）
- 新协议 logo（logo_url 已在 JSON，独立机制）

## Technical Notes

- 派生层复用 docPromise 单次 RPC 缓存（defaults.ts:169）。
- async 化首帧：调用点初始 labelMap/colorMap = {}，useEffect 加载后 setState 触发重渲染（与现 labelMap 模式一致，已有验证）。
- cp key endpoints 真实 URL：参考各平台 coding plan 官方文档（kimi moonshot coding / minimax coding / qianfan coding lite / xiaomi mimo token plan）。
- 相关 spec：.trellis/spec/frontend/index.md（前端约定）+ CLAUDE.md「平台默认配置」节（coding_plan 独立协议方向）。
