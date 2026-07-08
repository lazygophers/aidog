# Research: npm latest version 检测方案（CLI 升级门控）

- **Query**: 如何在 Rust/Tauri 后端检测 npm 包 latest version 以支撑「升级按钮仅确认有新版本才展示」
- **Scope**: mixed（内部代码 + 外部 npm/semver 文档）
- **Date**: 2026-07-08

## 结论速览（TL;DR）

**推荐方案 A（HTTP registry）**：用现有共享 reqwest client GET `https://registry.npmjs.org/<pkg>/latest`，解析 JSON `version` 字段，配合 `semver` crate 比对。**禁用** `npm view` spawn（依赖 npm 可执行文件、跨平台 `.cmd`/`.exe` 后缀差异、慢 5-10x、阻塞 CLI 检测的离线降级目标）。

---

## 内部代码现状

| File Path | Description |
|---|---|
| `src-tauri/src/commands/cli_env.rs` | 现有 CLI 检测。`CliInstallation` (line 21-32) 含 `path/version/runnable/source/is_path_default`，**无 latest/has_update**。`cli_check_versions` (line 284) 同步 spawn `--version`，不触网。 |
| `src/services/api/types/part4.ts` | TS 镜像。`CliInstallation` (line 59-67) 同样无 latest 字段。 |
| `src-tauri/src/gateway/http_client.rs` | 共享 reqwest client builder（`build_http_client` line 33-75）。**注意**：此 builder 强制 `.no_proxy()` / 读 DB 代理设置——CLI 升级检测应**绕过**这套（registry.npmjs.org 是公网直连，不应走 AirDog 自身代理递归），建议单独构一个 `reqwest::Client::new()` 或显式 `.no_proxy()` 的 ephemeral client。 |
| `src-tauri/Cargo.toml` | 已有 `reqwest = "0.12" (features=[...json...])` (line 67)、`serde_json = "1"` (line 50)、`tokio` (line 56)。**未引** `semver` crate，需新增依赖（或手写 X.Y.Z 比较器）。 |

### 现有 `OnceLock` 模式（缓存参考）

`src-tauri/src/gateway/middleware/mod.rs:242-244`、`src-tauri/src/gateway/peak_hours.rs`（CLAUDE.md 提及 `OnceLock` 解析）已用 `std::sync::OnceLock<T>` 做进程级单次缓存。`semver`/`once_cell` crate 无需引入——标准库 `OnceLock` + `tokio::sync::RwLock` 即可做带 TTL 的缓存。

---

## 方案对比

### 方案 A：HTTP registry API（推荐）

**端点三选一**（实测 2026-07-08，scoped package `@openai/codex`）：

| 端点 | 返回 | 实测 size | 实测耗时 | 评价 |
|---|---|---|---|---|
| `GET /<pkg>` (full packument) | 全部版本历史 | 9.75 MB | 5.27 s | **禁用**：每次拉全量版本历史，浪费严重 |
| `GET /<pkg>/latest` (metadata of latest) | 单版本元数据 JSON，含 `version`/`bin`/`engines`/`dist` | 3.2 KB | 1.9 s | 推荐：体积小、字段全 |
| `GET -/package/<pkg>/dist-tags` | `{"latest":"x","beta":"y",...}` | 603 B | 1.5 s | 最小，但只有 tag→version 映射 |

URL 中 scoped package 的 `@` 和 `/` **无需百分号编码**（npm registry 接受原样字符，实测 200 OK）。引用：
- npm Registry API doc: <https://github.com/npm/registry/blob/main/docs/REGISTRY-API.md#get-v1package-version> — `GET /<package>/<version>` 返回版本元数据。
- npm CLI `npm view` 即基于同一 registry（<https://docs.npmjs.com/cli/v10/commands/npm-view>）。

