# PRD — D2 平台 URL 导入

> parent: 07-01-aidog-deeplink-share。D1 协议框架已完成(commit 46dfc833)：scheme 注册 + URL parse + App.tsx 二次分发 `aidog:${entity}` CustomEvent。

## 目标
`aidog://platform/import?data=<base64>` URL 唤起 app → 解码平台分享 base64 → 走现有 SmartPasteModal 识别 → 用户确认创建平台。补平台「从 URL 导入」页内入口(粘贴 aidog:// URL → 同流程)。

## 决策(已锁)
| 维度 | 决策 |
|---|---|
| URL 编码 | 明文 base64(同平台 ShareModal 现状，用户主动分享场景) |
| 导入路径 | 复用 SmartPasteModal(platformPaste.ts 已识别平台分享 base64，含 paste-base64-recognize 成果)，**不新建 import_platform 命令**(api.ts 无此命令，平台导入历来走智能粘贴) |
| 触发 | deep-link 唤起自动触发 + Platforms 页「从 URL 导入」按钮(粘贴 URL)双入口 |

## 已知(codebase 实证)
- D1 App.tsx:97-106 已 listen `aidog-deep-link` → dispatch `aidog:platform` CustomEvent(detail={action,data})
- SmartPasteModal(Platforms.tsx:2557)已支持 base64 识别，platformPaste.ts extractCompoundFromBase64 识别平台分享 YAML→base64
- 平台分享产 SharePlatform 对象(YAML→base64)，接收方解码 = 等价智能粘贴输入
- Platforms.tsx:1602 applyPaste(SmartPasteApplyResult) 填创建表单

## 交付
1. **App.tsx 订阅 `aidog:platform`** — 拿 detail.data(base64) → 触发打开 SmartPasteModal 预填 data + 自动识别(复用 applyPaste 流程)。冷启动( app 未开时点 URL)由 D1 get_current 补发，此处同样处理。
2. **Platforms 页「从 URL 导入」按钮** — 工具栏加按钮 → 弹输入框接 `aidog://platform/...` URL → 解析 data → 同 #1 注入 SmartPasteModal。
3. **i18n** — `platform.importFromUrl` + `platform.importFromUrl.placeholder`，8 locale 全补。

## 验收
- 浏览器点 `aidog://platform/import?data=<base64>` → 唤起 app → SmartPasteModal 弹出识别到平台字段 → 用户确认创建
- Platforms 页「从 URL 导入」按钮 → 粘贴 URL → 同流程
- dev 模式 macOS deep-link 限制(D1 README 注)→ 文档说明生产构建验证
- `yarn build` + `cargo test/clippy` + `check-i18n` 全绿

## 非目标(YAGNI)
- 新建 import_platform 后端命令(复用智能粘贴)
- 加密配对码(明文 base64，同 ShareModal)
- 批量 URL(单 URL 单平台)

## 调度(串行)
- write-files: `src/App.tsx` + `src/pages/Platforms.tsx` + `src/locales/*.json`
- 与 D3/D4 撞 App.tsx(各自加 listen 不同 entity)→ **D2 → D3 → D4 串行**
- 与 audit(改 Platforms.tsx)撞 → audit finish 后 start

## 风险
- deep-link 唤起时 SmartPasteModal 挂载时机(App 组件需先 render)→ useEffect 延迟注入 or 状态机等 ready
- 冷启动 URL 补发(D1 get_current)与 App listen 时序 → D1 已处理 emit，D2 确保订阅先于 emit 或缓存
