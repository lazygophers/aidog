# Research: Rust 后端覆盖率 ≥80% 规划方案

- **Query**: 为 `06-20-test-cov-rust`（Rust 后端分支覆盖率≥80%）做规划调研，产出可落 PRD 方案
- **Scope**: internal（只读 src-tauri/）+ 工具链评估
- **Date**: 2026-06-23

---

## 0. 关键纠偏（CLAUDE.md 已过时）

CLAUDE.md 里 `gateway/proxy.rs` / `router.rs` / `db.rs` / `converter.rs` 单文件描述**已过时**。代码已重构为子模块目录：
- `gateway/proxy/`（19 个产物文件 + 9 个 test_*.rs）
- `gateway/db/`（拆 group/platform/model_price/proxy_log/query_stats/... + test_support.rs）
- `gateway/router/`（candidates/ordering/selection/model_mapping/mod）
- `gateway/adapter/converter/`（request/response）+ `adapter/openai/`（parse/request/response/sse）

规模：252 个 .rs 文件，40,240 LOC；其中 73 个 `test_*.rs`。已有 in-memory DB 测试 harness（`gateway/db/test_support.rs`：`test_db()` 建 `:memory:` 库 + `sample_platform/sample_group`）。

---

## 1. 当前覆盖率基线（已实测）

**工具**：`cargo-llvm-cov`（已装 `/Users/luoxin/.cargo/bin/cargo-llvm-cov`）+ `llvm-tools-preview`（本次调研已 `rustup component add`）。

**命令**：
```bash
cd src-tauri
cargo llvm-cov --summary-only                                  # 含 test_*.rs（虚高）
cargo llvm-cov report --summary-only --ignore-filename-regex 'test_|/tests/'   # 仅生产代码（真实）
```

**基线数字**：

| 口径 | Region | Function | Line |
|---|---|---|---|
| 含 test 文件（虚高） | 60.03% | 49.15% | 58.40% |
| **仅生产代码（真实基线）** | **41.63%** | **35.98%** | **43.60%** |

> 注意：`test_*.rs` 自身 100% 覆盖会把总数拉高近 17 个百分点。PRD 必须以 **`--ignore-filename-regex 'test_'` 口径**（生产代码 ~43.6% line）为基准，否则会误判已接近目标。

### 分支覆盖（Branch）说明 — 测得出但需 nightly

llvm-cov 默认报表 Branches 列全为 `0/0 -`（stable 工具链不产出分支计数）。分支覆盖需：
```bash
rustup run nightly cargo llvm-cov --branch --summary-only --ignore-filename-regex 'test_'
```
（nightly 工具链本机已装：`nightly-aarch64-apple-darwin`；`--branch` 为 unstable flag，仅 nightly 可用。）

**推测**：分支覆盖数值通常低于 line 覆盖 5~15 个百分点，故达成「分支≥80%」实际门槛比「line≥80%」更高。PRD 须明确：CI gate 用 nightly `--branch`，目标 = **分支覆盖 ≥80%（生产代码口径）**。

---

## 2. 覆盖缺口清单（按热路径优先级排序）

### P0 — 热路径，0% 或极低，纯函数易补（最高 ROI）

| 文件 | Line Cover | 性质 | 缺口 |
|---|---|---|---|
| `gateway/adapter/gemini.rs` | 6.51% | **纯函数** to_gemini/from_gemini/parse_gemini_sse/to_gemini_sse | 协议转换四向全未测 |
| `gateway/adapter/anthropic.rs` | 17.48% | **纯函数** to_anthropic/parse_anthropic_sse | 入站转换+SSE 解析 |
| `gateway/adapter/openai/parse.rs` | 0% | **纯函数** from_openai | OpenAI 入站解析 |
| `gateway/adapter/openai/sse.rs` | 28.97% | **纯函数** SSE 转换 | 流式事件转换 |
| `gateway/adapter/openai_completions.rs` | 22.58% | **纯函数** | completions 适配 |
| `gateway/adapter/converter/response.rs` | 69.12% | **纯函数** | 非流式响应转换分支 |
| `gateway/router/selection.rs` | 0% | DB-light/纯 | select_failover/select_load_balance/select_platform |
| `gateway/proxy/count_tokens.rs` | 16.67% | 纯+async | is_count_tokens_endpoint/estimate_input_tokens/collect_text 纯函数未测 |

