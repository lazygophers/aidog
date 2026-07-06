# defaults.json jsDelivr + raw 同步机制

**Parent**: `07-06-platform-defaults-json-sync` (Phase 2, 依赖 defaults-json-extract)

## Goal
Tauri command `sync_defaults_json()` 拉 jsDelivr master defaults.json, `last_updated` (Unix 秒) 比对, 远程较新写 app data。三路触发。

## 改动
1. **新增** `src-tauri/src/gateway/defaults_sync.rs` (或并入 price_sync.rs 同模块) — `sync_defaults_json()`:
   - 主源: `https://cdn.jsdelivr.net/gh/lazygophers/aidog@master/src-tauri/defaults/defaults.json`
   - fallback: `https://raw.githubusercontent.com/lazygophers/aidog/master/src-tauri/defaults/defaults.json` (jsDelivr 失败/缓存滞后时)
   - **架构参考**: child 1 已落 `src-tauri/defaults/defaults.json` (非 `resources/`); price_sync.rs (b8669b93) 已实现同款 jsDelivr 主 + raw 回退, 复用其模式
   - 解析远程 `last_updated` (Unix 秒 i64), 与本地 app data 比对, 远程 > 本地 → 写 `~/.aidog/defaults.json`
   - 本地缺失 → 直接写
   - 返 `{updated: bool, last_updated: i64, source: "jsdelivr"|"raw"|"local", error: Option<String>}`
2. **触发**:
   - 启动 hook 异步触发 (24h 节流, `~/.aidog/defaults.json.last_sync` 时间戳防抖)
   - 每日定时器
   - 设置页手动按钮 (无视节流)
3. **Tauri command** 注册 `sync_defaults_json` + 前端 invoke 封装
4. **设置页 UI** 加「默认配置同步」区: 显示当前 last_updated + 「立即检查更新」按钮 + 同步结果反馈
5. **失败回退**: 离线/网络错误用本地 fallback, 不破坏现有功能, log warn

## Acceptance
- [ ] sync_defaults_json 拉 jsDelivr 主 + raw fallback, last_updated (Unix 秒) 比对, 远程较新才写
- [ ] 启动异步 + 每日定时 + 手动按钮三路触发, 24h 节流
- [ ] 同步失败不破坏现有 (用本地 app data / bundled)
- [ ] 设置页 UI 显示同步状态 + 手动触发
- [ ] cargo test (mock fetch) + yarn build 全绿

## Out of Scope
- defaults.ts 函数 async 化 (child 1)
- purge jsDelivr cache API (接受缓存延迟)

## 依赖
- **child 1 (defaults-json-extract) 已完成** (merged 2e09690b, 落 `src-tauri/defaults/defaults.json` + commands/defaults.rs `get_defaults_json` 已含 app data → bundled 回退; 本 task 写 app data `~/.aidog/defaults.json`, child 1 的 reader 自动优先读 app data)
