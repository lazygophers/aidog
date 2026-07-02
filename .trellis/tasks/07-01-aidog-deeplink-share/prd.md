# PRD — aidog:// 协议 + URL 导入 + skills/mcp 分享

> 用户请求 (/trellisx-flow): 注册 aidog:// 协议 (启动自动注册), 支持 URL 导入平台/skills/mcp; skills/mcp 加独立分享按钮 (逻辑同平台分享); 支持粘贴导入 + URL 导入。

## 目标
1. **aidog:// 协议注册**: 启动自动注册系统 deep link (macOS Launch Services / Windows registry / Linux), `tauri-plugin-deep-link`
2. **URL 导入 3 类** (path 区分 entity, 用户定): `aidog://<entity>/import?data=<base64>` (entity = platform|skill|mcp) → 唤起 app → 按 path 路由 → 解码 → 走对应 import
3. **skills/mcp 分享按钮**: 复用平台 ShareModal 模式, 产出可粘贴 base64 + 可生成 URL
4. **粘贴导入**: skills/mcp 接收方粘贴分享内容导入

## URL 格式 (用户定: path 区分 entity)
- `aidog://platform/import?data=<base64>` — 平台配置 (复用 platform_share_export)
- `aidog://mcp/import?data=<base64>` — mcp 配置 JSON
- `aidog://skill/import?data=<base64>` — skill catalog id 列表
- path 第一段 = entity (路由分流); 第二段 = action (import, 预留扩展); query = data
- handler 解析: match path[0] → entity 路由 → 各 child task 的 import 逻辑

## 现状 (main 调研)
- 平台分享: `ShareModal` (components/platforms/), 3 格式 (yaml/json/base64), 后端 `platform_share_export` 返回含明文 api_key 对象; 用于 Platforms/Home/Groups
- mcp: `mcp_import` / `mcp_import_json` 已有 (粘贴 JSON 导入现成, Mcp.tsx:198)
- skills: 仅 install/search/enable/disable, **无 import/export/share** (npx 管理, id = owner/repo@skill)
- deep-link: tauri-plugin-deep-link **未装**
- `.aidogx` AES-256-GCM 加密容器 (container.rs) — 文件导出用, 非 URL
- Tauri identifier: com.aidog.app

## 决策(brainstorm 已锁)
| 维度 | 决策 | 据 |
|---|---|---|
| skills 分享语义 | **catalog id 列表**(接收方 npx install) | 用户确认(2026-07-02)；ponytail 复用 skills_install，免新后端打包 |
| URL 编码 | 明文 base64(同平台 ShareModal 现状) | 用户主动分享场景；ponytail 不加配对码 |
| URL 长度 | 接受长 URL(单实体 base64 ≤2k) | 不建短码后端(YAGNI) |
| 拆 task | parent(D1 协议框架)+ 3 child(D2 平台/D3 mcp/D4 skill) | D1 已 done(46dfc833) |
| 协议注册 dev 限制 | D1 README 注 macOS dev LSRegisterURL | D1 已处理 |

## 进度
- **D1 ✅ done**(commit 46dfc833，worktree trellisx-07-01-aidog-deeplink-share)：scheme 注册 + URL parse + App.tsx 二次分发 `aidog:${entity}` + 10 单测
- **D2/D3/D4 planning**(prd 已写，等 audit finish 释放 active 槽 + 串行 exec)

## scope (D2/D3/D4 见各 child prd)
1. **协议层**: tauri-plugin-deep-link 接入 + 启动自动注册 + URL scheme handler (Rust) → 前端事件路由
2. **URL 格式**: `aidog://import/<type>?data=<base64>` (type = platform|skill|mcp)
3. **平台 URL 导入**: 反向 platform_share_export → 接收 base64 → 走现有 platform import 路径
4. **mcp URL 导入**: 复用 mcp_import_json (base64 JSON)
5. **skills URL 导入**: 待定语义 (id 列表 install vs 文件打包)
6. **skills 分享按钮**: 产出可分享内容 (待定)
7. **mcp 分享按钮**: 产出 mcp 配置 base64 (复用 ShareModal 模式)
8. **粘贴导入 UI**: skills/mcp 页加粘贴入口

## 待 brainstorm 决策点
1. **skills 分享语义**: 分享 catalog id 列表 (接收方 npx install) vs skill 文件打包 (需新后端)
2. **URL 编码安全**: 明文 base64 (含 api_key, 用户主动分享, 同 ShareModal) vs 加密 (URL 无法带密钥, 需配对码)
3. **URL 长度**: base64 平台配置可能 >2k 字符, 系统 URL 长度限制; 接受长 URL vs 短码后端存
4. **拆 task**: 单 task 顺序推 vs parent (协议框架) + child (3 类导入 + 2 类分享)
5. **协议注册**: tauri-plugin-deep-link 自动注册 (macOS dev 需手动 LSRegisterURL) vs 仅生产构建注册

## 验收 (待细化)
1. 启动 app → aidog:// 协议注册到系统
2. 浏览器/其他 app 点 `aidog://import/platform?data=...` → 唤起 app → 平台导入对话框
3. skills/mcp 同理
4. Skills/Mcp 页有分享按钮 → 弹 ShareModal → 复制 base64 / 生成 URL
5. skills/mcp 页有粘贴导入入口
6. `cargo test` + `cargo clippy` + `yarn build` + `check-i18n` 全绿

## 非目标
- 不改现有平台 ShareModal 逻辑 (复用, 泛化到 skills/mcp)
- 不改 .aidogx 文件格式
- 不实现配对码加密 (除非 brainstorm 定加密)

## 风险
- URL 长度限制 (平台配置 base64 可能超长)
- macOS dev 模式 deep-link 注册需手动 (Launch Services)
- skills 分享语义若选文件打包, 需新后端打包/解包 + skill 目录结构
- 协议注册跨平台差异 (macOS Info.plist / Windows registry / Linux .desktop)
- 安全: URL 明文含 api_key (用户主动分享场景, 同 ShareModal 现状)

## 调度
- **排队**: 撞 export-extra-cleanup (同 ImportExport/分享逻辑), 等 finish 后 start
- brainstorm 定 skills 语义 + URL 编码 + 拆 task → design → start → exec
