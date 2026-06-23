# Research: 前端 vitest 框架搭建 + 分支覆盖率 ≥80% 规划

- **Query**: 为 trellis 任务 `06-20-test-cov-frontend` 做规划调研，产出可直接落 PRD 的方案
- **Scope**: mixed（内部 src/ 只读盘点 + 外部 vitest/Tauri mock 版本与 API 核实）
- **Date**: 2026-06-23

---

## 0. 现状快照（核实自仓库）

| 项 | 事实 | 来源 |
|---|---|---|
| 技术栈 | Vite 7 + React 19.1 + TS ~5.8.3 + `@vitejs/plugin-react` ^4.6 | `package.json:19-47` |
| 测试现状 | 零前端测试；scripts 仅 dev/build/preview/tauri/check:i18n/check:statusline-runtime/test:statusline-golden/version:* ；**无 vitest 依赖** | `package.json:8-18` |
| 包管理 | yarn@4.16.0 (berry)，`nodeLinker: node-modules`（非 PnP，工具链兼容好） | `package.json:7` / `.yarnrc.yml` |
| Node | v26.3.0（注意：vitest 4 peer 要求 `@types/node ^20 \|\| ^22 \|\| >=24`，26 满足） | `node --version` |
| vite.config | `defineConfig(async () => ({...}))` 异步工厂；双入口 main+popover | `vite.config.ts:8-35` |
| tsconfig | `strict` + `noUnusedLocals` + `noUnusedParameters` + `noEmit`，`moduleResolution: bundler`，`include: ["src"]` | `tsconfig.json` |
| Tauri API | `@tauri-apps/api` ^2（实测装的是 2.11.1），**已自带 `mocks` 模块**（`mocks.js/.cjs/.d.ts`），导出 `mockIPC`/`clearMocks`/`mockWindows`，`mockIPC` 自 2.7.0 起支持 `shouldMockEvents` 可 mock `listen` | `node_modules/@tauri-apps/api/mocks.d.ts` |
| api.ts | 1986 行，155 处 `invoke`，仅 import `@tauri-apps/api/core` 的 `invoke` 与 `event` 的 `listen`，无其他副作用 import | `src/services/api.ts:1-2` |

---

## 1. vitest 框架接入方案

### 1.1 需装依赖（全部 devDependencies，版本为撰写时 latest，落 PRD 时锁定）

```bash
yarn add -D vitest@^4.1.9 \
  @vitest/coverage-v8@^4.1.9 \
  @testing-library/react@^16.3.2 \
  @testing-library/dom@^10.4.1 \
  @testing-library/jest-dom@^6.9.1 \
  @testing-library/user-event@^14.6.1 \
  jsdom@^29.1.1
```

要点（已核实 npm peerDependencies）：
- **vitest 4.1.9** peer `vite: ^6 || ^7 || ^8` → 兼容本仓 Vite 7。`@vitest/coverage-v8` 必须与 vitest **同版本**（peer 锁 `4.1.9`）。
- **`@testing-library/react` 16.x 把 `@testing-library/dom` 列为 peer（`^10.0.0`），不再内置**，必须显式装 `@testing-library/dom@^10.4.1`，否则运行时报 `Cannot find module @testing-library/dom`。react 16.x peer 已支持 react/react-dom `^19`，匹配 React 19.1。
- `jsdom@29` peer 仅可选 `canvas`（不装无碍，除非测 `<canvas>`，本仓 CostTrendChart 用 SVG 非 canvas，无需）。
- `@testing-library/jest-dom` 提供 `toBeInTheDocument` 等 DOM 匹配器，无 peer 约束。
- `@testing-library/user-event` 用于交互测试（点击/输入），peer 仅 `@testing-library/dom`。

> 备选 DOM 环境 `happy-dom` 更快但兼容性弱于 jsdom；本仓有 WKWebView/CSS/i18n 等边角，先用 **jsdom** 稳妥。

### 1.2 vite.config.ts 配置（在现有 async 工厂里加 `test` 块）

现有 `vite.config.ts` 是 `defineConfig(async () => ({...}))`。vitest 读同一份配置。需在返回对象里追加 `test`，并在顶部加 triple-slash 引用拿类型：

