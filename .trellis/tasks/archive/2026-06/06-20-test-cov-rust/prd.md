# PRD: Rust 后端覆盖率 ≥80%（阶段一：line）

> 调研依据: `research/plan.md`（2026-06-23）。本 PRD 已锁定主会话决策口径。

## 1. 目标

提升 src-tauri Rust 后端**生产代码 line 覆盖率 ≥80%**（实测基线仅 43.6%），CI gate 用 stable `cargo llvm-cov --fail-under-lines 80`。**分支覆盖 ≥80%（nightly `--branch`）拆为阶段二后续 task**，本轮不卡分支门禁，但记录分支基线供后续。

## 2. 决策口径（已拍板）

| 项 | 决策 |
|---|---|
| 本轮目标 | **line ≥80%（stable 工具链）**。分支 80% 留阶段二 task |
| commands/ 薄壳 | **全纳入分母必须测**（不 ignore commands/）。Tauri `#[command]` 封装需建 State/AppHandle 测试基建逐个覆盖 |
| 覆盖率工具 | `cargo-llvm-cov`（本机已装，无需新工具）+ `llvm-tools-preview` |
| 口径 | `--ignore-filename-regex` 仅排 `test_` + 纯启动胶水（`main.rs`/`startup.rs`/`app_setup.rs`/`shared.rs`/`logging.rs`/`lib.rs`/adapter 薄 mod.rs）。**commands/ 不排除** |

> ⚠️ commands/「全纳入必须测」是本轮最大工作量来源（~2000 region 薄壳几乎全 0%）。需建 Tauri `tauri::State` / `AppHandle` 测试 harness，逐 command 覆盖；薄壳转发类可批量构造 State 冒烟。

## 3. 基线（实测）

| 口径 | Region | Function | Line |
|---|---|---|---|
| 含 test 文件（虚高，勿用） | 60.0% | 49.2% | 58.4% |
| **仅生产代码（真实基准）** | 41.6% | 36.0% | **43.6%** |

命令：`cargo llvm-cov report --summary-only --ignore-filename-regex 'test_|/tests/'`
分支基线（阶段二用）：`rustup run nightly cargo llvm-cov --branch --summary-only --ignore-filename-regex 'test_'`

## 4. 路径纠偏（CLAUDE.md 已过时）

`proxy.rs`/`router.rs`/`db.rs`/`converter.rs` 单文件描述过时，已拆子模块目录：`gateway/proxy/`、`gateway/db/`、`gateway/router/`、`gateway/adapter/converter/` + `adapter/openai/`。已有 in-memory DB harness `gateway/db/test_support.rs`（`test_db()` + `sample_platform/sample_group`）。

## 5. Deliverables

| # | Deliverable | 并行 | 验收 |
|---|---|---|---|
| D0 | 工具链 + CI 基线（串行前置）：`cargo llvm-cov` 配置 + ignore-regex（排 test_/启动胶水，**不排 commands/**）+ `make coverage` 单入口 + stable `--fail-under-lines 80` gate + 记录 nightly 分支基线 | 否 | `cargo llvm-cov --fail-under-lines` 可跑；分支基线数字落档 |
| D1 | adapter 纯函数（最高 ROI）：gemini 四向 / anthropic to+parse_sse / openai parse+sse+completions / converter/response 分支 | ✅ | 各文件 line ≥80%；`cargo test adapter` |
| D2 | router/selection（select_failover/load_balance/platform）+ proxy/count_tokens 纯函数 | ✅ | 两文件 ≥80% |
| D3 | i18n（from_locale + t + 7 语言）+ codex（build/write/cleanup group profile，tempdir） | ✅ | i18n ≥95%、codex ≥80% |
| D4 | db/model_price（list/count/get/upsert/resolve_price 回退链/search/filtered_list）+ estimate(algo+db_ops) + db/group CRUD 边界 | ✅ | model_price ≥80%、estimate ≥80% |
| D5 | quota 响应解析：抽「reqwest 调用 / JSON→余额结构解析」纯函数，喂脱敏 fixture 测；HTTP 壳 mock 或排除 | 依赖小重构 | 解析函数 ≥80% |
| D6 | proxy 编排层：建 mock 上游基建（httpmock/wiremock dev-dep 或本地 axum test server）+ ProxyState（in-memory db），跑 handle_proxy 主路径 + forward/finish/non_success/group_info/headers/passthrough；优先拆 non_success/headers 纯构造函数（免 mock） | 依赖 mock 基建，串行于 D1 | proxy/ 目录整体 ≥80% |
| D7 | **commands/ 全覆盖**（本轮重头）：建 Tauri State/AppHandle 测试 harness；sync_settings/tray_render 拆纯函数测；其余薄壳逐个构造 State 冒烟覆盖转发路径 | 依赖 D0 harness | commands/ 纳入分母后全量 line ≥80% |

并行性：D1/D2/D3/D4 完全独立可并行。D5 需先小重构。D6 依赖 mock 基建。D7 依赖 Tauri 测试 harness（D0 顺带建）。

## 6. 风险

1. **commands/ 全纳入 = 工作量翻倍**：~2000 region 薄壳，需 Tauri State/AppHandle harness。本轮最大不确定性。
2. **proxy 编排无现成 mock 上游**（仓库无 wiremock/httpmock；`proxy/mock.rs` 是产品 feature 非测试 double）→ 引入 dev-dep。
3. **quota/price_sync reqwest 出站**→ 拆纯解析函数，HTTP 壳 mock/排除（属生产代码重构，走 check 防回归）。
4. **CI 计费**（macOS 10×）：PR 跑 stable line gate，nightly branch 周期跑，需缓存 + 分级。
5. 分支口径严于 line（推测低 5~15pt）→ 阶段二达成分支 80% 实际需 line ~88%+，本轮 line 80 是中间里程碑。

## 7. 验证

- `make coverage` stable line ≥80%（含 commands/，仅排 test_/启动胶水）
- `cargo clippy` 零 warning、`cargo test` 全绿
- nightly 分支基线数字落档（阶段二输入）
