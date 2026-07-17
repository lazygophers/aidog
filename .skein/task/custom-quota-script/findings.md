# 自定义 quota 脚本支持 — 调研收敛

## 现状（file:line 引用）

- `query_quota(db, base_url, api_key, platform_id)` 入口 → `query_quota_inner` 按 base_url host 硬编码 if-else 路由（`quota/mod.rs:52-100`）。
- 现有 provider: balance（deepseek/stepfun/siliconflow/openrouter/novita）+ coding_plan（kimi/glm/minimax）+ newapi（两步）+ devin（ACU）。
- `BalanceInfo`（`quota/http.rs:47`）: remaining/total/used/currency/is_valid。
- `CodingPlanInfo`（`quota/http.rs:61`）: tiers[name/resets_at/utilization/limit/remaining]/level。
- `PlatformQuota`（`quota/http.rs:32`）: success/error/queried_at/balance/coding_plan/newapi_user_id。
- 统一出站 HTTP `quota_get_json`（`quota/http.rs:130`）落 proxy_log(source_protocol="quota")，复用 `http_client::build_http_client_system`（proxy/trellis-14：禁新建 reqwest::Client）。
- 现有 spawn（`commands_ai_tools/script_executor.rs:47` `sh -c curl|sh` / `skills/npx.rs:37` / `notification/tts.rs:47` / `shared.rs:150`）**全部裸 Command::output() 无 timeout/cap/env 隔离**，不可复用于用户脚本。
- 模板替换先例：`notification/test_render.rs` + `commands_proxy/mitm.rs` 已有 `{{var}}` 正则命中。

## 技术选型（researcher 调研）

### JSONPath: `serde_json_path` 0.7.x
- RFC 9535 全合规，nom parser，官方 CTS CI 回归。
- API: `JsonPath::parse("$.data.balance")?.query(&value)` → `NodeList`，`.first().map(|n| n.value())`。
- aidog 现状：**无 jsonpath 依赖**（Cargo.toml/lock 零命中），全新引入。
- 备选：jsonpath-rust 2.x（pest，legacy API 面）。不推荐 jsonpath_lib（未维护）/ rsonpath（YAGNI）。

### 脚本 spawn 安全（OWASP + tokio）
| 维度 | 配置 |
|---|---|
| Shell | **禁** `sh -c`/`cmd /C`，直接 `Command::new(path).args(argv)` argv 单元素 |
| Timeout | `tokio::time::timeout(30s, child.wait())` + 超时 `kill` + `kill_on_drop(true)` |
| stdout cap | 禁 `.output()`（OOM），`take(64 KiB)` |
| stdin | `Stdio::piped()` 写 platform config JSON，禁 argv（防 ps 泄漏+注入）|
| env | `env_clear()` 仅注入 PATH+HOME+LANG；Windows 回填 SystemRoot/TEMP/USERPROFILE |
| cwd | `current_dir(temp_dir())` |
| 退出码 | 0=成功 stdout JSON / 非 0=失败 stderr |
| 路径 | 用户填脚本文件绝对路径（shebang 起首），禁一行 shell 字符串 |

残余风险（目标仅「防误配+防上游注入+防 OOM」，非防用户恶意自伤）：脚本同 uid 可读 ~/.aidog/*，无 FS sandbox（Seatbelt/bubblewrap YAGNI）；PATH 污染靠文档提示绝对路径；降权 uid/gid YAGNI。

### HTTP 模板: 正则 `{{var}}` 严格替换
- `regex::new(r"\{\{([a-z_][a-z0-9_]*)\}\}")` + `replace_all` + 闭包查表。
- **严格模式**: 未知变量报错（禁静默空字符串透传上游）。
- 纯正则无表达式求值 → 天然禁模板注入。
- 占位变量: `{{api_key}}` / `{{base_url}}` / `{{platform_id}}` / `{{platform_name}}` / `{{group_name}}` / `{{timestamp}}` / `{{user_id}}`。
- **禁** `{{extra}}` 整对象 dump（含端点凭证）；白名单显式 `{{extra.<key>}}`。

## 需要

无外部阻塞。JSONPath 新依赖 + spawn 安全配置在 design 内定。
