# 平台分享补 aidog:// 地址 + 二维码

## Goal

平台分享弹窗缺 `aidog://` 深链地址按钮 + 二维码。修 urlScheme 漏传 + 给 ShareModal 加 QR。

## What I already know

**Bug 1 — 平台 ShareModal 没传 urlScheme（地址按钮不显示）**：
- `PlatformListView.tsx:194-196` 渲染 ShareModal 只传 share + title，**没传 urlScheme**
- 对照：`McpModals.tsx:310` 传 `urlScheme="aidog://mcp/import"` ✓；`SkillModals.tsx:198` 传 `urlScheme="aidog://skill/import"` ✓
- ShareModal.tsx:216-224：`urlScheme` 存在才渲染「复制为 aidog:// 链接」按钮
- 修法：PlatformListView 补 `urlScheme="aidog://platform/import"`

**Bug 2 — 二维码从未实现**：
- ShareModal 无 QR 组件（mcp/skill/平台 三处都无）
- 用户要「二维码」

**接收端全通（已验证）**：
- 协议注册：`tauri.conf.json:51-53` deep-link schemes `["aidog"]` + `tauri-plugin-deep-link`
- 协议层：`deep_link.rs:48-70` parse `aidog://<entity>/<action>?data=<base64>` → emit `aidog-deep-link`
- 前端分发：`App.tsx:112-117` emit → dispatch `aidog:<entity>` + window 缓存
- platform 接收：`usePlatformsState.ts:598-623` `openDeepLinkImport` + SmartPasteModal 预填（:149, :38-39）
- 即：地址复制后，接收端可唤起导入 ✓

## Assumptions (temporary)

- ShareModal 泛化加 QR（urlScheme 存在时显示）= 一处改三处复用（ponytail）
- QR 内容 = aidog:// URL（扫即唤起导入，最合"分享地址+二维码"语义）
- QR 库：package.json 现有 QR 库优先；无则加 `qrcode` npm 包（成熟标准）

## Open Questions

- [x] QR 加在哪 + 内容？→ **用户选 A**：所有 ShareModal 统一加（urlScheme 存在时显示），QR 内容 = aidog:// 深链 URL

## Decision (ADR-lite)

**Context**: 平台分享缺 aidog:// 地址按钮 + 二维码。ShareModal 已泛化（mcp/skill 共用），已有 urlScheme prop + copyUrl 逻辑，接收端全通。
**Decision**: 方案 A — ShareModal 统一加 QR（urlScheme 存在时显示），QR 内容 = copyUrl 同款 `${urlScheme}?data=<base64 JSON>` URL；PlatformListView 补传 urlScheme。
**Consequences**: 一处改三处复用（平台/mcp/skill）；新增 qrcode npm 依赖；超长 URL 降级隐藏 QR。

## Design

### Bug 1 修：PlatformListView 补 urlScheme
- `PlatformListView.tsx:194-196` ShareModal 加 `urlScheme="aidog://platform/import"`
- 对照 McpModals:310 / SkillModals:198
- 不需 titleKey/warningKey 覆盖（用默认 platform.* key）

### Bug 2 加：ShareModal 加 QR（urlScheme 存在时）
- QR 内容 = copyUrl 同款 URL（`${urlScheme}?data=<base64(JSON.stringify(share))>`），与「复制为 aidog:// 链接」按钮一致
- QR 库：新增 `qrcode` npm 包（`qrcode.toDataURL(url)` → img 渲染，成熟标准）
- 显示位置：内容预览 `<pre>` 下方 + 操作按钮上方（独立 QR 区块，含「扫码导入」提示文案）
- **超长降级**：URL 长度 > 2953 字符（QR v40 L 二进制上限）→ 不渲染 QR + 显示「内容过长，二维码不可用，请用上方复制链接」提示
- urlScheme 不存在（理论上当前所有调用点都传了，但防御）→ 不显示 QR（与按钮同条件）

### i18n
- 新 key：`platform.share.scanToImport`「扫码导入」/ `platform.share.qrTooLong`「内容过长，二维码不可用，请用复制链接」
- 8 locale 全补（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）

## Implement 编排 (subtask 并行)

| subtask | 文件 | 工作单元 | 并行 |
|---|---|---|---|
| A | `src/components/platforms/ShareModal.tsx` + `package.json`（yarn add qrcode）+ 8 locale json | 加 QR 组件 + 超长降级 + i18n key | 文件集与 B 不相交 → 可并行 |
| B | `src/pages/platforms/PlatformListView.tsx` | 补 urlScheme prop | 可并行 |

main 调度：A + B 并行派 2 trellis-implement（共享 task worktree，并发上限 2，文件不相交）。

## Requirements

- PlatformListView ShareModal 传 `urlScheme="aidog://platform/import"`（Bug 1）
- ShareModal 加 QR（urlScheme 存在时显示，内容 = aidog:// URL，超长降级）（Bug 2）
- 新增 qrcode npm 依赖
- 8 locale 新 key 补齐

## Acceptance Criteria

- [ ] 平台分享弹窗显示「复制为 aidog:// 链接」按钮 + QR
- [ ] mcp/skill 分享弹窗也显示 QR（复用，不破坏现有）
- [ ] QR 扫码得 aidog:// URL（接收端可唤起导入）
- [ ] 超长 URL（>2953）隐藏 QR + 显示降级提示
- [ ] 8 locale 新 key 齐全（scripts/check-i18n.mjs 过）
- [ ] cargo clippy 0 warning（无 Rust 改动，预期无变化）/ yarn build 过

## Out of Scope

- deep-link 协议注册（已存在）
- platform/mcp/skill import 接收端（已存在）
- QR 样式美化（沿用 glass 风格，最小可用）

## Requirements (evolving)

- 平台 ShareModal 补 urlScheme（Bug 1 修）
- ShareModal 加二维码（Bug 2）

## Acceptance Criteria (evolving)

- [ ] 平台分享弹窗显示「复制为 aidog:// 链接」按钮 + 复制有效（接收端可唤起导入）
- [ ] 分享弹窗显示二维码，扫码可得分享内容 / 唤起导入
- [ ] 不破坏 mcp / skill 现有分享链路
- [ ] cargo clippy 0 warning / cargo test 过 / yarn build 过

## Definition of Done

- 上述 AC 全过
- QR 超长（超 QR v40 容量）有降级处理（不崩）

## Out of Scope (explicit)

- deep-link 协议注册（已存在）
- platform import 接收端（已存在）
- （待澄清后补）

## Technical Notes

- ShareModal：`src/components/platforms/ShareModal.tsx`
- 平台渲染处：`src/pages/platforms/PlatformListView.tsx:194`
- mcp 渲染（对照）：`src/pages/Mcp/McpModals.tsx:305-310`
- skill 渲染（对照）：`src/pages/Skills/SkillModals.tsx:193-198`
- 接收端：`src/pages/platforms/usePlatformsState.ts:598-623`
- **QR 容量风险**：aidog://platform/import?data=<base64 JSON>，典型平台配置 JSON ~1.2KB → base64 ~1.6KB → URL ~1.64KB，QR v40 L 级二进制上限 2953B 通常可容；极端超长（多模型 + 长 desc）需降级提示
