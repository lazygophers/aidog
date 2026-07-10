# 前端派生层 + 删 3 常量 + 调用点 async 化

## Goal
删 `constants.ts` 三常量（PROTOCOLS/PROTOCOL_COLORS/PROTOCOL_LABELS），建 async 派生层（从 JSON 经 loadDoc 派生），13 调用点全 async 化（useState+useEffect，与现 labelMap 模式一致）。ENDPOINT_PROTOCOLS 保留硬编码（请求格式协议层，非 platform）。

## Requirements
### R1 派生层（defaults.ts）
- R1.1 `buildProtocolsFromPresets(locale?): Promise<ProtocolOption[]>`：loadDoc → 每 key 派生 {value:key, label:name[locale]||en-US||key, codingPlan:is_coding_plan||false, keywords:keywords||[], hosts:派生自endpoints, codingKeyPrefixes:codingKeyPrefixes||[]}。injectProtocolHosts 逻辑并入（hosts 不再单独注入 PROTOCOLS）。
- R1.2 `getProtocolColorMap(): Promise<Record<Protocol,hex>>`：loadDoc → 每 key color 字段。
- R1.3 labelMap（getProtocolLabelMap）已存在，覆盖 PROTOCOL_LABELS 删除后全部 fallback。

### R2 删/改常量
- R2.1 删 PROTOCOLS（line 6）/ PROTOCOL_COLORS（line 202）。
- R2.2 **PROTOCOL_LABELS 改薄**：仅保留 5 请求格式协议条目（`openai`/`openai_responses`/`openai_completions`/`anthropic`/`gemini`），删全部 platform 类型 label（glm/kimi/minimax/... 60+ 条 → 由 JSON name 经 labelMap 派生）。5 请求格式协议不在 presets JSON platform 列表（是 endpoint.protocol 请求层），label 留 PROTOCOL_LABELS 硬编码。
- R2.3 ENDPOINT_PROTOCOLS（line 84，5 请求格式协议数组）保留 + 注释说明（请求格式协议层，非 platform，不迁 JSON）。PROTOCOL_LABELS 5 条与 ENDPOINT_PROTOCOLS 5 条同集（Record vs array 不同 shape，服务不同 consumer）。
- R2.4 grep 残留 PROTOCOL_LABELS 调用点：平台类型展示处全改 labelMap 单源（JSON name），请求格式展示处继续用 PROTOCOL_LABELS 5 条。
- R2.5 grep 残留 import 清零。

### R3 调用点 async 化（13 文件）
- R3.1 PROTOCOLS 调用点：SearchableProtocolSelect（含键盘导航 findIndex 改 async 加载后数组）/ ccswitchMatch / Sub2ApiImport / PlatformEditForm / platformPaste.test / defaults.ts。
- R3.2 PROTOCOL_COLORS 调用点：ProtocolLogo / PlatformCard / PlatformListView / PlatformEditForm → useState colorMap + useEffect getProtocolColorMap。
- R3.3 PROTOCOL_LABELS 调用点（9 文件）：PlatformPicker / SearchableProtocolSelect / SmartPasteModal / PlatformCard / ModelTestPanel / usePlatformForm / MultiKeyPreview / PlatformListView / PlatformEditForm → 删 PROTOCOL_LABELS fallback，labelMap 单源。
- R3.4 首帧 fallback：初始 map = {}，useEffect 加载后 setState 重渲染（与现 labelMap 模式一致）。cancelled flag 防竞态。locale key `[i18n.language]`。

### R4 门禁
- R4.1 yarn build（tsc + vite）过 — async type 全对。
- R4.2 check:i18n 过（5 新 cp key name 多 locale 完整）。
- R4.3 platformPaste.test 调整 mock（PROTOCOLS async 化）。

## Acceptance
- [ ] 三常量删，ENDPOINT_PROTOCOLS 保留
- [ ] buildProtocolsFromPresets + getProtocolColorMap 落地
- [ ] 13 调用点 async 化，首帧不崩（fallback 空 map）
- [ ] yarn build + check:i18n 全绿
- [ ] 主仓零改动

## Dependencies
depends_on: 07-10-protocols-json-schema（派生源字段需 JSON 先有）, 07-10-protocols-rust-enum（跨层 Protocol 类型对齐）。