### P1 — 热路径，proxy 编排层 0%（需 mock 上游或拆纯函数）

| 文件 | Line Cover | 性质 | 缺口 |
|---|---|---|---|
| `gateway/proxy/handler.rs` | 0% | Axum handler `handle_proxy` | 主入口编排，需 ProxyState + mock 上游 |
| `gateway/proxy/forward.rs` | 0% | async 转发（490 region） | 转发链路 |
| `gateway/proxy/finish.rs` | 0% | async 收尾 | 日志/usage 落库收尾 |
| `gateway/proxy/group_info.rs` | 0% | `/api/group-info` handler | 鉴权+组信息 |
| `gateway/proxy/non_success.rs` | 0% | 错误响应构造 | 失败路径（含纯构造函数可拆） |
| `gateway/proxy/notify.rs` | 0% | 通知触发 | |
| `gateway/proxy/mock.rs` | 0% | mock 平台响应（内部 feature） | handle_mock |
| `gateway/proxy/headers.rs` | 32.14% | 头处理（部分纯函数） | header strip/构造分支 |
| `gateway/proxy/passthrough.rs` | 12.77% | 同协议旁路 | 旁路改写分支 |
| `gateway/proxy/responses.rs` | 5.88% | /v1/responses 处理 | |

### P2 — DB 层，已有 harness 可直接补

| 文件 | Line Cover | 性质 | 缺口 |
|---|---|---|---|
| `gateway/db/model_price.rs` | 21.32% | DB（in-memory 可测） | list/count/get/upsert/resolve_price/search/filtered_list |
| `gateway/db/group.rs` | 59.00% | DB | 部分 CRUD/边界 |
| `gateway/db/platform_lifecycle.rs` | 80.36% | DB | 接近达标，补边界 |

### P3 — 估算/配额/工具/i18n

| 文件 | Line Cover | 性质 | 缺口 |
|---|---|---|---|
| `gateway/estimate/algo.rs` | 63.33% | 纯函数 | 估算分支 |
| `gateway/estimate/db_ops.rs` | 54.19% | DB | est_cost 落库 |
| `gateway/quota/balance.rs` | 0% | async + reqwest | 5 个 balance 查询（响应解析可拆纯函数） |
| `gateway/quota/coding_plan.rs` | 0% | async + reqwest | kimi/zhipu/minimax coding plan |
| `gateway/quota/http.rs` / `newapi.rs` / `mod.rs` | 0% | async + reqwest | HTTP 出站 |
| `gateway/i18n.rs` | 0% | **纯函数** from_locale/t/t_*(7语言) | 易补满分 |
| `gateway/codex.rs` | 0% | 纯+filesystem | build_group_profile_toml/write/cleanup（用 tempdir） |
| `gateway/price_sync.rs` | 12.33% | async + reqwest | |
| `gateway/skills/*` | bulk 3%/cache 0%/npx 8%/ops 36%/env 37% | 进程/IO 重 | 部分可测 |

### P4 — commands/ 层（Tauri `#[command]` 薄封装，几乎全 0%）

`commands/*.rs` 25 个文件多数 0%（about/app_log/backup/fs_autocomplete/group/hooks/mcp/middleware/model_fetch/model_test/notification/platform/popover/price/proxy/proxy_log/quota/scheduling/settings/skills/stats/tray/tray_render）。
- **性质**：多为 `tauri::State` + 转发到 gateway 已测逻辑的薄壳，单测需构造 AppHandle/State，ROI 低。
- **例外**：`commands/sync_settings.rs`（8.83%，385 region，含真实逻辑）、`commands/tray_render.rs`（0%，491 region，渲染逻辑可拆纯函数测）值得补。

### 非目标（建议排除覆盖统计）