```ts
/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: { /* ...不变... */ },
  build: { /* ...不变... */ },
  test: {
    globals: true,                 // 免每文件 import { describe, it, expect }
    environment: "jsdom",          // 组件测试需 DOM；纯函数文件可 per-file 用 // @vitest-environment node 提速
    setupFiles: ["./src/test/setup.ts"],
    css: false,                    // 不处理 CSS（主题是 CSS 变量，测试不依赖真实样式）
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov"],
      reportsDirectory: "./coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.{test,spec}.{ts,tsx}",
        "src/test/**",
        "src/main.tsx", "src/popover.tsx",   // 入口 bootstrap，不计
        "src/vite-env.d.ts",
        "src/themes/**",                      // 纯静态色板常量，无逻辑分支
        "src/locales/**",                     // i18n JSON 装配
        "src/assets/**",
      ],
      thresholds: {
        branches: 80,    // 任务硬指标
        // 建议同时设其余三项护栏（可低于 80 起步，逐步抬升）
        functions: 70,
        lines: 70,
        statements: 70,
      },
    },
  },
}));
```

> 注意：现配置是 **async 工厂**。vitest 支持异步 config，但若将来拆分，可改用独立 `vitest.config.ts` + `mergeConfig`。当前单文件追加 `test` 块最省事，无需拆。

### 1.3 setupFiles（`src/test/setup.ts`）

```ts
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";

afterEach(() => {
  cleanup();        // 卸载 RTL 渲染的组件
  clearMocks();     // 清 Tauri IPC mock，防跨用例串味
});
```

### 1.4 package.json scripts

```jsonc
"test": "vitest run",
"test:watch": "vitest",
"test:ui": "vitest --ui",            // 可选，需额外装 @vitest/ui
"test:cov": "vitest run --coverage"
```

> CI 用 `yarn test:cov`（带 `--coverage` 自动套用 thresholds，未达标退出码非 0 即门禁）。本仓 CLAUDE.md 约定「warning 必须清」，建议把 `test:cov` 接入现有质量关卡。

### 1.5 tsconfig 调整（避免 test 文件污染 build）

`tsconfig.json` 现 `include: ["src"]` 会把 `*.test.ts` 也纳入 `tsc` 类型检查（`yarn build` 跑 `tsc && vite build`）。两种处理：
- **方案 A（推荐）**：保持 test 文件在 `src/` 内被 tsc 检查（测试类型也该过编译），在 tsconfig `compilerOptions.types` 加 `["vitest/globals", "@testing-library/jest-dom"]` 让 `globals:true` 与 jest-dom 匹配器类型可识别。
- **方案 B**：新建 `tsconfig.test.json` 隔离，并在主 `tsconfig.json` 的 build include 用 `exclude: ["src/**/*.test.*","src/**/*.spec.*","src/test/**"]`。
> 推荐 A：测试代码也应受 strict 检查，仅需补 `types`。**需要: 确认是否接受 build 时 tsc 一并检查测试文件（影响 `yarn build` 时长，通常可忽略）。**

---

## 2. 测试目标优先级清单（易测 + 高价值排序）

排序原则：纯函数（零 DOM/零 IPC，分支密集，ROI 最高）→ 轻依赖工具 → 数据映射 → 关键组件。

### 梯队 A — 纯函数，无任何外部依赖（最高优先，直接拉满分支覆盖）

