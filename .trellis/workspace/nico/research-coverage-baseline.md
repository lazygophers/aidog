# Research: 测试覆盖率基线 (Coverage Baseline)

- **Query**: 跑 Rust + 前端当前测试覆盖率基线，出各模块覆盖率 % + 缺口清单，为 coverage 80% task 提供数据（只读测量）
- **Scope**: 内部测量（cargo-llvm-cov + vitest coverage）
- **Date**: 2026-07-01
- **Task**: .trellis/tasks/06-20-test-coverage-80（parent；children: 06-20-test-cov-rust, 06-20-test-cov-frontend）
- **Mode**: 只读，未改任何源码/测试

---

## TL;DR（结论先放）

| 维度 | 总 line% | 达 80%? | 主要缺口 |
|---|---|---|---|
| **Rust** (`cargo llvm-cov`) | **87.47%** | ✅ 已达标 | commands 层 (34%)、root 文件 (20%: app_setup/startup/main/logging/shared) |
| **前端** (`vitest --coverage`) | **94.99%** (line) / 92.29% stmt | ✅ 已达标 | 但**只测了 7 个文件**，pages/settings/platforms 共 36 组件**零测试**（覆盖率高的假象，因 v8 只统计被 import 进测试的文件） |

> **关键发现**: 双端 line coverage **均已 ≥ 80%**，task 06-20 名义目标已达成。但**前端覆盖率数字具有误导性**：vitest/v8 默认只统计被测试 import 的文件，未 import 的 page/component **不计入分母**。真实"有测试覆盖"的前端文件 = 7/94（7.4%）。

---

## 1. Rust 覆盖率

### 1.1 命令与结果

```bash
cd /Users/luoxin/persons/lyxamour/aidog/src-tauri && cargo llvm-cov --summary-only
```

- 编译耗时: ~1m38s（debug + coverage cfg）
- 测试结果: **1092 passed; 0 failed; 4 ignored**（lib unittests；main.rs 无 test）
- Exit code: 0
- 工具: cargo-llvm-cov（/Users/luoxin/.cargo/bin/cargo-llvm-cov），llvm-tools 已装

### 1.2 总体覆盖率（TOTAL 行）

| Metric | Total | Missed | Cover % |
|---|---|---|---|
| Regions | 63,978 | 8,078 | **87.37%** |
| Functions | 4,851 | 972 | 79.96% |
| **Lines** | **35,566** | **4,456** | **87.47%** ✅ |
| Branches | (0 measured) | — | — |

### 1.3 各模块覆盖率（source 文件加权，按 missed lines 降序 = 缺口大→小）

| 模块 | total lines | missed | line% | 说明 |
|---|---|---|---|---|
| `gateway/db` | 5,370 | 500 | **90.69%** | ✅ 最大模块，覆盖最好 |
| `gateway/proxy` | 2,959 | 672 | **77.29%** | ⚠️ <80，passthrough/forward/handler 是缺口 |
| `gateway/import_export` | 2,278 | 420 | **81.56%** | apply/mod.rs 缺口大 |
| `gateway/adapter` | 1,871 | 52 | **97.22%** | ✅ 协议转换覆盖极好 |
| `gateway` (顶层散文件) | 1,498 | 155 | 89.65% | codex/claude_integration/price_sync |
| `gateway/skills` | 1,048 | 280 | **73.28%** | ⚠️ <80，env/bulk/cache/ops 缺口 |
| `gateway/mcp` | 746 | 100 | 86.60% | domain.rs 是缺口 |
| `gateway/quota` | 694 | 185 | **73.34%** | ⚠️ <80，balance/newapi/coding_plan 缺口 |
| `gateway/models` | 584 | 35 | 94.01% | ✅ |
| `gateway/middleware` | 478 | 54 | 88.70% | |
| `gateway/estimate` | 337 | 29 | 91.39% | ✅ |
| `gateway/router` | 332 | 34 | 89.76% | |
| `gateway/notification` | 306 | 53 | 82.68% | tts.rs 是大缺口 (49%) |
| `gateway/backup` | 191 | 47 | **75.39%** | ⚠️ scheduler.rs 缺口 |
| `gateway/hooks` | 137 | 7 | 94.89% | ✅ |
| `commands` (command 壳) | 1,956 | 1,284 | **34.36%** | ⚠️⚠️ 最大缺口区（见 1.4） |
| `<root>` (app_setup/main/startup/logging/shared) | 536 | 431 | **19.59%** | ⚠️⚠️ 几乎全无测试 |

