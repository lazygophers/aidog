# platform-presets protocols homepage + logo (本地缓存)

## Goal
为 `src-tauri/defaults/platform-presets.json` 每个 protocol 项加**官网 homepage URL** + **logo 资源** (URL 源 + `~/.aidog/logos/` 本地缓存), 前端 UI 展示协议可识别品牌 (logo + homepage 链接)。logo 不内嵌仓库 (零体积涨), 运行时按需下载缓存到 `~/.aidog/` (类似 models.json 同步模式: 离线可用 + 仓库干净)。

## 决策 (用户已裁)
- ✅ Q1 归属: **独立新 task** (排 `protocols-source-url` 后, 用户裁定)
- ✅ Q2 logo 存法: **本地缓存 ~/.aidog** (非仓库 bundle, 非纯外链; 源 URL + 运行时下载缓存, 离线可用 + 版权风险低)
- ✅ Q3 缓存路径: **`~/.aidog/logos/`** (顶层与 backups/ logs/ 平级, 默认声明)
- ✅ Q4 下载触发: **启动后台异步批量 + 前端懒加载 fallback** (不阻塞启动, 预热 + 补遗)
- ✅ Q5 fallback: **协议首字母圆形** (CSS 生成, 零资源, 不崩)
- ✅ Q6 展示: **协议选择器 + 平台卡** (核心 UI 点, 默认声明)
- ✅ Q7 logo 源: **三路 fallback** (simpleicons.org CC0/GPL → 厂商官网 favicon → clearbit logo api), clearbit 末路 (隐私: clearbit 知用户访问品牌, 仅前两路均无时用)

## 改动范围

### A. platform-presets.json 数据
- 60 protocols 加 `homepage` (string, 官网 URL) + `logo_url` (string, 主源 URL, simpleicons/favicon/clearbit 之一)
- 三路 fallback 顺序在 Rust 同步逻辑硬编码 (非每 protocol 列三 URL, 简化 schema)
- 单 subtask 一次写完 (60 项, 无 fan-out)

### B. Rust 同步机制 (新增, 类比 price_sync 基础设施)
- 新模块 `gateway/logo_sync.rs` (或并入 price_sync 扩展):
  - 启动后台 async task: 遍历 protocols → 检查 `~/.aidog/logos/<protocol>.<ext>` 缓存 → miss 则按三路 fallback 下载 → 写缓存
  - Tauri command `sync_protocol_logos` (批量) + `get_protocol_logo_path(protocol)` (前端懒加载查路径, 缺失触发单 protocol 同步)
  - 不阻塞 app 启动 (tokio::spawn 后台)
- 缓存目录初始化: app 启动确保 `~/.aidog/logos/` 存在

### C. 前端展示 (新基础设施)
- Tauri 2.0 `convertFileSrc` + `asset:` protocol 配置 (tauri.conf.json + capabilities) — 前端访问 `~/.aidog/logos/` 本地文件
- 协议选择器 (SearchableProtocolSelect) + 平台卡 (Platforms) 渲染 logo `<img>`, 缓存 miss 时 fallback 首字母圆形 + 触发懒加载同步
- homepage 链接: 平台详情处 `<a target="_blank">` (协议选择器不展示 homepage, 仅 logo)

### D. TS 类型 (defaults.ts)
- `DefaultsDoc` 协议条目加 `homepage?: string` + `logo_url?: string` (可选)

### 不动
- Rust `ProtocolPreset` struct (透传 raw String, A1 已证)
- source_urls 字段 (归 source-url task)
- models.json 同步逻辑 (复用基础设施思路, 不改 price_sync 业务)

## Acceptance
- [ ] 60 protocols 含 `homepage` + `logo_url`, 零缺失
- [ ] Rust `logo_sync` 模块: 启动后台批量下载 → `~/.aidog/logos/`, 三路 fallback
- [ ] 前端 `convertFileSrc` 配置生效, 协议选择器 + 平台卡渲染 logo
- [ ] 缓存 miss + 下载失败 → 首字母圆形 fallback, 不崩 UI
- [ ] homepage 链接平台详情处可点击新窗口打开
- [ ] 离线场景: 缓存命中 logo 正常显示
- [ ] `yarn build` + `cargo build/test/clippy --lib` 全绿
- [ ] 仓库零 logo 文件 (全部 ~/.aidog/logos/ 缓存)

## What I already know
- 现状 (file:src-tauri/defaults/platform-presets.json): 60 protocols, 字段 = `client_type / endpoints / models / model_list / name / desc`; source-url task (排队) 待加 `source_urls{docs,pricing}`; 本 task 待加 `homepage + logo_url`
- `~/.aidog/` 已有 `platform-presets.json` (运行时同步) — logos 缓存目录应同在此下 (如 `~/.aidog/logos/` 或 `~/.aidog/cache/logos/`)
- 缓存模式类比 `price_sync.rs` (GitHub raw 拉 models.json + 本地写) — 可复用同步基础设施
- 前端展示 logo 需 Tauri `convertFileSrc` 把本地路径转 webview 可访问 URL (或 Tauri command 返 base64 / 路径)

## Open Questions
- (无, brainstorm Q3-Q7 已闭合)

## Requirements (evolving)
- 60 protocols 含 `homepage` (官方站 URL) + `logo_url` (logo 源 URL)
- logo 本地缓存到 `~/.aidog/logos/` (或子路径), 离线可用
- 前端 UI 展示 logo (协议选择器 / 平台卡)
- homepage 可点击跳转 (平台详情处)
- 下载失败 fallback 机制 (默认图标)

## Acceptance Criteria (evolving)
- [ ] 60 protocols 含 `homepage` + `logo_url`, 零缺失
- [ ] Rust 同步机制: 启动/按需下载 logo → 缓存 `~/.aidog/logos/`
- [ ] 前端展示 logo (Tauri convertFileSrc 或 command), 协议选择器 + 平台卡可见
- [ ] 下载失败 fallback 到默认图标, 不崩 UI
- [ ] homepage 链接可点击 (新窗口打开)
- [ ] 离线场景: 缓存命中时 logo 正常显示
- [ ] `yarn build` + `cargo build/test/clippy` 全绿
- [ ] 仓库零 logo 文件 (全部 ~/.aidog 缓存)

## Out of Scope
- source_urls{docs,pricing} (归 source-url task)
- logo 自动校验脚本 (一次性维护)
- logo 版权审计 (用户自担, 缓存机制降低风险)
- models.json 同步逻辑改动 (复用基础设施, 不改 price_sync)

## Technical Notes
- **依赖 (强)**: 与 source-url + i18n task 文件集**完全重叠** (都改 platform-presets.json + ProtocolPreset) → MUST 串行: i18n finish → source-url finish → 本 task start, 禁双 worktree 同改 JSON
- **依赖 (弱)**: source-url task 加 `source_urls` 字段后, 本 task 再加 `homepage + logo_url` — 两次 JSON 改, 不冲突 (不同字段)
- logo 缓存机制是新模式 (动态下载 + 本地缓存), 复杂度 > source-url (纯文本字段), 涉及 Rust 同步 + 前端展示 + Tauri file:// 协议
- 缓存目录选择影响备份 / 清理策略 (~/.aidog 整体备份, logos 子目录是否纳入)
- Tauri 2.0 `convertFileSrc` + `tauri.conf.json` 的 `asset` protocol 配置可能需调 (前端访问本地文件)

## Decision (pending brainstorm)
- Q3-Q7 待 brainstorm 闭合