| 优先级 | 文件 | 行数 | 导出 | 分支点 / 测试要点 | 依赖 |
|---|---|---|---|---|---|
| A1 | `src/utils/formatters.ts` | 66 | `formatNumber/formatCost/formatCostUsd/formatPercent/successRate/sumTokens` | M/K 边界、`n%1` 整数/小数分支、`formatCost` 4 档（≤0/≥1/≥0.01/极小值 decimals clamp）、`successRate` total=0、`sumTokens` 忽略 NaN/null | 无 |
| A2 | `src/components/shared/usageColor.ts` | 105 | `usageLevelToColor/cycleMsForTier/codingRemainPct/colorFromCodingRemainPct/codingTierLevel/balanceColorLevel` | switch 全分支、pace 算法边界(utilRatio≤0 / elapsedRatio≤0)、clamp、util≥100→danger、`Number.isFinite` 守卫、null 入参→neutral。**与后端 `usage_color.rs` 阈值对齐，是单一事实源，回归价值极高** | 仅 type import |
| A3 | `src/components/shared/colorScale.ts` | 53 | `levelColor/levelBg/successRateLevel/errorRateLevel/costLevel/successRateColor` | 各阈值边界(99/95、1/5、warnAt/dangerAt)、total≤0→neutral | 无 |
| A4 | `src/utils/deepMerge.ts` | 36 | `deepMerge` | 嵌套对象递归、数组/标量覆盖、base-only key 保留、`isPlainObject`（null/array/Date 等非 plain 分支） | 无 |
| A5 | `src/utils/chart.ts` | 39 | `smoothPath` | 0 点/1 点/2 点退化、≥3 点贝塞尔、clampY 过冲 | 无 |
| A6 | `src/utils/platformPaste.ts` | 384 | `normalizeForMatch/guessProtocol/matchPlatform/parsePlatformPaste` + 类型 | 杂乱文案解析、base64 解码、防爬汉字剔除、host 最长特异匹配、keyword fallback。**无 DOM import（已核实纯函数）**，分支密集高价值 | 无 |
| A7 | `src/utils/sub2apiMatch.ts` | 96 | `mapPlatformToProtocol/sub2apiAccountToPlatformJson` | platform→protocol 映射全表、account→json 装配 | 轻（类型） |
| A8 | `src/utils/ccswitchMatch.ts` | 260 | `matchCcProvider/extractModels/ccProviderToPlatformJson` + `DEFAULT_DIMS` | MatchedBy 三态(preset_keyword/base_url_host/protocol_fallback)、模型抽取、dims 维度组合 | 轻（类型） |

### 梯队 B — 纯函数但带 1 个外部库依赖

| 优先级 | 文件 | 导出 | 要点 | 依赖 |
|---|---|---|---|---|
| B1 | `src/utils/pinyin.ts` | `pinyinMatch` | 直接子串/拼音/中文查询转拼音 4 条匹配路径、空查询→true、LRU 缓存命中与淘汰（>500） | `pinyin-pro`（真实库，无需 mock，nodejs 可跑） |
| B2 | `src/utils/navGuard.ts` | `registerNavGuard/requestNavigation` | 无 guard 直接 proceed、有 guard 中介、last-wins、unregister 仅清当前 active | 无（模块级单例 state，注意用例间需 reset，可 `vi.resetModules` 或测后 unregister） |

### 梯队 C — 服务层（Tauri IPC 映射，需 mock）

| 优先级 | 文件 | 要点 | 依赖 |
|---|---|---|---|
| C1 | `src/services/api.ts`（1986 行，155 invoke） | 不必逐 155 个测全。挑**有逻辑的封装**（参数 camelCase 转换、默认值、返回值整形、`listen` 事件订阅包装）测；纯 `invoke('x', args)` 一行直传的可批量 smoke。优先覆盖被多页复用的 api（platform/group/stats/usage） | `mockIPC`（见 §4） |

### 梯队 D — 组件（DOM 渲染，i18n/state 依赖，最后做）

按「逻辑含量高 + 依赖少」优先：
| 优先级 | 组件 | 理由 |
|---|---|---|
| D1 | `src/components/shared/StatChip.tsx` / `BalanceBar.tsx` / `CompactCard.tsx` | 纯展示，props→DOM，无 IPC，配合 A2/A3 色彩逻辑验证渲染 |
| D2 | `src/components/shared/CostTrendChart.tsx` | 消费 `chart.ts`，SVG path 断言 |
| D3 | `src/components/platforms/usePlatformCards.ts` / `src/hooks/usePolling.ts` | 自定义 hook，用 `renderHook` 测，mock IPC |
| D4 | 巨石组件（Settings/Platforms/Groups/Logs 等）| 见 §5 风险，**MVP 不强求**，靠纯函数抽离覆盖 |

---

## 3. 达成 ≥80% 分支覆盖的 MVP 任务拆解（按 deliverable）

