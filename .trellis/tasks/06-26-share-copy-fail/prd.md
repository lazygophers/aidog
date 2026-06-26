# 分享按钮复制失败修复

## 背景
用户反馈: 「点击分享平台的按钮，提示复制失败」

## 根因 (已定位)
`src-tauri/capabilities/default.json:31` 仅含 `clipboard-manager:allow-read-text`, **缺 `allow-write-text`**。
- `ShareModal.tsx:54` `await writeText(text)` (Tauri plugin) → 权限拒 → catch → toast `platform.share.copyFail` "复制失败"
- `Platforms.tsx:2790` `navigator.clipboard.writeText(apiKey)` — 非 share 按钮 (apiKey 复制), WKWebView 无手势静默失败 (memory [[wkwebview-html5-dnd-drop-fails]] 同类 WKWebView 限制)

## 需求
1. `capabilities/default.json` 补 `clipboard-manager:allow-write-text` (修 share 复制)
2. `Platforms.tsx:2790` apiKey 复制改用 Tauri `writeText` (统一, 免 WKWebView navigator 静默失败) — 顺带修, grep 全仓 `navigator.clipboard` 确认有无其他调用点一并迁移

## 现状
- ShareModal.tsx:54 已用 Tauri writeText (正确, 仅权限缺)
- ShareModal.tsx:5 注释自承 "macOS WKWebView 无手势激活时 navigator.clipboard 被拒静默失败, Tauri 侧走权限系统更可靠"
- memory: SmartPasteModal readText 正常 (allow-read-text 已配); writeText 路径未配权限 = 复制失败根因

## 验收
- capabilities 补 allow-write-text
- 全仓 `navigator.clipboard.writeText` 迁移到 Tauri writeText (或确认无其他调用)
- ShareModal 复制成功 (dev 验, agent 跑 cargo build 验 capabilities 生效)
- cargo clippy 0 warning, yarn build + check-i18n 全绿
- 主仓零改动, 全在 worktree

## 风险
- capabilities 改动需重新构建 (Tauri ACL manifest 重生成) — agent 须 cargo build 验证
- 若还有其他 clipboard 操作 (readFileSync 等), grep 全覆盖