**优点**
- 无需 `npm` 可执行文件存在（与 CLI 检测的「npm 未装 / PATH 损坏」降级场景正交）。
- 直接复用项目既有 reqwest 0.12 + serde_json。
- 单次请求 ~2s 内完成，JSON 字段固定。
- 网络失败可 `Result::Err` 自然降级到 `has_update=None`（UI 隐藏按钮）。

**缺点**
- 需要外网访问；离线场景必须降级。
- 用户处于内网镜像（如 npmmirror）时 `registry.npmjs.org` 可能不可达 —— **不读 `.npmrc`**（保持简单；推测：内网用户能用即用，不可用就隐藏按钮，可接受）。

### 方案 B：spawn `npm view <pkg> version`

**命令**：`npm view @openai/codex version` → stdout 一行版本号（实测等价 registry `/latest` 的 `version` 字段）。

**跨平台差异**（必须处理）：
- Windows: `Command::new("npm")` 在 Rust 默认不查 PATHEXT，需 `npm.cmd`（参考已存在的 `cli_env.rs:333` `Command::new("npm")` —— 该处依赖 Tauri 启动期 `gateway::skills::ensure_runtime_path` 把 npm 所在目录并入登录 shell PATH，并且 Windows 上 `Command::new` 在 std::process 层**不会**自动追加 `.cmd`/`.exe`；项目里 `cli_install`/`cli_upgrade` 的 `Command::new("npm")` 实际是隐患，但既存代码已用此模式）。
- POSIX: `npm` 直接可执行。
- 依赖 npm 已安装且在 PATH；与「检测 CLI 是否需要升级」前置条件（npm 可能就坏了）冲突。

**性能**：spawn npm 启动 node 进程，冷启动 500ms-2s，比 HTTP 慢且不可控。

**结论**：**不推荐**。引入 npm-binary 依赖、跨平台后缀坑、慢，且与 CLI 检测降级目标冲突。

---

## 版本比较

### 选项 1：`semver` crate（推荐）

- crate: <https://crates.io/crates/semver>（官方，cargo 自身使用，1.0+ 稳定）。
- doc: <https://docs.rs/semver> — `Version::parse("1.2.3") -> Result<Version, Error>`，`Version` 实现 `Ord`/`PartialOrd` 直接比较。
- 处理 prerelease / build metadata 符合 semver 规范。
- 实测两个目标包的 latest 都是纯 X.Y.Z（`@openai/codex@0.143.0`、`@anthropic-ai/claude-code@2.1.204`），但 codex 历史 beta tag 有 `0.1.2505172116` 这种 4 段 patch（`extract_version` in `cli_env.rs:69` 已兼容），`semver` crate 严格模式下解析 4 段会失败 —— **需在 parse 失败时回退到「字符串相等才视为同版」**或截断到 X.Y.Z。

Cargo.toml 增量：
```toml
semver = "1"
```

### 选项 2：手写 X.Y.Z 比较

`cli_env.rs:69 extract_version` 已有版本号字符串抽取逻辑。手写比较器（split `.` → 比 major/minor/patch 数值）可行，但需处理 prerelease / 4 段 patch / 非数字段。**不推荐**：增加测试面，与 `semver` crate 已验证的实现相比无收益。

**推荐**：方案 A（HTTP） + 选项 1（semver crate），并在 semver parse 失败时降级为 `has_update = None`（保守隐藏按钮，不假阳/假阴）。

---

## 离线 / 网络失败降级

| 场景 | 行为 |
|---|---|
| reqwest 超时 / DNS 失败 / 非 2xx | `Err` → `latest_version=None, has_update=None` |
| JSON parse 失败 / `version` 字段缺失 | 同上 |
| semver parse 失败（local 或 latest 非法） | 同上 |
| local.version=None（CLI 未装 / broken） | 直接 `has_update=None`（不查 registry；UI 不显示升级按钮，因 `installed=false`） |

