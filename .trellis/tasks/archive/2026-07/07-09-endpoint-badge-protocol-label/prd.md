# PlatformCard endpoint badge 协议名补修

## Goal

补修 `src/components/platforms/PlatformCard.tsx:726` expanded detail endpoint badge 仅用 `PROTOCOL_LABELS[ep.protocol]`（硬编码常量回退），未取 JSON `name` 真值（`getProtocolLabelMap` 批量 async）。属 protocol-name-not-type 系列漏点（第 5 处），与已修 4 处（PlatformCard:281 头部 / SmartPasteModal / ModelTestPanel / PlatformPicker）范式不一致。

**为什么**：用户报「平台详情看到的还是 glm_coding 不是 name」。line 726 endpoint badge 展 `PROTOCOL_LABELS[ep.protocol] || ep.protocol` —— 硬编码短 label（如 "GLM Coding"），非 JSON `name` 真值（如「智谱 GLM Coding Plan」本地化名）。其余 4 处已统一走 getProtocolLabel/labelMap → PROTOCOL_LABELS → key 三级回退，line 726 落单。

## 现状

- `PlatformCard.tsx:726`：`{PROTOCOL_LABELS[ep.protocol] || ep.protocol}` —— 单级硬编码回退，无 JSON name
- 已修范式（参照）：line 281 头部用 `protocolLabel || PROTOCOL_LABELS[p.platform_type] || p.platform_type`（protocolLabel 经 getProtocolLabel async line 180）
- endpoint badge 特殊：`ep.protocol` 可能多个不同值（平台含混合 endpoint），非单一 `p.platform_type` → 需 **labelMap 批量**（getProtocolLabelMap，一次 RPC 全表），非逐 endpoint async

## 真值源

- JSON name（本地化真值）：`platform-presets.json` protocols[k].name[locale]
- API：`getProtocolLabelMap(locale)` in `src/domains/platforms/defaults.ts:207`（batch async 一次 RPC 全表 labelMap）
- 硬编码回退：`PROTOCOL_LABELS` in `src/domains/platforms/constants.ts:112`

## Requirements

### R1 line 726 endpoint badge 走 labelMap 三级回退

- R1.1 line 726 改：`{labelMap?.[ep.protocol] || PROTOCOL_LABELS[ep.protocol] || ep.protocol}`（labelMap → 硬编码 → key 三级，与已修 4 处同构）
- R1.2 **labelMap 获取**：PlatformCard 已有 `protocolLabel` state（line 176-185，单 protocol getProtocolLabel）。endpoint badge 需 **batch labelMap**（覆盖所有 ep.protocol）：
  - 方案 A（推荐）：新增 `labelMap` state + useEffect keyed `[i18n.language]` 调 `getProtocolLabelMap(i18n.language)` 取全表，line 726 用 labelMap
  - 方案 B：复用现有 protocolLabel（仅 p.platform_type）—— 不够（endpoint protocol 可能 ≠ platform_type），否决
- R1.3 labelMap useEffect 与现有 protocolLabel useEffect 可合并或并列（实施时择优，倾向并列清晰）

### R2 范式一致

- R2.1 三级回退链 `labelMap?.[protocol] || PROTOCOL_LABELS[protocol] || protocol` 与 PlatformCard:281 / SmartPasteModal:259 / ModelTestPanel:129 / PlatformPicker:115 同构
- R2.2 i18n.language 变化时 labelMap 重取（useEffect keyed [i18n.language]）

### R3 门禁

- R3.1 `yarn build`（tsc）过
- R3.2 grep line 726 已换 labelMap 回退链（非单级 PROTOCOL_LABELS）
- R3.3 主仓零改动（worktree 内）

## Acceptance Criteria

- [ ] line 726 endpoint badge 三级回退链（labelMap → PROTOCOL_LABELS → key）
- [ ] labelMap state + getProtocolLabelMap useEffect keyed [i18n.language]
- [ ] yarn build 过
- [ ] 范式与已修 4 处同构
- [ ] 主仓零改动

## Definition of Done

- endpoint badge 展 JSON name 真值（本地化）
- protocol-name-not-type 系列第 5 漏点补修，契约覆盖 5 处全一致
- journal 记「endpoint badge 漏点补修」

## Technical Approach

```
PlatformCard.tsx 改动:
  新增 state: const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  新增 useEffect:
    useEffect(() => {
      let cancelled = false;
      getProtocolLabelMap(i18n.language).then(m => { if (!cancelled) setLabelMap(m); });
      return () => { cancelled = true; };
    }, [i18n.language]);
  line 726: {labelMap?.[ep.protocol] || PROTOCOL_LABELS[ep.protocol] || ep.protocol}
  import 增 getProtocolLabelMap（若未有）
```

## Decision (ADR-lite)

**Context**：endpoint badge 需覆盖多 ep.protocol，单 protocol async 不够。
**Decision**：用 getProtocolLabelMap batch（一次 RPC 全表），与 PlatformPicker 父组件同模式（批量）。三级回退链复用既有范式。
**Consequences**：labelMap 与 protocolLabel 两个 state 并存（前者 batch 后者单）—— 可后续合并优化，本 task 保并列清晰。

## Out of Scope

- 合并 protocolLabel + labelMap state（重构，另开）
- 改 endpoint badge 视觉样式
- 其他 protocol 展示点（已全覆盖 5 处）

## Technical Notes

- getProtocolLabelMap：`src/domains/platforms/defaults.ts:207`（batch，复用 docPromise 单次 RPC）
- 已修范式范本：`PlatformCard.tsx:176-185, 281`（单 protocol）/ `PlatformPicker.tsx`（batch labelMap prop）
- 既有 guide：`.trellis/spec/guides/cross-layer-rules.md` + `.trellis/spec/guides/code-reuse-rules.md`