> ⚠️ = line% < 80 的模块。commands 与 root 虽 line% 低，但 commands 多为薄壳（Tauri command 注册 + 调 db 层），db 层已 90% 覆盖，**重复测 commands 性价比低**。

### 1.4 缺口清单（source 文件，line% < 80，按 missed lines 降序）

共 **59 个 source 文件** line% < 80，合计 **3,260 missed lines**（占全部 missed 4,456 的 73%）。

#### 0% 覆盖（完全无测试，10 文件）

| File | lines | 备注 |
|---|---|---|
| `app_setup.rs` | 240 | 应用初始化（Tauri setup hook，难单测） |
| `commands/tray_render.rs` | 215 | tray 渲染命令 |
| `commands/tray.rs` | 132 | tray 命令壳 |
| `commands/skills.rs` | 97 | skills 命令壳 |
| `commands/model_test.rs` | 76 | model_test 命令壳 |
| `commands/proxy.rs` | 73 | proxy 命令壳 |
| `commands/model_fetch.rs` | 63 | |
| `commands/quota.rs` | 52 | |
| `startup.rs` | 19 | 启动流程 |
| `main.rs` | 3 | 入口（天然难测） |

#### 低覆盖但 >0%（按 missed lines 排序 TOP，节选）

| File | total | missed | line% |
|---|---|---|---|
| `gateway/proxy/passthrough.rs` | 255 | 166 | 34.9% |
| `gateway/import_export/apply/mod.rs` | 437 | 124 | 71.6% |
| `gateway/proxy/forward.rs` | 331 | 95 | 71.3% |
| `logging.rs` | 145 | 94 | 35.2% |
| `commands/backup.rs` | 102 | 87 | 14.7% |
| `commands/sync_settings.rs` | 249 | 78 | 68.7% |
| `commands/group.rs` | 95 | 77 | 18.9% |
| `shared.rs` | 129 | 75 | 41.9% |
| `gateway/mcp/domain.rs` | 333 | 73 | 78.1% |
| `gateway/db/stats_agg.rs` | 270 | 65 | 75.9% |
| `gateway/proxy/handler.rs` | 310 | 64 | 79.3% |
| `gateway/import_export/skills_sync.rs` | 229 | 62 | 72.9% |
| `gateway/quota/coding_plan.rs` | 269 | 61 | 77.3% |
| `gateway/quota/balance.rs` | 137 | 59 | 56.9% |
| `gateway/proxy/group_info.rs` | 151 | 58 | 61.6% |
| `gateway/skills/env.rs` | 87 | 55 | 36.8% |
| `gateway/proxy/mod.rs` | 75 | 54 | 28.0% |
| `commands/hooks.rs` | 126 | 53 | 57.9% |
| `commands/platform.rs` | 108 | 49 | 54.6% |
| `gateway/quota/newapi.rs` | 116 | 48 | 58.6% |

（完整 59 文件清单见 `/tmp/llvm-cov-out.txt` 解析；测试文件 `test_*.rs` 不计入此列表——它们的"0%"是因测试函数体本身不被插桩，不代表未运行）

### 1.5 测试文件分布