**关键**：`has_update` 必须是 `Option<bool>` 而非 `bool`，三态：`Some(true)` 显示按钮 / `Some(false)` 已最新不显示 / `None` 未知不显示（保守）。

---

## 请求频率 / 缓存

**问题**：每次前端调 `cli_check_versions`（每次打开设置页或刷新）都打 registry → 浪费 + 可能被 rate limit。

**推荐**：进程内 TTL 缓存，借鉴项目现有 `OnceLock` 模式（`gateway/middleware/mod.rs:242`、CLAUDE.md 提及 `peak_hours.rs`）。

参数建议：
- **TTL**: 1 小时（CLI 发版频率最高约每日，1h 足够新鲜；推测）。
- **存储**：`tokio::sync::RwLock<HashMap<String, (Instant, Option<String>)>>`，key=包名，value=(过期时刻, latest version 或 None)。
- **并发去重**：用 `tokio::sync::Mutex` 或 `tokio::sync::OnceCell` 包住 in-flight future，避免并点设置页时双重请求（推测：当前 UI 单实例不太会并发，可选优化）。
- **强制刷新**：升级按钮点击后调 `cli_upgrade` 成功后清缓存，下次 `cli_check_versions` 重拉确认。

**禁** 落盘缓存（与 settings.json / proxy_log 同库可，但 CLI version 非持久必要，进程重启重拉一次成本可接受，保持简单）。

---

## Rust 实现草图（API 形状，非最终码）

```rust
// 新文件 src-tauri/src/commands/cli_latest.rs（或并入 cli_env.rs）
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::RwLock;
use once_cell::sync::Lazy; // 或 std::sync::OnceLock<HashMap> + 手动锁

const REGISTRY_BASE: &str = "https://registry.npmjs.org";
const TTL: Duration = Duration::from_secs(3600);

/// 进程内 latest-version 缓存。
static CACHE: Lazy<RwLock<HashMap<String, (Instant, Option<String>)>>>
    = Lazy::new(|| RwLock::new(HashMap::new()));

/// 单独的 client：禁 env proxy / 不读 DB 代理设置（避免 AirDog 自递归）。
fn registry_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()                       // 关键：参见 http_client.rs:54-58 注释
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// 拉 latest version；网络/解析失败返 None（不区分原因，UI 统一降级）。
async fn fetch_latest(pkg: &str) -> Option<String> {
    let url = format!("{}/{}/latest", REGISTRY_BASE, pkg);
    let resp = registry_client().get(&url).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;
    v.get("version")?.as_str().map(|s| s.to_string())
}

/// 带缓存的入口；force=true 跳过缓存（升级后重拉）。
pub async fn latest_version(pkg: &str, force: bool) -> Option<String> {
    if !force {
        if let Some((exp, v)) = CACHE.read().await.get(pkg) {
            if *exp > Instant::now() {
                return v.clone();
            }
        }
    }
    let v = fetch_latest(pkg).await;
    CACHE.write().await.insert(pkg.to_string(), (Instant::now() + TTL, v.clone()));
    v
}

/// 比对：local vs latest。任一 None 或 parse 失败 → None（保守）。
pub fn has_update(local: &Option<String>, latest: &Option<String>) -> Option<bool> {
    let l = semver::Version::parse(local.as_ref()?).ok()?;
    let r = semver::Version::parse(latest.as_ref()?).ok()?;
    Some(r > l)
}
```

`cli_check_versions` 内调用（仍保持同步签名 → 改 async，或拆出独立 `cli_check_updates` command）：

```rust
// 推荐：拆独立 command，避免把现有同步 cli_check_versions 改成 async（破坏前端调用约定）
#[tauri::command]
pub async fn cli_check_updates() -> Vec<UpdateInfo> {
    let mut out = Vec::new();
    for (tool, pkg) in [("claude", "@anthropic-ai/claude-code"), ("codex", "@openai/codex")] {
        let status = /* 复用 cli_check_versions 的 probe_version 结果，或前端组装时合并 */;
        let local = status.version.clone();
        let latest = latest_version(pkg, false).await;
        out.push(UpdateInfo {
            tool: tool.to_string(),
            latest_version: latest.clone(),
            has_update: has_update(&local, &latest),
        });
    }
    out
}
```

