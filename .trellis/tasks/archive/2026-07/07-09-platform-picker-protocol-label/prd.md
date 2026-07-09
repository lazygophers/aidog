# PlatformPicker 下拉协议裸展补修

## Goal

补修 `src/domains/groups/PlatformPicker.tsx:111` 漏点：关联平台下拉 option 裸展 `{p.platform_type}`（如 `glm_coding`），应展协议本地化 label（如「智谱 GLM Coding Plan」）。属已归档 task `07-09-protocol-name-not-type` 的漏修点（彼时只修 PlatformCard / SmartPasteModal / ModelTestPanel 三处）。

## 现状

- `PlatformPicker.tsx:111`：`<option ...>{p.name} ({p.platform_type})</option>` —— platform_type 裸 key 展示（glm_coding）。
- line 64 `{p.platform_type.slice(0,2).toUpperCase()}` 头像缩写 = by design（非 bug，保留）。
- 已修范式（参照）：`PlatformCard.tsx:281` / `SmartPasteModal.tsx:259` / `ModelTestPanel.tsx:113` 用 `protocolLabel || PROTOCOL_LABELS[p.platform_type] || p.platform_type` 三级回退链，label 经 `getProtocolLabel(p.platform_type, i18n.language)` async + useEffect keyed `[i18n.language, ...]`。

## Requirements

### R1 PlatformPicker option 展协议 label

- R1.1 line 111 option 文本：`{p.name} ({protocolLabel})` 替 `{p.name} ({p.platform_type})`，protocolLabel 走 labelMap 回退链（labelMap[platform_type] || PROTOCOL_LABELS[platform_type] || platform_type）。
- R1.2 labelMap 获取：**父组件**（GroupCreateForm / GroupEditForm 等使用 PlatformPicker 的容器）一次性 `getProtocolLabelMap(i18n.language)` 取全表 labelMap，**作为 prop 传入 PlatformPicker**（避免每个 option 单独 async，dropdown 渲染同步）。
  - 备选：PlatformPicker 内部 useEffect 取 labelMap（自包含）—— 若父组件改动面大则采此。**实施时 grep 调用点数量决定**，倾向 prop 注入（父已有 useEffect 模式可复用）。
- R1.3 line 64 头像缩写 **不动**（slice(0,2) toUpperCase 是视觉缩写非协议名展示，保留）。

### R2 跨层 / 契约

- R2.1 PROTOCOL_LABELS 常量来源：`src/domains/platforms/constants.ts`（与已修 3 处同源）。
- R2.2 getProtocolLabelMap：`src/domains/platforms/defaults.ts:207`（async, batch, 一次 RPC 全表）。
- R2.3 i18n.language 变化时 labelMap 重取（useEffect keyed [i18n.language]）。

### R3 门禁

- R3.1 `yarn build`（tsc）过。
- R3.2 grep `PlatformPicker` 调用点全部注入 labelMap prop（无 TS undefined 报错）。
- R3.3 主仓零改动（worktree 内）。

## Acceptance Criteria

- [ ] line 111 option 展 labelMap 回退链（非裸 platform_type）
- [ ] line 64 头像缩写保留
- [ ] labelMap 经 prop 或内部 useEffect 获取（实施时按调用点数量择优）
- [ ] yarn build 过
- [ ] 主仓零改动

## Definition of Done

- PlatformPicker 下拉 glm_coding → 协议本地化 label
- 范式同 protocol-name-not-type 三处已修点（labelMap 三级回退）
- journal 记「protocol-name-not-type 漏点补修」

## Technical Approach

```
方案 A（倾向）: 父组件注入 labelMap prop
  PlatformPicker 加 prop: labelMap?: Record<string, string>
  line 111: {p.name} ({labelMap?.[p.platform_type] || PROTOCOL_LABELS[p.platform_type] || p.platform_type})
  父组件: const [labelMap, setLabelMap] = useState({})
    useEffect(() => { getProtocolLabelMap(i18n.language).then(setLabelMap) }, [i18n.language])
    <PlatformPicker labelMap={labelMap} ... />

方案 B（备选，若调用点多 prop 注入面大）: PlatformPicker 内部 useEffect
  自取 labelMap state，line 111 同回退链
```

## Decision (ADR-lite)

**Context**：已归档 task 漏点，1 文件微修，复用既有 labelMap 范式。
**Decision**：
1. 复用 protocol-name-not-type 三级回退链（labelMap → PROTOCOL_LABELS → key），不发明新模式。
2. labelMap 获取方式（prop 注入 vs 内部 useEffect）由实施时按调用点数量择优 —— 调用点 ≤2 用 prop，≥3 用内部 useEffect。
3. line 64 头像缩写保留（非协议名展示，by design）。
**Consequences**：protocol 展示契约（禁裸 platform_type.toUpperCase / key）现覆盖 4 处全部一致。

## Out of Scope

- 改 line 64 头像缩写（非 bug）
- 重构 PlatformPicker props 架构
- presets-view HTML proto badge（内部工具 by design）

## Technical Notes

- 已修范式范本：`src/components/platforms/PlatformCard.tsx:180-185, 281`（useEffect + getProtocolLabel + 回退链）。
- getProtocolLabelMap：`src/domains/platforms/defaults.ts:207`（批量 async，一次 RPC）。
- PROTOCOL_LABELS：`src/domains/platforms/constants.ts`（硬编码回退常量）。
- 既有 guide：`.trellis/spec/guides/cross-layer-rules.md`（契约稳定）+ `.trellis/spec/guides/code-reuse-rules.md`（复用 labelMap 范式）。