> 策略：分母上排除无逻辑文件（themes/locales/assets/入口，见 §1.2 coverage.exclude），分子集中打梯队 A/B 纯函数（分支极密），即可把全局 branches 推到 80%。组件梯队作为加分项。

| # | Deliverable | 文件 | 可并行 | 验收（独立可验证） |
|---|---|---|---|---|
| T0 | 框架接入 | 装依赖 + vite.config `test` 块 + `src/test/setup.ts` + scripts + tsconfig types | 否（前置，阻塞全部） | `yarn test` 能跑空套件并退出 0；`yarn test:cov` 产出 coverage/ |
| T1 | formatters 测试 | `src/utils/formatters.test.ts` | ✅ | formatters.ts branches 100% |
| T2 | usageColor 测试 | `src/components/shared/usageColor.test.ts` | ✅ | usageColor.ts branches ≥95%（含 pace 边界） |
| T3 | colorScale 测试 | `src/components/shared/colorScale.test.ts` | ✅ | colorScale.ts branches 100% |
| T4 | deepMerge + chart 测试 | `src/utils/deepMerge.test.ts` / `chart.test.ts` | ✅ | 两文件 branches ≥90% |
| T5 | platformPaste 测试 | `src/utils/platformPaste.test.ts` | ✅ | branches ≥80%（解析路径多，重点 matchPlatform/guessProtocol） |
| T6 | sub2api + ccswitch 测试 | `src/utils/sub2apiMatch.test.ts` / `ccswitchMatch.test.ts` | ✅ | 两文件 branches ≥80% |
| T7 | pinyin + navGuard 测试 | `src/utils/pinyin.test.ts` / `navGuard.test.ts` | ✅ | branches ≥85%（pinyin 含 LRU 淘汰；navGuard 含 last-wins） |
| T8 | api.ts 关键封装 + IPC mock 基建 | `src/services/api.test.ts` + mock helper | 依赖 T0，与 T1-T7 并行 | 选 10-15 个有逻辑的 api 通过 mockIPC 断言 |
| T9（加分） | shared 组件渲染 | StatChip/BalanceBar/CompactCard/CostTrendChart `.test.tsx` | 依赖 T0 + i18n 测试 wrapper | 组件 branches 计入，推高总覆盖 |
| T10 | 门禁固化 | 确认 `coverage.thresholds.branches:80` 生效，CI 接入 | 末尾 | 故意降覆盖能让 `test:cov` 失败 |

并行说明：**T0 是唯一串行前置**；T1-T8 完成 T0 后可全并行（互不共享文件）；T9 依赖 i18n test wrapper（见 §5）。

---

## 4. Tauri invoke mock 策略

**首选：`@tauri-apps/api/mocks` 的 `mockIPC`（官方方案，本仓已装 2.11.1，零额外依赖）。**

- `mockIPC((cmd, payload) => ...)` 拦截所有 `invoke`，按 `cmd` 名分派返回值（可同步值或 Promise）。
- `listen` mock：`mockIPC(() => {}, { shouldMockEvents: true })`（2.7.0+），配 `emit` 触发回调。
- `afterEach(clearMocks)` 清状态（已在 §1.3 setup）。
- 因 api.ts 参数走 camelCase（见 memory `tauri-invoke-param-camelcase`），mock 回调里可断言 `payload` 的 key 是 camelCase，顺带防回归。

示例：

```ts
import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, it, expect, vi } from "vitest";
import { platformApi } from "@/services/api"; // 实际导出名以 api.ts 为准

describe("platformApi.list", () => {
  it("invokes correct command and returns rows", async () => {
    const handler = vi.fn((cmd: string) =>
      cmd === "platform_list" ? [{ id: 1, name: "x" }] : undefined,
    );
    mockIPC(handler);
    const rows = await platformApi.list();
    expect(handler).toHaveBeenCalledWith("platform_list", expect.anything());
    expect(rows).toHaveLength(1);
  });
});
```

**备选：`vi.mock("@tauri-apps/api/core")`** —— 当只想 stub 单个 `invoke` 且不关心 IPC 协议层时更轻：

```ts
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
```

