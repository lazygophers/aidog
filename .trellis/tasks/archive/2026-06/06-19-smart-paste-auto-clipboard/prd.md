---
task: 06-19-smart-paste-auto-clipboard
title: 智能识别弹窗渲染后自动读剪贴板填入
status: planning
created: 2026-06-19
---

# PRD — 智能识别弹窗自动读剪贴板

## 背景

`SmartPasteModal`（`src/components/platforms/SmartPasteModal.tsx`）当前纯手动粘贴：用户打开弹窗后须手动 Ctrl+V。项目**无任何 clipboard 读能力**——capabilities/default.json 无 `clipboard:allow-read-text`，Cargo.toml 无 clipboard 插件，src 内仅 `navigator.clipboard.writeText`（Groups/Home/Logs/Platforms），**零 readText**。locale 残留 `platform.paste.readClipboard` / `platform.paste.clipDenied` 键但对应按钮已删。

需求：弹窗**渲染完成即自动读剪贴板**填入 textarea，免去手动粘贴。

## 为什么用 Tauri 插件而非 navigator.clipboard

`navigator.clipboard.readText()` 在 macOS WKWebView **要求用户手势激活**（WebKit 安全策略）。「渲染完成自动读」恰好无手势 → 大概率被拒静默失败。`tauri-plugin-clipboard-manager` Rust 侧 `read_text` 走 Tauri 权限系统，**无需手势**，可靠。

## 目标

1. 引入 `tauri-plugin-clipboard-manager`（v2，对齐现有 tauri-plugin-* 版本线）。
2. 注册插件 + capability 授 `clipboard:allow-read-text`（仅读，不授 write——写仍走既有 navigator.clipboard）。
3. SmartPasteModal mount 时自动读剪贴板 → 若非空且当前 textarea 空 → 填入；失败/空 → 静默不动（保留手动粘贴兜底）。
4. 防覆盖：用户已手动粘贴后不二次读（仅首次 mount 且 text===空时触发）。

## 范围边界

- **改**：`src-tauri/Cargo.toml`（加依赖）、`src-tauri/src/lib.rs`（`.plugin(tauri_plugin_clipboard_manager::init())`）、`src-tauri/capabilities/default.json`（加权限）、`package.json`（加 `@tauri-apps/plugin-clipboard-manager`）、`src/components/platforms/SmartPasteModal.tsx`（useEffect 自动读）。
- **不改**：`utils/platformPaste.ts`（纯解析函数）、Platforms.tsx 调用点、其它 navigator.clipboard.writeText 用法。
- **不涉及**：iOS/Android（桌面应用）。

## 交付矩阵

| ID | 交付 | 验收 |
| --- | --- | --- |
| D1 | Rust 侧插件注册 + capability | `cargo build` 过；app 启动无插件错误 |
| D2 | 前端 npm 依赖 | `yarn` 装成功；`yarn build` 过 |
| D3 | SmartPasteModal 自动读 | R1-R3 |

## 需求

### R1 — 自动读触发
- SmartPasteModal mount 后（`useEffect([])` 一次）调用 `readText()` from `@tauri-apps/plugin-clipboard-manager`。
- 仅当当前 `text === ""` 才填入（避免覆盖用户在手势窗口期手动粘贴的内容）。
- 填入用 `setText`，后续 `parsed` memo 自动重算识别结果。

### R2 — 失败兜底
- `readText()` reject（权限不足/无剪贴板）→ `catch` 静默，textarea 保持空，用户手动粘贴。
- 不弹错（locale `platform.paste.clipDenied` 键保留不删，备将来手动按钮复用；本轮不用）。

### R3 — 无重复读
- 只在 mount 跑一次。弹窗内 textarea onChange 不触发重读。

## 设计决策

- **不授 write 权限**：写仍走 `navigator.clipboard.writeText`（既有 4 处工作正常），最小权限原则。
- **不在 Platforms.tsx 主列表「添加平台」直进弹窗的手动入口（onManualEntry）路径特殊处理**：onManualEntry 跳过弹窗走空表单，不涉及剪贴板读。
- **不读则已读则全填**：剪贴板含多行杂乱文案 → 整段填 textarea，由 platformPaste.ts 解析器拆字段（既有能力）。

## 单 subtask

S1：Rust 插件 + capability + 前端依赖 + SmartPasteModal useEffect。跨 Rust/前端，但无内部并行依赖，单 agent 顺序执行。

## 验证门禁

```bash
cd src-tauri && cargo build      # 插件注册 + capability 编译
cd src-tauri && cargo clippy     # 零 warning
yarn                             # 装 npm 插件
yarn build                       # tsc + vite
```

手动验证（dev）：复制一段含 base_url+key 的文案 → 打开智能识别弹窗 → textarea 应自动填入 → 识别结果显示。剪贴板空时弹窗 textarea 空、无报错。

## 自检（start 前）

- [ ] R1-R3 全覆盖。
- [ ] capability 仅加 `clipboard:allow-read-text`，不加 write。
- [ ] useEffect 仅 mount 一次，且 `text === ""` 守卫。
- [ ] 失败 catch 静默，无 alert/console.error 刷屏。
