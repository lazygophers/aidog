# 自定义 quota 脚本支持 — 详细设计

## 架构定位

用户在 per-platform `extra.custom_quota` 配置 HTTP 模板 或 外部脚本，aidog 调度时 override 优先走 custom，跳过内置 host 匹配。核心逻辑下沉 `aidog_core/src/gateway/quota/custom.rs`，command 壳在 `commands_platform`（arch/trellis-03）。

## 接入点（custom override 优先）

`commands_platform/src/quota.rs::platform_query_quota`（及 cold_start_init_tray_estimates）调度：

```
读 platform.extra.custom_quota
  ├─ 有值 → query_custom_quota(db, extra, base_url, api_key, platform_id) （新 entry）
  └─ 无值 → query_quota(db, base_url, api_key, platform_id) （原 host 匹配，签名不变）
```

`query_quota` 原签名不动（波及 N 调用点，禁改）。custom 独立 entry `query_custom_quota(db, extra: &Value, base_url, api_key, platform_id)`。

## 配置 schema（platform.extra.custom_quota，nested）

```jsonc
{
  "custom_quota": {
    "type": "http_template",  // 或 "script"
    // http_template 分支
    "method": "GET",
    "url": "https://api.example.com/balance?key={{api_key}}",
    "headers": { "Authorization": "Bearer {{api_key}}" },
    "body": null,
    "response_mapping": {
      "balance": { "remaining": "$.data.remaining", "used": "$.data.used", "total": "$.data.total", "currency": "$.data.currency" },
      "coding_plan": { "level": "$.plan.level", "tiers": [{ "name": "five_hour", "utilization": "$.plan.fh_pct", "resets_at": "$.plan.fh_reset" }] },
      "metrics": { "requests_today": "$.usage.requests" }
    },
    // script 分支
    "command": ["/abs/path/script.py"],
    "stdin": "config",
    "response_mapping": { /* 同上, 作用于 stdout JSON */ }
  }
}
```

字段契约（planning 锁，跨 Rust serde + TS type + JSON 三层一致，见 [[skein-parallel-field-shape-contract]]）:
- `type`: `"http_template"` | `"script"`
- `response_mapping.{balance,coding_plan,metrics}` 三段全可选，任一缺省 → PlatformQuota 对应 None。
- JSONPath 字面量 RFC 9535（`$.` 起首）。
- `command` 首元素绝对路径，禁 shell 字符串。

## 模板替换引擎（{{var}} 正则严格）

`render_template(tpl: &str, vars: &HashMap<&str, String>) -> Result<String, String>`:
- `regex::new(r"\{\{([a-z_][a-z0-9_]*)\}\}")` 全局匹配 + 闭包查 vars。
- 命中替换；**未命中报错**（严格模式，禁空字符串透传上游）。
- 禁 `{{extra}}` 整 dump；`{{extra.<key>}}` 白名单显式枚举。
- 复用先例 `notification/test_render.rs` + `commands_proxy/mitm.rs`。

vars: api_key / base_url / platform_id / platform_name / group_name / timestamp / user_id + extra 白名单。

## JSONPath 提取（serde_json_path）

`extract_by_path(value: &Value, path: &str) -> Result<Value, String>`:
- `JsonPath::parse(path)?.query(value).first().map(|n| n.value().clone()).ok_or("no match")`。
- response_mapping 各字段提取 → 填 BalanceInfo/CodingPlanInfo/metrics。
- 提取失败（路径无效/无命中）→ 该字段 None + warn（禁整体 fail）。

## 脚本 spawn（安全配置）

`run_custom_script(command: &[String], stdin_config: Option<Value>) -> Result<Value, String>`:
- `tokio::process::Command::new(&command[0]).args(&command[1..])` argv 单元素，**禁 shell**。
- `.env_clear()` + 注入 PATH/HOME/LANG（Windows 回填 SystemRoot/TEMP/USERPROFILE）。
- `.current_dir(temp_dir())` + stdin/stdout/stderr piped + `.kill_on_drop(true)`。
- stdin="config" → 写 platform config JSON。
- `tokio::time::timeout(30s, ...)` + stdout cap 64 KiB（禁 `.output()`）。
- 超时 → kill + Err；非 0 → Err(stderr)；0 → parse stdout JSON。
- 失败处理复用 trellis-08（禁新错误分类）。

## 返回（PlatformQuota 扩展）

`PlatformQuota` 加字段:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub custom_metrics: Option<serde_json::Value>,
```

`query_custom_quota` 按 response_mapping 填 { balance, coding_plan, custom_metrics }。

## HTTP 出站

新写 `custom_http_fetch`（模板化 url/headers/body + JSONPath 提取非整 body，不适配 `quota_get_json`），**复用 `http_client::build_http_client_system`**（proxy/trellis-14，禁新建 reqwest::Client）。落 proxy_log(source_protocol="quota") 同模式。

## 落库 + cold_start

- `est_balance_remaining`: custom `balance.remaining` 写同字段。
- `cold_start_init_tray_estimates`: 加 custom 分支。
- 缓存复用现有 quota 缓存机制。

## 前端

- `formSections.tsx` 加 `CustomQuotaSection`（protocol 无关）: type 选 http_template/script + 模板编辑器 + response_mapping（balance/coding_plan/metrics 三段 JSONPath）+ 「测试」按钮。
- `platforms.ts`: `parseCustomQuotaConfig`/`serializeCustomQuotaConfig`（nested extra.custom_quota）+ `quotaApi.queryCustom`/`testCustom`。
- `part1.ts`: CustomQuotaConfig TS 类型（cross-layer/trellis-20 字段同步）。
- i18n 8 locale（i18n/trellis-19，check-i18n.mjs）。
- modal `createPortal(document.body)`（modal-window-center-rule）。

## command 注册

- `commands_platform/src/quota.rs`: `query_custom_quota(platform_id)` + `test_custom_quota(config)`（test 不落库即时返）。
- `startup.rs` 注册。新 command 需 `yarn tauri dev` 重启（tauri-rust-command-needs-restart）。

## 取舍

- **custom 独立 entry 不改 query_quota 签名**: 避免波及 N 调用点，command 层分流。
- **正则模板非引擎**: YAGNI，无条分支需求。
- **serde_json_path 新依赖**: 唯一 RFC 9535 合规选型。
- **脚本禁 shell + env_clear**: OWASP 核心，残余风险（同 uid 无 FS sandbox）YAGNI 声明。
- **任意 metrics 用 serde_json::Value**: 灵活，前端 key-value 表。

## 反例（禁）

- ❌ 改 query_quota 签名加 extra → 波及 N 调用点
- ❌ 脚本 `sh -c <user string>` → 元字符注入
- ❌ `.output()` 无 cap → OOM
- ❌ 透传父 env（凭证泄漏）
- ❌ 模板未知变量静默空字符串 → 上游空 auth
- ❌ 新建 reqwest::Client → 绕 proxy 递归防护
