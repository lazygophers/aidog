# npx skills 走 aidog 上游代理

## Goal

当 aidog 配置并启用了上游代理（ProxyClientSettings.enabled）时，执行 `npx skills` 命令（list/enable/disable/update/check_env）也通过该代理——给 npx 子进程注入 `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY` 等代理环境变量，使 skill 下载/查询经代理（突破网络限制）。

## 背景（已定位）

- 上游代理配置：`ProxyClientSettings { enabled, proxy_type("socks5"|"http"|"https"), host, port, username, password, dns_over_proxy }`（models.rs:1033）。DB key `proxy/proxy_client`，`http_client::load_proxy_client_settings(db)` 读取（async）。
- 现有 reqwest 走代理：`build_http_client(force_proxy)`（http_client.rs）。
- skills npx 执行：`skills.rs` `run_npx_in_scope`（:318 `Command::new("npx").args(&args).output()`）+ `check_env`(:128) **均未注入代理 env** → npx 走直连，配了代理也不生效。
- skills.rs 函数是 **sync**（Command），`load_proxy_client_settings` 是 **async + 需 &Arc<Db>** → 需由 lib.rs skills 命令（async，有 db State）读代理后**传入** skills.rs。

## Requirements

### R1 — npx 子进程注入代理 env
- `run_npx_in_scope`（及 catalog 抓取的 npx find 回退、check_env 的 npx 探测如适用）：接受一个 `proxy: Option<ProxyEnv>`（或代理 URL `Option<String>`）参数。
- 当代理 enabled：构造代理 URL `{proxy_type}://[user:pass@]host:port`（socks5/http/https），对 npx Command 设 env：`HTTP_PROXY` + `HTTPS_PROXY`（+ `ALL_PROXY` 给 socks5；小写同名亦设以兼容）。npm/npx 读这些 env。
- 代理 disabled 或未配 → 不注入（现有直连行为不变）。

### R2 — lib.rs skills 命令读代理传入
- `skills_enable`/`skills_disable`/`skills_list_installed`/`skills_update`（lib.rs，async + db State）：调 `load_proxy_client_settings(&db)` 取设置 → 构造代理 env（enabled 时）→ 传给对应 skills.rs 函数。
- `skills_check_env`（探测 npx/node）：是否走代理？探测用 `--version` 不联网，可不注入；但 catalog 浏览（HTTP 抓 skills.sh）走 reqwest 时应复用 `build_http_client(force_proxy=Some(true))` 或注入——确认 catalog 抓取路径也尊重代理。

### R3 — socks5 兼容
- npm 原生对 socks5 支持有限。`ALL_PROXY=socks5h://host:port`（dns_over_proxy 时 socks5h，否则 socks5）尽力支持；http/https 代理用 HTTP_PROXY/HTTPS_PROXY。文档/注释说明 socks5 限制。

## Acceptance Criteria

- [ ] 代理 enabled 时，npx skills 子进程含 HTTP_PROXY/HTTPS_PROXY(+ALL_PROXY for socks5) env
- [ ] 代理 disabled/未配 → 不注入，直连行为不变
- [ ] 代理 URL 含认证时格式正确（user:pass@host:port）
- [ ] catalog 抓取（skills.sh / npx find）同样尊重代理
- [ ] lib.rs skills 命令读 proxy 传入 skills.rs（契约清晰）
- [ ] cargo clippy 无新 warning + cargo test 绿（加 proxy_env 构造单测）；yarn build 绿

## Definition of Done

- cargo clippy 无 warning + cargo test 绿；yarn build 绿（前端基本不动）
- 改动落 worktree，闭环 check→commit(merge)→archive
- 更新 [[skills-management-module]]（npx 走上游代理）

## Technical Approach

- skills.rs：加 `fn proxy_env_url(settings: &ProxyClientSettings) -> Option<String>`（enabled→Some(url)）；`run_npx_in_scope` 加 `proxy_url: Option<&str>` 参，构造时 `cmd.env("HTTP_PROXY",u).env("HTTPS_PROXY",u)`（socks5 加 ALL_PROXY）。
- lib.rs：skills_* 命令 `let proxy = http_client::load_proxy_client_settings(&db).await; let url = skills::proxy_env_url(&proxy);` 传入。
- catalog 抓取若用 reqwest → `build_http_client(db, .., Some(true))`；若用 npx find → 同 proxy_url 注入。
- ⚠️ 实施安全：worktree cargo test 用构造的 ProxyClientSettings 断言生成的 env/url，**禁真跑 npx 联网**。

## Out of Scope

- 代理配置 UI（已存在 ProxyClientSettings 设置）
- 非 skills 的其他子进程
- 前端改动（除非 check_env 回显需要）

## Technical Notes

- ProxyClientSettings(models.rs:1033) / load_proxy_client_settings(http_client.rs) / build_http_client(force_proxy)
- skills.rs run_npx_in_scope(:318) / check_env(:128) / catalog 抓取(browse/search)
- npm proxy env: HTTP_PROXY/HTTPS_PROXY/ALL_PROXY（npm_config_proxy/https_proxy 亦可）
- 与并行任务 scripts-py-uv 共享 lib.rs（不同区域: skills 命令区 vs 脚本生成区）→ worktree 隔离, merge 串行解冲突
- 参考 [[skills-management-module]] / [[npx-skills-cli]]
