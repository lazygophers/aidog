# PRD: 前端 vitest 框架 + 四维覆盖率 ≥80%

> 调研依据: `research/plan.md`（2026-06-23）。本 PRD 已锁定主会话决策口径。

## 1. 目标

前端从零接入 vitest 测试框架，并使**全局覆盖率四维度（branches / functions / lines / statements）均 ≥80%**，门禁固化到 `yarn test:cov`（未达标退出码非 0）。

## 2. 决策口径（已拍板）

| 项 | 决策 |
|---|---|
| 门禁维度 | **四维度全卡 80**（branches/functions/lines/statements 均 ≥80） |
| tsc 与测试文件 | **隔离 `tsconfig.test.json`**（方案 B）：测试文件不进 `yarn build` 的 tsc；主 `tsconfig.json` 的 build `include`/`exclude` 排除 `*.test.*` / `*.spec.*` / `src/test/**`，测试类型检查走独立 config |
| `@vitest/ui` | **不纳入**（开发体验非必需，避免额外依赖） |
| DOM 环境 | jsdom（稳妥优先于 happy-dom） |
| IPC mock | 官方 `@tauri-apps/api/mocks` 的 `mockIPC`（仓库已装 2.11.1，零额外依赖） |

## 3. 依赖（devDependencies，落地时锁版本）

```
vitest@^4.1.9
@vitest/coverage-v8@^4.1.9          # 必须与 vitest 同版本
@testing-library/react@^16.3.2
@testing-library/dom@^10.4.1        # react 16.x 不再内置，必须显式装
@testing-library/jest-dom@^6.9.1
@testing-library/user-event@^14.6.1
jsdom@^29.1.1
```

## 4. 范围与覆盖策略

分母排除无逻辑文件（`themes/` `locales/` `assets/` 入口 `main.tsx`/`popover.tsx` `vite-env.d.ts`），分子集中打**纯函数层**（分支极密、ROI 最高），把全局四维推到 80。组件层作为补足项（四维全卡需 functions/lines 达标，纯函数层不足以单独凑齐 lines/statements，故组件 smoke 渲染必须纳入而非加分）。

> ⚠️ 与「仅卡 branches」不同：四维全卡要求 **functions/lines/statements 也 ≥80**，纯函数层覆盖不够，必须补足组件与 api.ts 封装的行/函数覆盖。组件梯队（T9）由「加分」上升为「必做」。

## 5. Deliverables（T1-T8 在 T0 后可并行）

| # | Deliverable | 验收 |
|---|---|---|
| T0 | 框架接入：装依赖 + vite.config `test` 块 + `src/test/setup.ts` + `tsconfig.test.json` + scripts(`test`/`test:watch`/`test:cov`) | `yarn test` 跑空套件退 0；`yarn test:cov` 产 coverage/；`yarn build` 不被测试文件类型检查阻断 |
| T1 | `utils/formatters` | branches 100% |
| T2 | `shared/usageColor`（与后端 `usage_color.rs` 阈值对齐，回归价值高） | branches ≥95% |
| T3 | `shared/colorScale` | branches 100% |
| T4 | `utils/deepMerge` + `utils/chart` | 两文件 branches ≥90% |
| T5 | `utils/platformPaste` | branches ≥80% |
| T6 | `utils/sub2apiMatch` + `utils/ccswitchMatch` | 两文件 branches ≥80% |
| T7 | `utils/pinyin` + `utils/navGuard`（注意模块级单例 state，用例间 reset） | branches ≥85% |
| T8 | `services/api.ts` 关键封装 + IPC mock 基建（`mockIPC`），挑有逻辑的封装 10-15 个 + 批量 smoke 覆盖一行直传 | api.ts functions/lines 计入，断言 payload key camelCase 顺带防回归 |
| T9 | shared 组件渲染 smoke：StatChip/BalanceBar/CompactCard/CostTrendChart + 关键 hook（usePlatformCards/usePolling），需 `src/test/render.tsx`（I18nextProvider wrapper，断言 key 非译文） | 组件 functions/lines 计入，推全局四维达 80 |
| T10 | 门禁固化：`coverage.thresholds` 四维全 80 生效 | 故意降覆盖能让 `test:cov` 失败 |

## 6. 难测点（不纳入 MVP / 缓解）

- **WKWebView 拖拽**（DnD pointer + `elementFromPoint`，jsdom 恒 null）→ 不直测，抽网格 row/col 纯计算单独测
- **巨石组件**（Settings/Platforms/Groups/Logs，IPC fan-out + epoch 守卫 + 乐观更新）→ 不全量渲染测，靠抽离层 + 少量 smoke
- **i18n**：组件需 `src/test/render.tsx` 包 I18nextProvider，断言 key
- **四维全卡风险**：巨石组件的 lines/statements 若计入分母可能拉低 lines/statements，需评估是否将其超大编排组件纳入 coverage.exclude（实现阶段按实测分母决定，避免为薄编排壳硬凑行覆盖）

## 7. 验证

- `yarn test:cov` 四维全 ≥80 且退 0
- `yarn build` 通过（测试文件隔离不阻断）
- `yarn check:i18n` 不回归