`main.rs`/`startup.rs`/`app_setup.rs`/`shared.rs`/`logging.rs`/`lib.rs`/各 adapter mod.rs 薄壳 + Tauri `#[command]` 入口 — 这些是 Tauri/启动胶水代码，建议 `--ignore-filename-regex` 排除或标 `#[cfg_attr(coverage,...)]`，避免拉低分母。

---

## 3. MVP 任务拆解（按模块分 deliverable）

> 总目标：生产代码分支覆盖 ≥80%。当前 line ~43.6%，缺口主要集中在 adapter 纯函数 + proxy 编排 + quota HTTP + commands 薄壳。
> 策略：**先吃纯函数（P0/P3 易测），再啃 proxy 编排（需 mock 基建），commands 薄壳排除或最小覆盖**。

### D0 — 工具链与 CI 基线（串行前置，阻塞其余）
- 落 `cargo llvm-cov` 配置；定 ignore-regex（排除 test_/main/startup/启动胶水/Tauri command 入口）
- 确认 nightly `--branch` 可跑，记录基线分支数字
- 产出 `make coverage` / CI 脚本 + 阈值 gate
- **验证**：`cargo llvm-cov --branch` 出数；CI 脚本本地跑通

### D1 — adapter 纯函数转换（P0，**可并行**，最高 ROI）
- gemini.rs 四向转换 fixture 测（to/from/sse 双向）
- anthropic.rs to_anthropic + parse_anthropic_sse
- openai/parse.rs from_openai + openai/sse.rs + openai_completions.rs
- converter/response.rs 非流式响应分支补全
- **验证**：上述文件各自 line ≥80%；`cargo test adapter`

### D2 — router/selection + count_tokens 纯函数（P0，**可并行**）
- router/selection.rs：select_failover/select_load_balance/select_platform（用 sample GroupPlatformDetail）
- proxy/count_tokens.rs：is_count_tokens_endpoint/estimate_input_tokens/collect_text 纯函数
- **验证**：两文件 ≥80%；`cargo test router selection count_tokens`

### D3 — i18n + codex 纯/FS 函数（P3，**可并行**，易满分）
- i18n.rs：from_locale + t + 7 语言 t_* 全枚举
- codex.rs：build_group_profile_toml（断言 TOML）/write_group_profile/cleanup_group_profiles（tempdir 隔离）
- **验证**：i18n ≥95%、codex ≥80%

### D4 — db/model_price + estimate（P2，依赖 test_support harness，**可与 D1-D3 并行**）
- model_price.rs：list/count/get/upsert/resolve_price 回退链/search/filtered_list（in-memory db）
- estimate/algo.rs + db_ops.rs 补分支
- db/group.rs 补 CRUD 边界到 ≥80%
- **验证**：model_price ≥80%、estimate ≥80%；`cargo test db estimate`

### D5 — quota 响应解析（P3，**依赖重构**：建议先抽纯解析函数）
- 把 balance.rs / coding_plan.rs / newapi.rs 的「reqwest 调用」与「JSON 响应→余额结构」解析拆分
- 对纯解析函数喂真实响应 fixture（脱敏）做单测
- HTTP 出站层用 mock server（见 §4 风险）或标记排除
- **验证**：解析函数 ≥80%；HTTP 壳可排除或 mock

### D6 — proxy 编排层（P1，**依赖 mock 上游基建**，串行在 D1 之后）
- 建 mock 上游 HTTP 基建（httpmock / wiremock dev-dep 或本地 axum test server）
- 构造 ProxyState（in-memory db + mock 平台）跑通 handle_proxy 主路径
- forward/finish/non_success/group_info/headers/passthrough 编排分支
- 优先拆 non_success.rs / headers.rs 中的纯构造函数单测（不需 mock）
- **验证**：proxy/ 目录整体 ≥80%；`cargo test proxy`

### D7 — commands 取舍（P4，最后）
- sync_settings.rs + tray_render.rs 拆纯函数补测
- 其余薄壳 command：评估排除 vs 最小冒烟，凑齐总分支 ≥80%
- **验证**：全量 `cargo llvm-cov --branch` 总分支 ≥80%

