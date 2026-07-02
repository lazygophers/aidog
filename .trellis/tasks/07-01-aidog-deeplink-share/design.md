# Design — D1 aidog:// 协议框架 (parent 本体)

> parent 做 D1 (协议框架 + handler); children (D2/D3/D4) 基于本框架的 event 接口各自实现 entity 分享+导入。

## 目标 (D1)
注册 aidog:// scheme + 启动自动注册 + URL 唤起 → parse → path-entity 路由 → emit 前端事件。D1 只做协议层 + 事件分发, 不做具体 entity import 逻辑 (children 做)。

## 依赖
- `tauri-plugin-deep-link` v2 (官方 plugin, 跨平台 scheme 注册)
- exec 自查 tauri-plugin-deep-link v2 精确 API (`DeepLinkExt`, `on_open_url` / `register_all`), 文档 https://v2.tauri.app/plugin/deep-linking/

## URL 格式 (brainstorm 已定)
`aidog://<entity>/<action>?data=<base64>`
- entity: `platform` | `mcp` | `skill` (path[0], 路由分流)
- action: `import` (path[1], 预留扩展)
- query: `data` = base64 编码的内容

## 实现 (D1)

### 1. 依赖 + config
- `src-tauri/Cargo.toml`: + `tauri-plugin-deep-link = "2"`
- `src-tauri/tauri.conf.json`:
  - `plugins.deep-link.desktop` / mobile host (按 plugin 文档)
  - macOS bundle scheme: `CFBundleURLTypes` → scheme `aidog` (via tauri.conf.json `app.macOS` / 或 Info.plist 模板)
  - Win/Linux: plugin 自动注册 (生产构建)
- exec 查现有 tauri.conf.json `plugins` 段结构, 按文档接入

### 2. Rust handler (src-tauri/src/lib.rs)
- `plugin_builder.add(tauri_plugin_deep_link::init())`
- setup 阶段: 注册 `on_open_url` (或 `DeepLinkExt::on_open_url`) 回调
- 回调逻辑:
  1. 取 URL (`aidog://<entity>/<action>?data=<base64>`)
  2. parse: split path → entity + action; query → data
  3. emit 前端事件: `app.emit("aidog-deep-link", DeepLinkPayload { entity, action, data })`
  4. 容错: 非法 URL → log warn + 忽略 (不 panic)
- `DeepLinkPayload` struct (serde): `{ entity: String, action: String, data: String }`

### 3. 前端事件分发 (src/App.tsx 或顶层)
- `listen<DeepLinkPayload>("aidog-deep-link", ...)` 注册一次 (App mount)
- 收到事件 → 按 entity 路由:
  - `platform` → 触发平台 import dialog (D2 实现, D1 留事件钩子 / 全局 state)
  - `mcp` → mcp import dialog (D3)
  - `skill` → skill import dialog (D4)
- D1 实现: 事件监听 + entity 分发到全局事件总线 (如自定义 event 或 state), children 各自 subscribe 自己的 entity
- 推荐: 用简单全局 EventBus (前端) 或 window dispatch CustomEvent, children 各自 `addEventListener("aidog:platform", ...)` 等

### 4. 协议注册 (启动自动)
- 生产构建: plugin 自动注册 scheme (macOS Launch Services / Win registry / Linux .desktop)
- dev 模式: macOS 需手动 `LSRegisterURL` (plugin 文档说明); 文档 README 注明 dev 限制
- 启动时 plugin init 即注册, 无需额外用户操作

## children 接口契约 (D2/D3/D4 依赖)
- D1 emit Tauri event `aidog-deep-link` payload `{entity, action, data}`
- D1 前端分发: 按 entity 二次分发到 `aidog:<entity>` 前端事件
- children 各自:
  - 监听 `aidog:<entity>` 事件 → 拿 data (base64) → decode → 走 entity import 逻辑
  - 实现分享按钮 → 产出 base64 → 生成 `aidog://<entity>/import?data=<base64>` URL + 可粘贴 base64

## 验收 (D1)
1. app 启动 → aidog:// scheme 注册到系统 (macOS Launch Services 可查)
2. 浏览器/其他 app 点 `aidog://platform/import?data=dGVzdA==` → 唤起 app → 前端收到 `aidog-deep-link` event {entity:"platform", action:"import", data:"dGVzdA=="}
3. 非法 URL (无 entity / 无 data) → log warn, 不崩
4. D1 不实现具体 import (children 做), 但 event 分发到 `aidog:<entity>` 可被监听
5. `cargo build` + `cargo clippy` + `yarn build` 全绿
6. dev 模式注册限制文档化 (README 或 task journal 注明)

## 非目标 (D1)
- 不实现 platform/mcp/skill 具体 import 逻辑 (children)
- 不实现分享按钮 UI (children)
- 不实现 base64 内容的 entity 业务 decode (children)

## 风险
- tauri-plugin-deep-link v2 API 细节 (exec 自查文档, 可能版本差异)
- macOS dev 注册需手动 (生产 OK)
- URL 长度 (brainstorm 定直接 base64, 接受超长)
- 前端事件分发机制选择 (EventBus vs Tauri event 直发) — design 选 Tauri event 直发 + 前端 entity 分发
