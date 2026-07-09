# 协议显示用本地化名替代原始类型（protocol-display-name 漏点）

## Goal

修复 3 处协议展示仍用 `platform_type.toUpperCase()`（原始类型键大写，如 "GLM_CODING"）而非本地化名（如 "GLM 编码套餐（智谱）"）的漏点。统一走 `getProtocolLabel` 异步派生 + labelMap fallback 链（与已修复的 `PlatformEditForm.tsx:88` 同模式），让「协议不可更改」的展示位全部显示用户友好的本地化协议名。

**为什么**：用户报「glm_coding 创建后协议不可更改，应该展示的是 name 而非类型」。之前 protocol-display-name 任务修复了 `PlatformEditForm.tsx`（编辑表单 header），但漏了卡片 / 分享预览 / 测试面板 3 处仍 `.toUpperCase()` 显示原始类型键。glm_coding 这种长名 + 下划线键最丑（"GLM_CODING" vs "GLM 编码套餐（智谱）"）。

## 现状（bug 漏点）

grep `platform_type.toUpperCase` 全仓命中 3 处展示用（创建后 / 预览 / 测试）：

| 文件:行 | 现状代码 | 场景 |
| --- | --- | --- |
| `src/components/platforms/PlatformCard.tsx:269` | `{p.platform_type.toUpperCase()} · {getBaseUrl(...)}` | **用户报的创建后卡片 subtitle** |
| `src/components/platforms/SmartPasteModal.tsx:243` | `{share.name} · {share.platform_type.toUpperCase()}` | 分享串识别预览 |
| `src/pages/ModelTestPanel.tsx:113` | `{platform.name} · {platform.platform_type.toUpperCase()}` | 模型测试面板行 |

已修复的参照模式（**不改，仅作为 fallback 链范本**）：
- `src/pages/platforms/PlatformEditForm.tsx:64-74,88,134` — labelMap state + useEffect 拉 `getProtocolLabel`（docPromise 单次 RPC 缓存），渲染用 `labelMap[x] || PROTOCOL_LABELS[x] || x` 三级 fallback。

非 bug（**不动**）：
- `PlatformCard.tsx:221/224/226` `alt={p.platform_type}` — `<img>` 无障碍 alt，键名合理。
- `PlatformCard.tsx:230` / `PlatformPicker.tsx:64` `slice(0,2).toUpperCase()` — 无 logo 时的头像首字母缩写（"GL"），语义不同于类型展示，保留。
- `PlatformPicker.tsx:111` `({p.platform_type})` — picker option 括号注记，非主要展示，本次不动（如需统一另开任务）。
- `PlatformListView.tsx:113/161` 用 `PROTOCOL_LABELS[...]` 硬编码 fallback（非大写、已有 label），非本 bug 模式，不动。

## 真值源（已验证）

- `src/domains/platforms/defaults.ts:169` `getProtocolLabel(protocol, locale)` — 从 `defaults.json` 的 `doc.protocols[protocol].name[locale]` 派生，fallback `locale → "en-US" → protocol key`。复用 `docPromise` 单次 RPC 缓存（`loadDoc` line 78），多调用零额外 RPC。
- `src-tauri/defaults/platform-presets.json` — glm_coding 条目 `name` 全 8 locale 已填（"GLM 编码套餐（智谱）" / "GLM Coding Plan (Zhipu AI)" 等）。
- `PROTOCOL_LABELS`（`src/domains/platforms/constants.ts`）硬编码 map 作 fallback；glm_coding 在其中为 "GLM Coding"。

## Requirements

### R1 PlatformCard.tsx:269 修复

- R1.1 加 `labelMap` state（`useState<Record<string, string>>({})`）+ `useEffect` 拉 `getProtocolLabel(p.platform_type, i18n.language)`，keyed `[i18n.language, p.platform_type]`。同文件既有 isCpProtocol / defaultModels / homepage useEffect 模式（line 108/119/166），新增一个对齐。
- R1.2 渲染改 `{labelMap[p.platform_type] || PROTOCOL_LABELS[p.platform_type] || p.platform_type} · {getBaseUrl(...) || p.base_url}`（三级 fallback 链，与 PlatformEditForm:88 一致）。
- R1.3 `useTranslation()` 已取 `i18n`（line 98 `const { t } = useTranslation()` → 改 `const { t, i18n } = useTranslation()` 取 i18n）。
- R1.4 import 加 `getProtocolLabel` from `../../domains/platforms/defaults`。`PROTOCOL_LABELS` 已 import（line 9）。
- R1.5 卡片是 `memo` 组件列表渲染，每卡一个 useEffect —— docPromise 进程内单次 RPC 缓存，getProtocolLabel 命中缓存零往返，per-card 开销可接受（同既有 isCpProtocol 等多 useEffect 同模式）。

### R2 SmartPasteModal.tsx:243 修复

- R2.1 加 `labelMap` state + `useEffect` keyed `[i18n.language, share?.platform_type]`，share 存在时拉 `getProtocolLabel(share.platform_type, i18n.language)`。
- R2.2 渲染改 `{share.name} · {labelMap[share.platform_type] || PROTOCOL_LABELS[share.platform_type] || share.platform_type}`。
- R2.3 `useTranslation()` 取 `i18n`（line 50 加 `i18n`）。import `getProtocolLabel` + `PROTOCOL_LABELS` from `../../domains/platforms`（defaults re-export 或直 import defaults）。

