# 基础信息平台选择显示 name 非 key

## Goal

PlatformEditForm 编辑态「基础信息」section 内协议 chip（line 113）+ header 副标题（line 67）当前显 `protocol.toUpperCase()`（协议键名大写，如 "GLM_CODING"），用户要显 JSON name（多语言真值，如 "GLM 编码套餐（智谱）"）。与新建态 SearchableProtocolSelect（已用 labelMap = JSON name）一致。

## What I already know

- 现状（`src/pages/platforms/PlatformEditForm.tsx`）：
  - line 67 header 副标题：`{editing.platform_type.toUpperCase()} · {getPrimaryBaseUrl(...)}`
  - line 113 基础信息 section 编辑态 chip：`{protocol.toUpperCase()}`
- 新建态用 `SearchableProtocolSelect`（已 labelMap 优先 JSON name）
- `getProtocolLabel(protocol, locale)` 已存在（`src/domains/platforms/defaults.ts:169`）：返回 JSON name（locale → en-US → key fallback）
- SearchableProtocolSelect 已有 labelMap useEffect 模式可参考（line 30-41）

## Requirements

- R1: header 副标题 line 67 protocol 显示 → JSON name（异步 getProtocolLabel），保留 ` · {base_url}` 后缀
- R2: 基础信息 section 编辑态 chip line 113 protocol 显示 → JSON name（异步 getProtocolLabel）
- R3: labelMap state + useEffect（参考 SearchableProtocolSelect:30-41 模式），i18n.language 切换重拉
- R4: labelMap 未加载前 fallback：`PROTOCOL_LABELS[protocol] || protocol`（短暂硬编码 label 优于 key，加载完替换为 name）
- R5: 编辑态 chip 样式保留（背景色 + 大小写：name 不toUpperCase）

## Acceptance

- [ ] line 67 + line 113 显 JSON name（非 protocol.toUpperCase()）
- [ ] labelMap useEffect（i18n.language 依赖）
- [ ] grep `protocol.toUpperCase()` 在 PlatformEditForm = 0（或仅注释）
- [ ] yarn build 0 错
- [ ] 切语言 name 跟随

## Out of Scope

- SearchableProtocolSelect（新建态已正确，不动）
- 协议键名重命名
- 其他页面 protocol 显示（PlatformCard 等已数据驱动）

## Technical Notes

- getProtocolLabel 已存在，直接复用
- PROTOCOL_LABELS fallback（constants.ts）保留加载前兜底
- i18n.language 切换 labelMap 重拉（同 SearchableProtocolSelect 模式）