**并行性**：D1/D2/D3/D4 完全独立可并行（不同模块、各自 test_*.rs 文件）。D5 需先小重构。D6 依赖 mock 基建（D0 可顺带建）。D7 收尾。

---

## 4. 推荐覆盖率工具与 CI 集成

### 工具：cargo-llvm-cov（本机已装，无需引入新依赖）
- 优于 tarpaulin：基于 LLVM source-based coverage，精度高、支持分支、与 rustc 同源
- `llvm-tools-preview` 已加；nightly 已装可跑 `--branch`

### 推荐 CI 配置
```yaml
# 行覆盖 gate（stable，快）
cargo llvm-cov --ignore-filename-regex 'test_|main.rs|startup.rs|app_setup.rs|/commands/' \
  --fail-under-lines 80

# 分支覆盖 gate（nightly，慢但是任务真实目标）
rustup run nightly cargo llvm-cov --branch \
  --ignore-filename-regex 'test_|main.rs|startup.rs' \
  --summary-only
# （--fail-under-branches 待 cargo-llvm-cov 版本支持；否则脚本解析 TOTAL 行 branch% 做 gate）
```
- 产出 lcov：`cargo llvm-cov --lcov --output-path lcov.info` → 上传 Codecov/Coveralls
- 建议 `src-tauri/Makefile` 或 `cargo xtask` 收口为 `make coverage` 单入口（对齐项目「单入口」惯例）
- release.yml 计费敏感（macOS 10×），覆盖率 job 建议独立、用 `--no-clean`/缓存，或仅 PR 跑 stable line gate、nightly branch 周期性跑

### ignore-regex 必排清单
`test_` / `main.rs` / `startup.rs` / `app_setup.rs` / `shared.rs` / `logging.rs` / `lib.rs` / adapter 薄 mod.rs / Tauri `#[command]` 入口胶水。

---

## 5. 风险 / 难测点

1. **分支口径 > line 口径**：任务写「分支≥80%」，比 line≥80% 难。必须用 nightly `--branch` 量，且预留额外补测（分支通常低 line 5~15pt）。**推测**：达成分支 80% 实际需 line ~88%+。
2. **Axum handler（proxy/handler.rs 等 0%）**：`handle_proxy(AxumState<Arc<ProxyState>>, Request)` 需构造完整 ProxyState（db + http_client + 配置）。无现成 mock 上游基建（仓库无 wiremock/httpmock，`proxy/mock.rs` 是产品内 mock 平台 feature，非测试 double）。**解法**：引入 httpmock/wiremock dev-dep 或起本地 axum test server 当上游。
3. **async + reqwest 外部 HTTP（quota/price_sync）**：直接打真实平台 API 不可测且不稳定。**解法**：抽「响应解析纯函数」单独测（fixture），HTTP 壳层 mock 或排除。
4. **进程/IO 重模块（skills/* npx、scripts、import_export）**：spawn npx / 文件系统 / 外部命令，单测成本高。**解法**：tempdir 隔离 FS 部分；spawn 部分排除或抽接口。
5. **commands/ 薄壳大面积 0%（~2000+ region）**：构造 Tauri State/AppHandle 成本高、ROI 低，但占分母。**解法**：ignore-regex 排除 + 把含真实逻辑的 sync_settings/tray_render 拆纯函数测。
6. **覆盖率 CI 计费**：项目 release CI 对 GitHub Actions 计费敏感（macOS 10×）。覆盖率全量编译慢，需缓存 + 分级（PR 跑 line gate，branch 周期跑）。
7. **重构风险**：D5（quota）拆纯函数属生产代码改动，本调研禁改源码 — 须在实现阶段做，且走 check 防回归。

---

## Caveats / Not Found

- 未实测 nightly `--branch` 的具体分支百分比（编译耗时，且本任务为只读调研）。基线分支数字需实现阶段 D0 首次产出。已确认工具链就绪（nightly + --branch flag 存在）。
- 生产代码 line 基线 43.6% 为实测可信值；分支基线为「推测低于 line」。
- 各文件函数清单基于 grep，复杂泛型/宏展开函数计数可能与 llvm-cov function 数略有出入。