- 168 source files / 113 test files = 281 .rs（test_*.rs 模式集中放，#[cfg(test)] 隔离）
- 测试最多的文件: `gateway/proxy/test_headers.rs` (45)、`test_retry.rs` (33)、`test_integration.rs` (31)
- 命令测试模式: `commands/test_<name>.rs` 配对 `commands/<name>.rs`（多数已配对，tray/skills/model_test/proxy/quota/model_fetch 缺配对）

---

## 2. 前端覆盖率

### 2.1 命令与结果

```bash
yarn test:cov   # = vitest run --coverage (v8 provider)
```

- 测试结果: **16 test files, 186 tests passed**, duration 3.31s
- 无 vitest.config（默认配置：扫 `**/*.test.*`，coverage provider v8）

### 2.2 总体覆盖率（v8）

| Metric | % | covered/total |
|---|---|---|
| Statements | **92.29%** | 839/909 |
| Branches | 83.08% | 511/615 |
| Functions | 95.47% | 253/265 |
| **Lines** | **94.99%** | 740/779 |

### 2.3 已测文件覆盖率（仅 7 个文件被统计）

| File | Stmts% | Branch% | Funcs% | Lines% | Uncovered |
|---|---|---|---|---|---|
| `components/shared/BalanceBar.tsx` | 91.66 | 95.65 | 100 | 100 | L25 |
| `components/shared/CostTrendChart.tsx` | 100 | 80 | 100 | 100 | L34,48 |
| `components/shared/TestResultBody.tsx` | 88.46 | 83.09 | 60 | 87.3 | L23,127-151 |
| `components/shared/usageColor.ts` | 97.95 | 97.56 | 100 | 100 | L68 |
| `services/api.ts` | 96.16 | 88.5 | 95.8 | 97.62 | L510-522,1598-1599 |
| `utils/ccswitchMatch.ts` | 94.54 | 87.27 | 100 | 94.33 | L81,186,218 |
| `utils/formatters.ts` | **40.00** | 40.42 | 77.77 | **38.23** | L77-108 |
| `utils/pinyin.ts` | 97.22 | 88.88 | 100 | 100 | L20,73 |
| `utils/platformPaste.ts` | 94.04 | 79.12 | 96.55 | 99.5 | L115 |
| `utils/sub2apiMatch.ts` | 100 | 81.25 | 100 | 100 | L47,69 |

> ⚠️ `utils/formatters.ts` 是已测文件中**唯一明显低覆盖**（38% line，L77-108 未覆盖段），是真实可补的缺口。

### 2.4 测试文件清单（16 个）

```
src/components/shared/  BalanceBar.test.tsx, CompactCard.test.tsx, CostTrendChart.test.tsx,
                       StatChip.test.tsx, TestResultBody.test.ts, colorScale.test.ts, usageColor.test.ts  (7)
src/services/           api.test.ts  (1)
src/utils/              ccswitchMatch, chart, deepMerge, formatters, navGuard, pinyin,
                       platformPaste, sub2apiMatch  (8 .test.ts)
```

### 2.5 ⚠️ 前端覆盖率的"假象"

**v8 默认只统计被测试 import 链触达的文件**。下表目录的文件**完全没被任何测试 import**，因此**不进分母**，导致总 94.99% 看起来很高但实际覆盖面极窄：

| 目录 | source 文件数 | 有测试? |
|---|---|---|
| `src/pages/` (18 页面) | 18 | ❌ **0 测试** |
| `src/components/settings/` (14 组件) | 14 | ❌ **0 测试** |
| `src/components/platforms/` (4 组件) | 4 | ❌ **0 测试** |
| `src/components/` 顶层 (Sidebar/SortableList/PopoverCards 等) | 5 | ❌ 0 |
| `src/services/` (api + 3 schema/updater) | 4 | 1（仅 api.ts） |
| `src/utils/` | 8 | 8 ✅ |
| `src/themes/` (3 主题文件) | 3 | ❌ 0 |

**真实有测试的前端文件**: 10/94 ≈ **10.6%**（按文件计），其余 84 个文件覆盖率 = "未测量"（≠ 100%）。

