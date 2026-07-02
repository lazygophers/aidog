# PRD — D3 mcp 分享 + URL 导入

> parent: 07-01-aidog-deeplink-share。依赖 D1(协议框架)+ 本 task 先做 ShareModal 泛化(D4 复用)。

## 目标
1. **ShareModal 泛化** — 现硬编码 `SharePlatform` + `platformName`，泛化为接任意 share 对象 + 标题(供 mcp/skill/平台复用)
2. **mcp 分享按钮** — Mcp.tsx mcp 卡片/行加分享按钮 → 弹泛化 ShareModal → mcp 配置 yaml/json/base64 + 生成 `aidog://mcp/import?data=<base64>` URL
3. **mcp URL 导入** — App.tsx 订阅 `aidog:mcp` → 解码 base64 → JSON → 复用 `mcp_import_json`
4. **mcp 粘贴导入** — Mcp.tsx 粘贴入口(复用现成 mcp_import_json，Mcp.tsx:198 已有 JSON 粘贴；补 base64 分享文本识别)

## 决策(已锁)
| 维度 | 决策 |
|---|---|
| 分享内容 | mcp 配置对象(name/url/headers/... 完整配置) |
| URL 编码 | 明文 base64(JSON 包裹)，同平台 ShareModal |
| 导入路径 | 复用 mcp_import_json(api.ts:1843 现成) |
| ShareModal | 泛化(本 task 首个改)，D4 skill 复用 |

## 已知(codebase 实证)
- ShareModal.tsx 接 `share: SharePlatform` + `platformName` + onToast/onClose；3 格式 yaml/json/base64(toBase64Utf8 + formatShare)
- Mcp.tsx:69 有 `selected: Set<string>`(已装 mcp 多选)；:181 filter selected；:568 导入按钮
- mcp_import_json(api.ts:1843) 接 JSON 字符串 → McpImportReport
- App.tsx D1 已 dispatch `aidog:mcp` CustomEvent

## 交付
1. **ShareModal 泛化**(`src/components/platforms/ShareModal.tsx`) — props `share: Record<string,unknown>`(或泛型)+ `title: string` 替代 `platformName`；平台调用点(Platforms/Home/Groups)传 SharePlatform + 平台名不变(向后兼容)；formatShare 接任意对象 yaml 序列化
2. **mcp 分享导出** — api.ts 加 `mcp_share_export(name) -> McpShareConfig`(后端 db.rs 读 mcp 配置返回完整对象) or 前端直接从 mcp list 序列化(ponytail: 前端序列化优先，免新后端命令，若 mcp list item 已含完整字段)
3. **Mcp.tsx 分享按钮** — mcp 卡片/行加分享图标 → mcp_share_export/序列化 → 弹泛化 ShareModal(title=mcp 名)+ 格式切换 + 「复制为 aidog:// URL」按钮(拼 `aidog://mcp/import?data=<base64>`)
4. **App.tsx 订阅 `aidog:mcp`** — detail.data base64 → atob → JSON → mcp_import_json → toast 结果
5. **Mcp.tsx 粘贴导入增强** — 现有 JSON 粘贴入口补:接 base64 分享文本(解码→JSON→mcp_import_json)
6. **i18n** — `mcp.share` + `mcp.share.copyUrl` + `mcp.importFromUrl`，8 locale 全补

## 验收
- mcp 卡片分享按钮 → ShareModal → 3 格式切换 + URL 生成
- 点 `aidog://mcp/import?data=<base64>` → 唤起 → mcp_import_json → 配置入库
- Mcp.tsx 粘贴 base64 分享文本 → 导入成功
- ShareModal 泛化后平台分享回归不破(Platforms/Home/Groups 调用点正常)
- 4 门禁全绿

## 非目标(YAGNI)
- 批量 mcp 分享(单条)
- 加密配对码
- mcp 配置 schema 变更

## 调度(串行)
- write-files: `src/components/platforms/ShareModal.tsx` + `src/pages/Mcp.tsx` + `src/App.tsx` + `src/services/api.ts` + `src/locales/*.json` + 可能 `src-tauri/src/`(若加 mcp_share_export)
- 与 D2 撞 App.tsx；与 D4 撞 ShareModal → **D2 → D3 → D4 串行**
- ShareModal 泛化在本 task(D3)，D4 复用

## 风险
- ShareModal 泛化破平台现有调用 → 回归测 Platforms/Home/Groups 三处
- mcp 配置含敏感 token(URL 明文，同平台 ShareModal 现状，用户主动分享)
- mcp list item 字段是否完整(决定免后端命令 or 加 mcp_share_export)