### R3 ModelTestPanel.tsx:113 修复

- R3.1 同 R1 模式：labelMap + useEffect keyed `[i18n.language, platform?.platform_type]`。
- R3.2 渲染改 `{platform.name} · {labelMap[platform.platform_type] || PROTOCOL_LABELS[platform.platform_type] || platform.platform_type}`。
- R3.3 import 对齐。

### R4 fallback 链一致性

- 三处统一用 `labelMap[x] || PROTOCOL_LABELS[x] || x` 三级链：
  1. JSON name（getProtocolLabel 派生，最准 / 本地化）
  2. PROTOCOL_LABELS 硬编码（docPromise 未就绪 / 协议不在 JSON 时兜底）
  3. 原始 key（终极兜底，禁上一次 `.toUpperCase()`）

### R5 门禁

- R5.1 `yarn build`（tsc + vite）过。
- R5.2 无新增 lint warning（前端无 lint 脚本，tsc 干净即可）。
- R5.3 主仓零改动（worktree 内）。
- R5.4 grep `platform_type.toUpperCase` 在 src/ 剩 0 处展示用（avatar `slice(0,2).toUpperCase()` 不计）。

## Acceptance Criteria

- [ ] PlatformCard.tsx:269 subtitle 显示本地化协议名（如 "GLM 编码套餐（智谱）"），glm_coding 卡片不复显 "GLM_CODING"
- [ ] SmartPasteModal.tsx:243 分享预览显示本地化协议名
- [ ] ModelTestPanel.tsx:113 测试面板行显示本地化协议名
- [ ] 三处 fallback 链一致（labelMap → PROTOCOL_LABELS → key，无 .toUpperCase()）
- [ ] yarn build 过；主仓零改动
- [ ] grep `platform_type.toUpperCase` 展示用剩 0

## Definition of Done

- 3 处 `.toUpperCase()` 展示漏点全修复，统一 labelMap 模式
- glm_coding 创建后卡片 / 分享预览 / 测试面板均显示 "GLM 编码套餐（智谱）" 等本地化名
- fallback 链三级一致
- journal 记录漏点根因（protocol-display-name 任务漏修非编辑表单展示位）+ 修复模式

## Technical Approach

```
每处修复 = 加 labelMap state + useEffect(getProtocolLabel) + 渲染换 fallback 链

PlatformCard.tsx (memo 卡片, per-card useEffect):
  const { t, i18n } = useTranslation();
  const [label, setLabel] = useState("");
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const l = await getProtocolLabel(p.platform_type, i18n.language);
      if (!cancelled) setLabel(l);
    })();
    return () => { cancelled = true; };
  }, [i18n.language, p.platform_type]);
  // line 269: {label || PROTOCOL_LABELS[p.platform_type] || p.platform_type} · {...}

SmartPasteModal.tsx (share?.platform_type keyed):
  同模式, useEffect 依赖 share?.platform_type

ModelTestPanel.tsx (platform?.platform_type keyed):
  同模式
```

## Decision (ADR-lite)

**Context**：3 处展示位漏修，需统一模式。
**Decision**：
1. 全用 `getProtocolLabel` + labelMap + 三级 fallback（与 PlatformEditForm:88 完全一致），非新建 helper hook —— 改动最小，模式复用，搜索性最好（grep `getProtocolLabel` 能找到所有展示位）。
2. 不抽共享 hook —— 3 处分散在不同组件生命周期 / keying（卡片 per-instance / modal share-keyed / panel platform-keyed），抽 hook 收益低于可读性成本。
3. `PROTOCOL_LABELS[x] || x` 两级 fallback 保底（docPromise 未就绪 / JSON 缺协议时不显空）。
**Consequences**：
- 每处加一个 useEffect + state，3 处共 ~30 行；card per-instance 多一次缓存命中（零 RPC）。
- 未来新展示位须记得套同模式（本任务 journal 沉淀此模式作 spec sediment 候选）。

## Out of Scope

- 抽共享 `useProtocolLabel` hook（3 处不够抽，留待第 4 处出现）
- PlatformPicker.tsx:111 option 括号注记（非主要展示，另开任务）
- PlatformListView.tsx 硬编码 PROTOCOL_LABELS 统一为 JSON name（已是 label 非 type，非本 bug）
- 无 logo 头像缩写 `slice(0,2)`（语义不同，保留）

## Technical Notes

- docPromise 单次 RPC 缓存（defaults.ts:76），getProtocolLabel 多调用零额外往返。
- glm_coding 在 platform-presets.json 有全 8 locale name，fallback 链第 1 级即命中。
- PROTOCOL_LABELS（constants.ts:121）`glm_coding: "GLM Coding"` 作第 2 级。
- 既有 guide：`.trellis/spec/guides/cross-layer-rules.md`（getProtocolLabel 跨层派生）+ `.trellis/spec/frontend/index.md`。