> 修法（不在本研究范围，仅记录）：需加 `vitest.config.ts` 配 `coverage.include: ['src/**']` 强制全量统计，才会暴露真实 line%。

---

## 3. 达 80% 工作量预估

### 3.1 Rust：**已达标，无需补测即可宣告 ≥80%**

- 当前 87.47%，缓冲 ~7pp
- 若要更稳（防回归 + 提 func% 到 80）：聚焦 commands 壳层重复测性价比低，**优先补 0% 的薄壳命令**（tray/skills/model_test/proxy/quota/model_fetch ≈ 6 文件，多数是 db 转发，单测成本低，估 ~6 个 test 文件可拉 commands 层到 70%+）
- 真实业务逻辑缺口集中在 `gateway/proxy/passthrough.rs`(166)、`import_export/apply/mod.rs`(124)、`proxy/forward.rs`(95)——但这三个已有 `test_passthrough.rs` / `test_integration.rs` 配对，缺的是**分支覆盖**而非文件缺失，补测难度高

### 3.2 前端：**line% 名义已达标（94.99%），但需配置修正才反映真实**

两种判读：

1. **按当前 v8 默认口径**: 94.99%，已远超 80%，**0 工作量**。
2. **按"真实覆盖面"口径**（配置 `coverage.include` 后）: 会暴跌至个位数 %。要拉到真 80%，需补：
   - 18 pages × 平均 1 test = ~18 文件
   - 14 settings components × ~1 = ~14 文件
   - 4 platforms components = ~4 文件
   - 3 services 剩余（schema/updater）= ~3 文件
   - 合计 **~39 test 文件**（粗估，未含 i18n / Tauri mock 成本——前端测 React 组件需 mock `@tauri-apps/api` invoke，setup 成本不低）

---

## 4. `需要:` 标注 / Caveats

- **前端覆盖率口径需 main 决策**: 当前 94.99% 是"被测文件局部覆盖率"，非"全仓覆盖率"。task 06-20 若按字面 line% 验收则已通过；若要求全量统计，需先加 `vitest.config.ts` 重测（不在本研究只读范围内）。**需要: main 确认验收口径**。
- Rust `cargo llvm-cov` 全量跑 ~1m38s 编译 + 16s 测试，稳定可复现，无报错。
- `block v0.1.6 future-incompat` warning 出现（已知，记忆 `block-future-incompat-accepted` 有记录），不影响覆盖率。
- 并行 task 07-01 改前端 locale 在 worktree 隔离，未干扰本次主仓测量。
- 测试文件（`test_*.rs`）在覆盖率表中显示 0%/100% 不一，是 llvm-cov 对 `#[cfg(test)]` 模块插桩的常态，**不代表测试未运行**——本报告一律按 source 文件（非 test_*）统计缺口。
- 全量原始输出: `/tmp/llvm-cov-out.txt`（1431 行，含 1096 测试逐条 + 281 文件表）。

---

## 5. 相关文件 / 数据源

- `/tmp/llvm-cov-out.txt` — Rust llvm-cov 完整原始输出
- `/tmp/cov-table.txt` — 提取的 281 文件覆盖率表
- `package.json` — `test:cov` = `vitest run --coverage`
- `.trellis/tasks/06-20-test-coverage-80/task.json` — parent task（status=planning）
- `.trellis/tasks/06-20-test-coverage-80/{check,implement}.jsonl` — 空（未启动）
- 子 task 目录未创建: `06-20-test-cov-rust` / `06-20-test-cov-frontend`（task.json children 引用但无目录）

## Reproduce

```bash
# Rust
cd /Users/luoxin/persons/lyxamour/aidog/src-tauri
cargo llvm-cov --summary-only 2>&1 | tail -300   # 总表在最后

# 前端
cd /Users/luoxin/persons/lyxamour/aidog
yarn test:cov 2>&1 | tail -40
```
