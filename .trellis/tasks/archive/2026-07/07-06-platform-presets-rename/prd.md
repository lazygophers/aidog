# defaults.json → platform-presets.json 重命名

## Goal
`src-tauri/defaults/defaults.json` 实际内容是平台预设配置 (default endpoints / models / model_list / client_type per protocol), 不是泛指 defaults。重命名为 `platform-presets.json` 让命名对齐语义, 减少 future reader 误解。

## 改动范围 (全量 rename, 仅文件名 + 路径引用, 不改模块名)

### 文件 rename
- `src-tauri/defaults/defaults.json` → `src-tauri/defaults/platform-presets.json` (`git mv`)

### Rust 路径引用
- `src-tauri/src/commands/defaults.rs`:
  - `include_str!("../../defaults/defaults.json")` → `"../../defaults/platform-presets.json"`
  - `dir.join("defaults.json")` → `dir.join("platform-presets.json")`
  - 注释 / log 文本 `defaults.json` → `platform-presets.json`
- `src-tauri/src/gateway/defaults_sync.rs`:
  - URL 常量 `.../src-tauri/defaults/defaults.json` → `.../src-tauri/defaults/platform-presets.json` (jsDelivr + raw 两条)
  - 节流时间戳路径 `~/.aidog/defaults.json.last_sync` → `~/.aidog/platform-presets.json.last_sync`
  - app data 写入路径 `~/.aidog/defaults.json` → `~/.aidog/platform-presets.json`
  - 注释 / log 文本
- `src-tauri/src/app_setup.rs:142` 注释提及 `defaults.json 同步调度器` → `platform-presets.json 同步调度器`

### 文档
- `CLAUDE.md` (项目) 「平台默认配置 (defaults.json)」节: 标题 + 内容 `defaults.json` → `platform-presets.json`

### 不动 (重要边界)
- **Rust 模块文件名保留**: `commands/defaults.rs` / `gateway/defaults_sync.rs` / Tauri command `get_defaults_json` / `sync_defaults_json` / 前端 `getDefaultsJson` / `syncDefaultsJson` —— 内部 API 名, 不影响用户, 改了 churn 太大 (Tauri command rename 破坏 invoke 契约 + serde + 前端封装), YAGNI。如未来需要再单独 task。
- **Tauri resources / build config**: 当前用 `include_str!` 编入二进制, 不依赖 Tauri resources, 无 tauri.conf.json 改动
- **archived task PRDs** 中提及的旧路径不追溯改 (历史归档, 不动)

## Acceptance
- [ ] `git mv` 保留 blame 历史
- [ ] `grep -rn "defaults/defaults\.json\|defaults\\.json" src-tauri/src/ src/ CLAUDE.md` 仅剩 module 名 (`defaults_sync.rs` / `defaults.rs`) 与 i18n key 引用 (若有), 无文件路径残留
- [ ] `cargo build` + `cargo test --lib` + `cargo clippy --lib` (0 新警告) 全绿
- [ ] `yarn build` 全绿
- [ ] dev 启动验证: `get_defaults_json` 仍正常返回 (bundled 路径生效)
- [ ] 同步功能: sync_defaults_json 拉新 URL (platform-presets.json), 节流/app-data 路径正确

## Out of Scope
- Rust 模块 / Tauri command / 前端 invoke API 改名 (churn 大, YAGNI)
- jsDelivr CDN 缓存清理 (24h 自然过期可接受)
- 历史 PRD 文档路径回溯

## 依赖
- 无 (与 locale-zh-hans-rename 文件集不相交: 本 task 改 src-tauri/*, locale 改 src/locales/* + AppContext.tsx; 可并行)