---

## CliInstallation / CliToolStatus 字段增量建议

**推荐位置**：加在 `CliToolStatus`（每个工具一条），而非 `CliInstallation`（每处安装一条 —— latest 是包级别概念，不是 per-binary；`CliConflict` 不需要，只关心版本分歧的本地枚举）。

```rust
// src-tauri/src/commands/cli_env.rs
#[derive(serde::Serialize, Clone)]
pub struct CliToolStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub broken: bool,
    pub conflict: bool,
    // 新增 ↓
    /// registry latest version；网络/解析失败为 None。
    pub latest_version: Option<String>,
    /// 是否有更新：Some(true)=有新版, Some(false)=最新, None=未知（不显按钮）。
    pub has_update: Option<bool>,
}
```

TS 镜像（`src/services/api/types/part4.ts` `CliToolStatus` line 70-79）同步加：

```ts
export interface CliToolStatus {
  // ...既有...
  /** registry latest version；网络/解析失败为 null。 */
  latest_version: string | null;
  /** 是否有更新：true=有新版, false=最新, null=未知（不显按钮）。 */
  has_update: boolean | null;
}
```

**字段命名**：保持 snake_case 对齐既有契约（CLAUDE.md 注释 `与后端 commands::cli_env 数据结构对齐，snake_case`）。**禁** 用 `updateAvailable` / `needsUpgrade` 等 camelCase 或近义异名。

**为什么 has_update 用 Option<bool> 而非 bool**：三态语义（未知/最新/有新版）必须区分，否则离线时若默认 `false` 会误判「已最新」隐藏按钮、用户错过升级；若默认 `true` 会误显按钮、点击后无操作。`Option<bool>` + UI 仅 `=== true` 才显示是唯一自洽解。

---

## 外部参考

- [npm Registry API](https://github.com/npm/registry/blob/main/docs/REGISTRY-API.md) — 端点规范（`GET /<package>/<version>` 返回 metadata）。
- [npm view CLI](https://docs.npmjs.com/cli/v10/commands/npm-view) — `npm view` 内部基于同一 registry，验证 HTTP 方案等价性。
- [reqwest 0.12 docs](https://docs.rs/reqwest/0.12) — `Client::builder().no_proxy().timeout()` API。
- [semver crate](https://docs.rs/semver) — `Version::parse` + `Ord` 比较，cargo 自用。
- [Scoped package name 规范](https://docs.npmjs.com/cli/v10/configuring-npm/package-json#name) — `@scope/pkg` URL 中无需编码（实测 200 OK）。

## Caveats / Not Found

- **推测**：TTL=1h 是平衡新鲜度与请求频率的建议值，无硬性文档约束；可按用户反馈调整。
- **推测**：内网 npmmirror 镜像用户场景下 `registry.npmjs.org` 不可达 —— 未读 `.npmrc` 解析用户镜像 URL；若需支持，后续可加 `~/.npmrc` parser（非本任务范围）。
- **未实测**：`semver::Version::parse` 对 codex 历史 `0.1.2505172116`（4 段）的实际行为 —— 标准 semver 仅允许 major.minor.patch + prerelease，4 段 numeric patch 会 parse 失败；当前 latest `0.143.0` / `2.1.204` 均 3 段，但历史版本可能 4 段，parse 失败时降级 `has_update=None` 是安全兜底。
- **不引入** `once_cell` crate：项目已全用 `std::sync::OnceLock`（middleware/peak_hours 同模式）；若需 `Lazy` 初始化 `RwLock<HashMap>`，可用 `OnceLock<RwLock<...>>` 替代，避免新增依赖。
