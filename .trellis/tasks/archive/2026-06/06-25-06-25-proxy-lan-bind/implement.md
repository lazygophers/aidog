# Implement — 默认代理支持局域网访问

载体：单 trellis-implement subagent，worktree 隔离（改动紧耦合，签名 ripple，不可并行）。
执行后 trellis-check 验收。

## 触点清单（按依赖顺序，全部一个 agent 串行改）

### 后端 Rust
1. **`src-tauri/src/shared.rs`**
   - `ProxySettings` 结构（line 44-50）增字段：
     ```rust
     #[serde(default = "default_bind_lan")]
     pub(crate) bind_lan: bool,
     ```
     并加 `fn default_bind_lan() -> bool { true }`（serde default 需函数返回 true，非 Default::default 的 false）。
   - 默认值字面量（line 84）`ProxySettings { port: 9876, autostart: true, silent_launch: false }` 增 `bind_lan: true`。
   - `save_proxy_settings` 签名（line 97-108）增 `bind_lan: bool` 参数，结构构造（line 106）带上。
   - **改签名 → 改全部调用点**：
     - `commands/proxy.rs:55`（proxy_start 保存）→ 传 `saved.bind_lan`
     - `commands/proxy.rs:90`（proxy_stop）→ 传 `settings.bind_lan`
     - `commands/proxy.rs:120`（set_autostart）→ 传 `current.bind_lan`
     - `commands/proxy.rs:153`（silent_launch）→ 传 `current.bind_lan`
     - grep 兜底：`rg "save_proxy_settings\(" src-tauri/src` 确保无遗漏。
   - 其它 `ProxySettings { ... }` 字面量构造点也要补 `bind_lan`：
     - `commands/popover.rs:61`
     - `commands/proxy.rs:54`（unwrap_or 的默认）
     - `app_setup.rs:233`

2. **`src-tauri/src/gateway/proxy/mod.rs`**
   - `start_proxy`（line 164）增参数 `bind_lan: bool`。
   - 绑定地址（line 194）改：
     ```rust
     let ip = if bind_lan { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
     let addr = std::net::SocketAddr::from((ip, actual_port));
     ```
   - 改调用点：
     - `commands/proxy.rs:45`（proxy_start）→ 读 `load_proxy_settings` 的 `bind_lan` 传入。
     - `app_setup.rs` autostart 路径调 start_proxy 处（grep `start_proxy(` 定位，约 line 233 附近）→ 传 settings.bind_lan。

3. **`src-tauri/src/commands/proxy.rs`** — 新增 command：
   ```rust
   #[tauri::command]
   pub async fn proxy_set_bind_lan(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
       let current = load_proxy_settings(&app).await?;
       save_proxy_settings(&app, current.port, current.autostart, current.silent_launch, enabled).await?;
       // 绑定地址只在 bind 时读取 → 若代理在跑，重启使生效
       if proxy_status(app.clone())? {
           proxy_stop(app.clone()).await?;
           proxy_start(current.port, app.clone()).await?;
       }
       Ok(())
   }
   ```
   注意 save_proxy_settings 末参顺序与签名一致。

4. **`src-tauri/src/startup.rs`** — invoke_handler 注册 `proxy_set_bind_lan`（仿 line 66 app_set_silent_launch）。

### 前端 TS
5. **`src/services/api.ts`**
   - `ProxySettings` interface（line 267）增 `bind_lan: boolean;`。
   - proxyApi 增：`setBindLan: (enabled: boolean) => invoke<void>("proxy_set_bind_lan", { enabled })`（参数 key camelCase `enabled`，见 [[tauri-invoke-param-camelcase]]）。

6. **AppSettings.tsx**（或代理设置所在 tab，grep `setAutostart`/`silent_launch` 定位）
   - 加开关行：标签「局域网访问」+ 说明「允许同局域网其他设备连接此代理」。
   - 切换调 `proxyApi.setBindLan`，乐观/失败补 toast（见 [[silent-catch-ui-feedback-gap]]）。
   - 初值从 proxy_get_settings 的 bind_lan 读。

### i18n
7. 为新开关标签 + 说明文案在 `src/locales/*.json` 全语言补 key（实测文件清单为准，含阿拉伯 ar）。跑 `node scripts/check-i18n.mjs` 验零缺口（见 [[frontend-i18n-coverage]]）。

## 验证（check 阶段）
- `cd src-tauri && cargo build && cargo clippy`（零 warning）`&& cargo test`
- `yarn build`
- `node scripts/check-i18n.mjs`（若存在）
- 跨层契约核对：api.ts cmd 字符串 ↔ Rust command 名 ↔ 参数 camelCase（[[tauri-invoke-param-camelcase]]）。

## 失败处理
- 调用点遗漏致编译错 → 按 rustc 报错逐个补 bind_lan。
- 同一处连续 2 次失败 → 标 `需要:` 返回 main。