取舍：`mockIPC` 更贴近真实 IPC 边界（payload 序列化语义一致），推荐作为 api.ts 测试默认；`vi.mock` 用于组件测试里只需让 invoke 静默/返回固定值的场景。

> `mockIPC` 需要 `window.__TAURI_INTERNALS__` 等，jsdom 环境下可用；纯 node 环境（`// @vitest-environment node`）的纯函数测试**不要**碰 Tauri，保持零依赖。

---

## 5. 风险 / 难测点

| 风险 | 说明 | 缓解 |
|---|---|---|
| **WKWebView 特性** | memory `wkwebview-html5-dnd-drop-fails`：DnD 走 pointer 事件 + `elementFromPoint` hit-test。jsdom 无真实布局，`elementFromPoint` 恒返回 null，**拖拽逻辑（SortableList / PopoverCards 二维拖拽）几乎无法在 jsdom 验证** | 拖拽交互不纳入 MVP；抽离其中的纯计算（网格 row/col 映射）单独测；端到端拖拽留给手动/未来 Tauri webdriver |
| **CSS 主题** | 颜色返回的是 `var(--color-*)` 字符串（colorScale/usageColor），**不依赖真实 CSS 解析**，反而好测（断言字符串即可）。但组件视觉（liquidGlass 不透明、fixed modal transform）jsdom 测不出 | `css:false`；只断言 className / inline style 字符串，不断言计算样式 |
| **i18n** | 组件用 `react-i18next` 的 `t()`；`locales/index.ts` 默认同步装 zh-CN+en-US，其余 5 语言 dynamic import。测试组件需包一层 i18n provider，否则 `t()` 返回 key | 建一个 `src/test/render.tsx` 自定义 render，包 `I18nextProvider`（用已初始化的同步 i18n 实例或 mock `useTranslation` 返回 `t:(k)=>k`）。断言用 key 而非译文，避免文案变动脆性 |
| **巨石组件** | Settings(编排容器)/Platforms/Groups/Logs/AppSettings 行数大、IPC fan-out 多、局部刷新 + epoch 守卫 + 乐观更新（见多条 memory）。**全量渲染测成本极高、脆** | MVP 不直测巨石；价值逻辑已大量抽到 utils/shared（formatters/usageColor/platformPaste/usePlatformCards）→ 测抽离层即可贡献覆盖。组件层留少量 smoke（能渲染不崩） |
| **模块级单例 state** | `navGuard.ts` 的 `activeGuard`、`pinyin.ts` 的 LRU `pinyinCache` 跨用例共享 | 用例后显式 unregister / 或 `vi.resetModules()` 重置；pinyin LRU 淘汰测需构造 >500 条 |
| **async vite.config** | 现配置是 `async () => ({...})`，vitest 支持但需确认 `test` 字段类型可见 | 顶部加 `/// <reference types="vitest/config" />` |
| **build 含 test 文件** | `tsconfig include:["src"]` 会让 `yarn build` 的 tsc 检查 `*.test.ts`，`noUnusedLocals/Parameters` 严格 | 见 §1.5 方案 A/B；**需要: 确认接受方案 A（测试也过 tsc）还是 B（隔离）** |
| **覆盖率分母** | 若不 exclude themes/locales/assets/入口，大量无分支静态文件会稀释（虽然无分支不拉低 branches%，但 functions/lines 受影响） | 见 §1.2 exclude 清单 |

---

## 待确认（需主会话转达）

- **需要**: build 时是否接受 tsc 一并检查测试文件（§1.5 方案 A），还是隔离 tsconfig.test.json（方案 B）？
- **需要**: 是否同时启用 functions/lines/statements 门禁（建议起步值 70），还是仅卡 branches 80？
- **需要**: 是否纳入 `@vitest/ui`（开发体验，非必需）？

## 相关 memory（已有踩坑沉淀）

- `tauri-invoke-param-camelcase` — invoke 参数必须 camelCase，mock 断言可顺带防回归
- `usage-color-single-source` — usageColor 前后端阈值单一事实源，测试是其回归防线
- `wkwebview-html5-dnd-drop-fails` — 拖拽难测根因
- `routeless-nav-guard` / `navcontext-*` — navGuard 行为
- `platform-smart-paste-parser` — platformPaste 解析规则
