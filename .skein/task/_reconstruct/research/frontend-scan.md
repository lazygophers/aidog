# frontend-scan — reconstruct 扫描（前端侧，src/ + docs/）

扫描时间基准：2026-07-31，分支 `feature/next`，HEAD `cf371e0c`。
范围：`src/`（React 19 + TS 5.8 + Vite 7 + Tailwind v4 + shadcn/Radix）、`docs/`（Rspress）、`scripts/`、`vite.config.ts`、`tsconfig.json`。
全部结论以 file:line 为证据；无证据的一律不写。

---

## 0. 事实底稿（供后续规则引用）

| 事实 | 证据 |
|---|---|
| 无 eslint / prettier / biome 配置 | 仓库根 `ls -a` 无 `eslint.config.*` / `.eslintrc*` / `.prettierrc` / `biome.json`；`package.json` 无 lint script |
| 唯一「lint」= `tsc` strict + `noUnusedLocals` + `noUnusedParameters` | `tsconfig.json:27-30` |
| tsconfig **排除**所有 test 文件 | `tsconfig.json:34-40`（`yarn build` 不做测试文件类型检查） |
| 测试 27 个 `.test.ts(x)`（非 CLAUDE.md 写的 18） | `find src -name '*.test.ts*' \| wc -l` = 27 |
| vitest 配置内联在 `vite.config.ts`，无独立 `vitest.config.*` | `vite.config.ts:41-76` |
| CI 只跑 `yarn build`（docs 站），**不跑** `yarn test` / `check:i18n` | `.github/workflows/deploy-docs.yml:39`；`.github/workflows/` 仅两个 workflow |
| `invoke` 调用点只有 3 处不在 `services/api/` 下 | `src/popover.tsx`、`src/domains/platforms/useProtocolLogo.ts`（其余 265 处 shadcn import 与之无关） |
| 40 个 `xxxApi` 对象命名空间 | `grep 'export const \w*Api = {' src/services/api/*.ts` = 40 |
| 内联 `style={{` 2540 处 vs `className=` 749 处 | grep 计数；样式主力仍是 inline style，Tailwind utility 是次要 |

---

## 1. 命名约定

### 1.1 页面目录归属规律（recall / frontend）
**规则**：`src/pages/<PageName>.tsx` 是编排壳（默认导出/具名导出同名组件）；一旦拆分，同名目录 `src/pages/<PageName>/` 承载该页私有的 `useXxx.ts` hook、`XxxView.tsx` / `XxxListView.tsx` 视图、`XxxModal(s).tsx` 弹窗、`constants.ts`、`primitives.tsx`。禁把页私有组件放 `components/`。
**证据**：`src/pages/Logs/`（`ListView.tsx` / `DetailPanel.tsx` / `useLogsFilters.ts` / `useLogsList.ts` / `useLogsDetail.ts` / `primitives.tsx` / `types.ts`）、`src/pages/Groups/`（`GroupListView.tsx` / `GroupCreateModal.tsx` / `useGroupData.ts` / `usePlatformDrag.ts`）、`src/pages/Mcp/`、`src/pages/Skills/`、`src/pages/CliProxy/`、`src/pages/platforms/`、`src/pages/PopoverConfigTab/`、`src/pages/AppSettings/`
**namespace**: recall ｜ **category**: frontend

> 注意大小写不一致：`src/pages/platforms/`（小写）vs `src/pages/Groups/`、`src/pages/Logs/`（大写）。**这是当前代码的既成事实，不是可执行约定**，不写成规则。

### 1.2 三层组件归属（recall / frontend）
**规则**：`components/shared/` = 跨页展示组件 + 其纯函数（有 `index.ts` barrel，是唯一被 coverage 纳入分母的组件目录）；`components/ui/` = shadcn 原语（24 个，`@/components/ui/*` 路径导入，265 处引用）；`components/settings/` + `components/platforms/` = 单域组件族。
**证据**：`src/components/shared/index.ts`；`vite.config.ts:59-63`（coverage include 只含 `src/utils/**`、`src/components/shared/**`、`src/services/api.ts`）；`src/components/ui/` 24 文件
**namespace**: recall ｜ **category**: frontend

### 1.3 domains/ 层（recall / arch）
**规则**：跨页复用的**业务派生逻辑**（非展示）放 `src/domains/<domain>/`，各带 `index.ts` barrel。现有 `domains/platforms`（defaults/health/autoCategorize/constants + `useProtocolLogo` / `useProtocolMeta` hook）、`domains/groups`（commands/query/routing/editReducer/proxy-env）、`domains/shared/tokens.ts`。
**证据**：`src/domains/platforms/index.ts`、`src/domains/groups/index.ts`、`src/domains/shared/tokens.ts`
**namespace**: recall ｜ **category**: arch

### 1.4 API 命名空间（recall / ts-rust-boundary）
**规则**：每个 Tauri 命令族封装成 `export const <domain>Api = { ... }` 对象常量，落在 `src/services/api/<domain>.ts`，由 `src/services/api/index.ts` barrel `export *`。前端一律 `import { xxxApi } from "../../services/api"`，**禁直接 `invoke`**（现存例外仅 `src/popover.tsx`、`src/domains/platforms/useProtocolLogo.ts`）。
**证据**：`src/services/api/index.ts:1-21`；40 个 `xxxApi`；`src/services/api/platforms.ts:3` 的 invoke import
**namespace**: recall ｜ **category**: ts-rust-boundary

---

## 2. 错误处理

### 2.1 主导模式：`catch → setError(String(e))` 渲染进页面
**规则**：`invoke` 失败在调用组件内 `try/catch`，`String(e)` 存进局部 `useState<string | null>`，渲染成页内错误条；**禁 `window.alert` / `window.confirm`**（无 toast 库接入——`sonner` 在 `package.json` 里但 `src/` 零引用）。
**证据**：78 处 `String(e)`，如 `src/components/settings/MiddlewareRules.tsx:164,191,537,578,588,612,725`、`src/components/settings/MitmConfig.tsx:78`、`src/components/settings/SchedulingSettings.tsx:77`、`src/components/UpdatePromptModal.tsx:40`；`grep sonner src/` 零命中
**namespace**: recall ｜ **category**: frontend

### 2.2 双态消息对象（recall / frontend）
**规则**：需要同时表达成功/失败的场景用 `useState<{ok|kind|type, text} | null>`，而非两个独立 state。
**证据**：`src/pages/Mcp/useMcpData.ts:23`（`{kind:"ok"|"err"; text}`）、`src/pages/AppSettings/LogSettingsSection.tsx:32`（`{text; type:"success"|"error"}`）、`src/components/settings/ImportExport/ScheduledBackupSection.tsx:36`（`{ok; text}`）
**namespace**: recall ｜ **category**: frontend

### 2.3 启动期/best-effort 读取显式静默吞错
**规则**：App 启动期的设置读取、预热调用一律 `.catch(() => {})` 静默（28 处），**不得**因这些失败阻断首屏；但解析类失败要 `console.warn` 留痕。
**证据**：`src/App.tsx:69,77,102,140`（`proxyLogApi.getSettings` / `notificationApi.getSettings` / `autoUpdateApi.get`）；`src/context/AppContext.tsx:298-300`（`loadSettingsFromDB` catch 返 `{}`）、`:316,346,371`（DB 写失败不阻断 UI）；对比 `src/domains/platforms/defaults.ts:96,102`（parse / RPC 失败 `console.warn` + 空文档降级）
**namespace**: recall ｜ **category**: frontend

### 2.4 Tauri `listen` 清理必须 catch
**规则**：`listen()` 返回 Promise，`useEffect` cleanup 写 `unlistenPromise.then(un => un()).catch(e => console.error(e))`，禁裸 `.then(un => un())`（unhandled rejection）。
**证据**：`src/App.tsx:92,128`
**namespace**: recall ｜ **category**: frontend

---

## 3. 测试

### 3.1 组件测试统一走 `src/test/render.tsx`（**core 候选**）
**规则**：组件测试必须 `import { render, screen } from "<rel>/test/render"`（不是直接 `@testing-library/react`）——它包一层**空 resources** 的隔离 i18n 实例，`t()` 回传 key 本身；断言一律断 **i18n key**，禁断译文。
**证据**：`src/test/render.tsx:1-34`（`parseMissingKeyHandler: (key) => key`）；`src/components/shared/CompactCard.test.tsx:2`、`src/components/platforms/PlatformCard.test.tsx:4`
**namespace**: **core** ｜ **category**: test

### 3.2 断行为不断 className / 快照（**core 候选**）
**规则**：组件测试用 `screen.getByText` / `getByRole` / `queryByRole` + `userEvent` 交互断言；**禁 `toHaveClass` 与快照**（shadcn 迁移后 className 随样式漂移）。
**证据**：`src/components/platforms/PlatformCard.test.tsx:1-2`（文件头顶注释即此规则）；`src/components/shared/CompactCard.test.tsx:8-30`（`getByText` / `queryByRole("button")` / `user.click`）
**例外**：`src/components/settings/__snapshots__/statusline-gen.test.ts.snap` — 唯一快照，测的是**生成的 bash 脚本文本**（非 DOM），不违反本规则。
**namespace**: **core** ｜ **category**: test

### 3.3 setup 按 window 存在性守卫
**规则**：纯函数测试文件用 `// @vitest-environment node`；全局 `afterEach` 里 `cleanup()` / `clearMocks()` 必须先判 `typeof window === "undefined"` 直接 return。
**证据**：`src/test/setup.ts:6-12`
**namespace**: recall ｜ **category**: test

### 3.4 jsdom 缺失 API 在测试文件内 shim，禁改 setup
**规则**：`IntersectionObserver` 等 jsdom 缺的浏览器 API，在**用到它的测试文件内**注入 noop shim（`??=` 语义），不污染全局 setup。
**证据**：`src/pages/platforms/usePlatformsState.test.ts:16-24`
**namespace**: recall ｜ **category**: test

### 3.5 Tauri IPC mock 两条路
**规则**：① 服务层测试用 `mockIPC` / `clearMocks`（`@tauri-apps/api/mocks`）；② 组件/hook 测试用 `vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))` + `vi.mock` 掉 `domains/platforms/*` 的 async 派生函数（`mockResolvedValue`），跨 mock 共享桩用 `vi.hoisted`。
**证据**：`src/services/api.test.ts:2`；`src/components/platforms/PlatformCard.test.tsx:8-45`；`src/pages/platforms/usePlatformsState.test.ts:29-36`（`vi.hoisted`）
**namespace**: recall ｜ **category**: test

### 3.6 覆盖率分母只含纯逻辑层（阈值 80%）
**规则**：coverage `include` 仅 `src/utils/**`、`src/components/shared/**`、`src/services/api.ts`；巨石页面 / 编排容器**明确不纳入分母**，其纯函数逻辑必须抽到 `utils/` 或 `shared/` 后单独测。四项阈值 80。
**证据**：`vite.config.ts:55-75`（含决策注释）
**⚠️ 已 drift**：include 里的 `src/services/api.ts` **文件已不存在**（现为 `src/services/api/` 目录 + 14 个分片）；该 glob 当前匹配 0 文件，服务层实际未计入覆盖率分母。测试文件 `src/services/api.test.ts` 仍在（`import * as api from "./api"` 靠目录 index 解析）。
**namespace**: recall ｜ **category**: build

### 3.7 测试文件不参与 `tsc`
**规则**：`tsconfig.json` exclude 掉 `**/*.test.ts(x)` / `src/test`，故 `yarn build` **不会**发现测试里的类型错误；测试类型问题只能靠 `yarn test` 暴露（vitest 走 esbuild 不做类型检查 → 实际上无人做）。
**证据**：`tsconfig.json:34-40`
**namespace**: recall ｜ **category**: build

---

## 4. 架构边界

### 4.1 TS 类型真值源在 Rust，`generated/` 禁手改（**core 候选**）
**规则**：`src/services/api/types/generated/**`（59 文件）由 `yarn gen:types`（= `cargo test -p aidog_core export_bindings`）从 Rust struct 经 ts-rs 生成，**禁手改**。改 Rust 结构后必须重跑 `yarn gen:types`。只有 6 类情形可手写进 `types/manual.ts`（锁定 enum / camelCase DTO / 越界 crate / 前端派生 / 已知 drift 豁免 / 待核实）。
**证据**：`package.json:16`（`gen:types` script）；`src/services/api/types/generated/Platform.ts:1`（ts-rs "Do not edit" 头）；`src/services/api/types/manual.ts:1-17`（6 节分类说明）；`src/services/api/types.ts:1-8`（barrel）
**namespace**: **core** ｜ **category**: ts-rust-boundary

### 4.2 无 react-router：导航 = 本地 state + `navGuard` 注册表（**core 候选**）
**规则**：导航是 `App.tsx` 的 `activeNav` state（形如 `"settings/claude"`，`split("/")[0]` 定页、后缀定 tab）+ `AppSettings` tab prop。任何页面切换必须走 `requestNavigation(proceed)`；有未保存改动的页用 `registerNavGuard` 注册（单例、后注册覆盖、cleanup 只在自己仍是 active 时清）。**禁原生 `confirm` / `beforeunload`**（破坏 Tauri）。
**证据**：`src/utils/navGuard.ts:1-39`（含设计说明注释）；`src/App.tsx:60,131-143,157-159`
**namespace**: **core** ｜ **category**: arch

### 4.3 侧栏配置驱动 + labelKey
**规则**：导航项在 `App.tsx` 的 `BASE_NAV: NavItem[]` 常量里声明（`id` / `icon` / `labelKey` / `section` / 可选 `children[].group`），**文案一律走 `labelKey`/`group` 的 i18n key 字面量**，不写死中文。隐藏项靠 `navItems.filter` 按开关剔除（logs / notifications）。
**证据**：`src/App.tsx:26-57,146-156`
**namespace**: recall ｜ **category**: frontend

### 4.4 页面切换用 `key={effectiveNav}` 强制重挂
**证据**：`src/App.tsx:184`（`<div className="animate-fade-in" key={effectiveNav}>`）
**namespace**: recall ｜ **category**: frontend

### 4.5 deep-link 分发：CustomEvent + `window.__aidogDeepLink` 缓存
**规则**：后端 emit `aidog-deep-link` `{entity, action, data}` → `App.tsx` 二次分发为 `aidog:<entity>` window CustomEvent，**同时**写 `window.__aidogDeepLink[entity]`（per-entity last-write-wins，非队列），目标页 mount 时取一次并删 key（因页面是条件挂载，事件会在未 mount 时丢失）；platform/mcp/skill 三个 entity 额外 `setActiveNav` 触发挂载。新增 deep-link entity 必须同时补这两侧。
**证据**：`src/App.tsx:105-129`
**namespace**: recall ｜ **category**: frontend

### 4.6 全局状态只有一个 Context：`AppContext`（locale + themeMode）
**规则**：跨页共享状态**只有** `AppContext`（`locale` / `themeMode` + setter + `reloadFromDB`），经 `useApp()` 消费（未包 Provider 直接 throw）。其余状态一律页内 `useState` / 页目录下 `useXxx` hook，**无 redux/zustand/jotai/tanstack-query**。
**证据**：`src/context/AppContext.tsx:250,325-440`（唯一 createContext）；`package.json` dependencies 无状态库
**namespace**: recall ｜ **category**: arch

### 4.7 设置持久化：localStorage 同步兜底 + DB 权威 + 首启迁移
**规则**：首渲染同步读 `localStorage["aidog-settings"]`（防白屏），启动 effect 再从 DB（`settingsApi.get("app", "theme"/"locale")`）覆盖；DB 缺 theme 行 → 一次性物化写入。写入双写（localStorage + DB），DB 失败仅静默。脏/legacy locale 经 `normalizeLocale` 归一到 `zh-Hans`，防 `t(\`lang.${locale}\`)` 落空显裸 key。
**证据**：`src/context/AppContext.tsx:222-227,266-278,284-323,330-361`
**namespace**: recall ｜ **category**: frontend

---

## 5. 构建

### 5.1 命令集
- `yarn build` = `tsc && vite build`（唯一类型门禁，无 eslint）
- `yarn test` = `vitest run`（27 文件）；`yarn test:cov` 带 80% 阈值
- `yarn check:i18n` = `node scripts/check-i18n.mjs`
- `yarn gen:types` = `cd src-tauri && cargo test -p aidog_core export_bindings`
- `yarn check:statusline-runtime` / `yarn test:statusline-golden` / `yarn version:check`
**证据**：`package.json:8-19`
**namespace**: recall ｜ **category**: build

### 5.2 双入口打包（**core 候选**，易踩）
**规则**：Vite 有两个 HTML 入口：`main → ./index.html`、`popover → ./popover.html`（popover 独立窗口，`src/popover.tsx` + `src/styles/popover.css`）。新增全局 Provider / 主题初始化必须**两个入口都接**，只改 `main.tsx` 会让 popover 窗口漏配。
**证据**：`vite.config.ts:35-40`；`src/popover.tsx`；`src/styles/popover.css`
**namespace**: **core** ｜ **category**: build

### 5.3 `@` alias 双写同步
**规则**：`@/*` → `./src/*` 在 `vite.config.ts` 与 `tsconfig.json` 各有一份，改一处必改另一处（shadcn 组件靠它解析 `@/components/ui` / `@/lib/utils`）。
**证据**：`vite.config.ts:12-16`（含 `// ponytail:` 同步注释）；`tsconfig.json:20-22`
**namespace**: recall ｜ **category**: build

### 5.4 dev server watch 必须排除 worktree
**规则**：`server.watch.ignored` 含 `**/src-tauri/**` 与 `**/.worktrees/**`（task worktree 内 dist 改动会触发误 reload）。
**证据**：`vite.config.ts:27-31`
**namespace**: recall ｜ **category**: build

---

## 6. i18n（frontend 型探针）

### 6.1 新 key 必须 8 语言齐 + 跑 `yarn check:i18n`（**core 候选**）
**规则**：`src/locales/` 8 个 locale（`zh-Hans` / `en-US` / `ar-SA` / `fr-FR` / `de-DE` / `ru-RU` / `ja-JP` / `es-ES`）；新增 key 必须同时补齐 8 份，改完跑 `yarn check:i18n`（退出码非 0 即 fail）。CI **不跑**此脚本，只能靠人/agent 自觉。
**证据**：`src/locales/index.ts:31-50`（`ALL_LOCALES`）；`scripts/check-i18n.mjs:28,161`
**namespace**: **core** ｜ **category**: i18n

### 6.2 check-i18n 的四类检查与盲区
**规则**：A = `t("字面量")` 静态 key 全 locale 覆盖；B = 8 locale key 集合必须等于并集；C = `t(\`模板\`)` 只输出清单供人工审计（**不能自动展开**）；D = `labelKey` / `group` 属性字面量数据源覆盖（堵 `t(变量)` 的盲区）。新增「配置驱动文案」（形如 `NAV_ITEMS[].labelKey`）时，若字段名不是 `labelKey` / `group`，D 检查扫不到，需同步扩 `scripts/check-i18n.mjs`。
**证据**：`scripts/check-i18n.mjs:5-22`（doc 注释）、`:36-46`（A/C 正则）
**⚠️ 已 drift**：`scripts/check-i18n.mjs:24` 注释仍写「规约见 `.trellis/spec/frontend/conventions.md`」——`.trellis/` 已迁 `.skein/`，该路径不存在。
**namespace**: recall ｜ **category**: i18n

### 6.3 locale 懒加载 + 平铺 key
**规则**：`zh-Hans` + `en-US` 同步打包（首屏 `t()` 立即可用），其余 6 语言 `dynamic import` 按需注入（Vite 拆 chunk）；切语言必须先 `await ensureLocaleLoaded(locale)` 再 `i18n.changeLanguage`。locale JSON 是**平铺单层对象**（`Object.keys` 直接当 key 集合），key 内含点号但不是嵌套结构。
**证据**：`src/locales/index.ts:59-68,80-95`；`src/context/AppContext.tsx:366-368`；`scripts/check-i18n.mjs:31-33`（`new Set(Object.keys(obj))`）
**namespace**: recall ｜ **category**: i18n

### 6.4 RTL 由 `AppContext` 写 `documentElement.dir`
**规则**：`RTL_LOCALES = ["ar-SA"]`；locale 变更 effect 内同时写 `document.documentElement.dir` 与 `.lang`，并把 locale 持久化到 DB（供后端 proxy 错误消息用）。加 RTL 语言只需扩 `RTL_LOCALES`。
**证据**：`src/locales/index.ts:52-57`；`src/context/AppContext.tsx:364-375`
**namespace**: recall ｜ **category**: i18n

### 6.5 app locale 与 docs locale 是两套标签，禁混用
**规则**：应用侧用 BCP 47（`zh-Hans` / `en-US` / …，8 个）；Rspress 文档站用短码目录（`docs/docs/{zh,en,ja,fr,de,es,ru,ar}` + `docs/i18n.json`）。改任一侧不要顺手统一另一侧。
**证据**：`src/locales/index.ts:31-50`；`docs/rspress.config.ts:11-40`；`ls docs/docs/`
**namespace**: recall ｜ **category**: i18n

---

## 7. 样式与主题（frontend 型探针）

### 7.1 主题机制 = inline CSS 变量 + `data-mode`，`dark:` utility 是死代码（**core 候选**）
**规则**：`applyTheme(mode)` 只做两件事——`applyThemeVars(mono[mode])` 写 `documentElement.style.setProperty` **inline 变量** + `setAttribute("data-mode", mode)`；**从不 `classList.add("dark")`**。故 `globals.css:8` 的 `@custom-variant dark (&:is(.dark *))` 与 `:765` 的 `.dark {}` token 块全是死代码，任何 `dark:` Tailwind utility **永不生效**。判深色态必须看 `src/themes/mono.ts` 的 `dark` 块或 `:root[data-mode="dark"]` 选择器。
**证据**：`src/themes/index.ts:14-17`；`src/themes/types.ts:10-16`；`src/styles/globals.css:8,765`
**残留死代码**（旧规则记录的 2 处仍在）：`src/components/ui/alert.tsx:13`（`dark:border-destructive`）、`src/components/ui/field.tsx:120`（`dark:has-data-`）
**namespace**: **core** ｜ **category**: frontend

### 7.2 单一 mono 主题（旧「每主题 light/dark」已废）
**规则**：主题轴已收敛为**唯一 `mono` 主题 × mode（light/dark）**，`DEFAULT_MODE = "dark"`。旧持久化里的 `themeStyle` / `themeColor` 字段一律忽略。`ThemeDefinition` 要求 light/dark 键集相同（切换无残留，无需 clear）。
**证据**：`src/themes/index.ts:1-17`（`export const DEFAULT_MODE: ThemeMode = "dark"`，注释「唯一 mono 主题」）；`src/themes/` 仅 `index.ts` / `mono.ts` / `types.ts` / `useThemeMode.ts`；`src/context/AppContext.tsx:262-265`
**namespace**: recall ｜ **category**: frontend

### 7.3 CSS reset / UA 补丁必须写进 `@layer base`（**core 候选**）
**规则**：`globals.css:4-6` 用分层导入（`@layer theme, base, components, utilities;` + `@import "tailwindcss/utilities" layer(utilities)`），**preflight 未启用**。CSS cascade layer 规范下**任何裸写（不在 `@layer` 内）规则都压过任意 layered 声明，与特异性无关**——裸写 `* { padding: 0 }` 会让 shadcn 的 `px-4 py-2` 全失效（按钮文字贴边），裸写 `button { color: ... }` 会让 `text-*-foreground` 全失效（金底浅白 1.68:1）。补 UA reset 必须包进 `@layer base { }`。
**证据**：`src/styles/globals.css:3-6,9-21,23-52`（三段红字注释即此规则的实证记录）
**namespace**: **core** ｜ **category**: frontend

### 7.4 `color-scheme` 必须随 `data-mode` 显式覆盖，禁写 `light dark`
**规则**：`:root` 的 `color-scheme` 控制 UA 弹层（`<datalist>` 下拉、滚动条、autofill 底、`datetime-local` 原生日历）明暗。主题不跟系统 → 不能用 `light dark` 自动配对，必须在 `:root[data-mode="dark"]` 显式覆盖。组件内需要感知时用 `useThemeMode()`（MutationObserver 监听 `data-mode`，不拉 settings 上下文，避免给 Platforms 页引入额外 re-render）。
**证据**：`src/styles/globals.css:64-68`；`src/themes/useThemeMode.ts:4-35`
**namespace**: recall ｜ **category**: frontend

### 7.5 语义色 token 必须成对，`--accent` 本值禁改
**规则**：任何 `bg-X` 语义 token 必须配达标对比度的 `--X-foreground`。本项目 `--accent` 语义**不等于** shadcn 惯例（这里当品牌强调金色用，被 `.btn-primary` 渐变 / checkbox `accent-color` / `.badge-accent` 依赖），改 `--accent` 本值会连带破坏依赖方——只能调 `--accent-foreground`。另有一组 `--color-success/warning/danger/info(-bg)` 语义状态色（萤火虫莫兰迪去饱和），供 `colorScale` / `StatChip` / `BalanceBar` 统一引用。
**证据**：`src/styles/globals.css:71-85`（语义状态色定义 + `ponytail:` 对比度退化说明）、`:120` 起 legacy token alias 层
**namespace**: recall ｜ **category**: frontend

### 7.6 样式主力仍是 inline style，shadcn 迁移未完成
**事实**（不写成规则，供判断上下文）：`style={{` 2540 处 vs `className=` 749 处；`components/ui/` 已建 24 个 shadcn 原语、265 处 `@/components/ui/*` 导入；仍有 8 处 legacy `.btn` className。改样式时先看目标文件走哪一套，不要在同一组件里混搭。
**证据**：grep 计数；`src/styles/globals.css:120` 起「Legacy token aliases → shadcn (compat for business pages pre-migration)」

### 7.7 modal 必须 `createPortal(document.body)`
**规则**：祖先含 `transform` / `backdrop-filter`（liquid glass）会让 `position: fixed` 退化为相对祖先定位，弹窗只在 page 内居中。自建 modal 一律 `createPortal(..., document.body)`（13 个文件在用）。
**证据**：`src/components/settings/UnsavedChangesModal.tsx`、`src/components/UpdatePromptModal.tsx`、`src/pages/Skills/SkillModals.tsx` 等 13 处 `createPortal`；`CLAUDE.md` UI 章节
**namespace**: recall ｜ **category**: frontend

---

## 8. shadcn / Radix 用法（frontend 型探针）

### 8.1 Radix `Select` 禁 `value=""`，用 `__none__` 哨兵
**规则**：`SelectItem value=""` 会抛错。空值用 `__none__` 常量，`value={!v ? "__none__" : v}` + `onValueChange={x => x === "__none__" ? undefined : x}` 映射回。
**证据**：`src/components/settings/editors/FieldRenderer.tsx:82-87`、`src/components/settings/CodingToolsSettings.tsx:453-458,491-496`、`src/domains/groups/PlatformPicker.tsx:105-109`
**namespace**: recall ｜ **category**: shadcn
**旧规则位置**：`recall/shadcn/rule-41.md` — **仍符合**。但旧规则举的例子 `src/pages/Logs/primitives.tsx:12-13` 现已无 `__none__`（grep 零命中），案例行号需换成上面三处。

### 8.2 Radix `Select` value 只收 string → number 双向映射
**规则**：`value={String(n)}` + `onValueChange={v => onChange(Number(v))}`。
**旧规则位置**：`recall/shadcn/rule-42.md`
**⚠️ 案例已失效**：旧规则引 `src/pages/Logs/primitives.tsx:374`（Pagination pageSize），该文件当前无此模式（`grep '__none__' src/pages/Logs/` 零命中）。规则本身（Radix API 约束）仍成立，但需重新取证或降级。

### 8.3 `Dialog.open` 用显式 `!== null`
**规则**：Promise-resolve 型或对象型 state 控制弹窗时写 `open={state !== null}`，不靠隐式布尔转换。
**证据**：12 处 `open={... !== null}`
**namespace**: recall ｜ **category**: shadcn
**旧规则位置**：`recall/shadcn/rule-43.md` — **仍符合**。

### 8.4 Radix `Dialog` 必须含 `DialogTitle`（a11y）
**规则**：自定义 header 时用 `<DialogTitle className="sr-only">` 保语义。
**证据**：2 处 `DialogTitle className="sr-only"`
**namespace**: recall ｜ **category**: shadcn
**旧规则位置**：`recall/shadcn/rule-45.md` — **仍符合**（案例文件 `SegmentEditModal.tsx` 需复核路径）。

---

## 9. 数值 / 格式化

### 9.1 格式化统一走 `src/utils/formatters.ts`
**规则**：数值/时间/字节格式化一律从 `utils/formatters.ts` 导入，禁页内重复定义 `formatNumber` 等。
**证据**：`src/utils/formatters.ts`（127 行）+ `src/utils/formatters.test.ts`；`CLAUDE.md` UI 章节
**namespace**: recall ｜ **category**: frontend

### 9.2 `utils/` = 无 React 依赖的纯函数，且必须配 `.test.ts`
**事实**：`src/utils/` 10 个模块中 9 个有同名 `.test.ts`（唯一无测试的是 `motion.ts`，纯常量）。它们同时是 coverage 分母主体（80% 阈值）。
**证据**：`ls src/utils/`；`vite.config.ts:60`
**namespace**: recall ｜ **category**: test

---

## 10. 异步与缓存

### 10.1 `defaults.ts` 模块级 `docPromise` 单次 RPC 缓存（**core 候选**）
**规则**：`src/domains/platforms/defaults.ts` 的 5 个 `getDefaultXxx` 函数**全部 async**，共享模块级 `docPromise`（首次调用发一次 `getDefaultsJson` invoke，之后复用同一 Promise）。**所有 caller 必须 `await` / `.then`**——TS 编译能捕获漏 await。测试需重置缓存时调 `__resetDefaultsCacheForTests()`（生产代码禁调）。
**证据**：`src/domains/platforms/defaults.ts:86-107`（`docPromise` + `loadDoc`）、`:110-112`（`__resetDefaultsCacheForTests`）；`src/context/AppContext.tsx:333-337`（启动期 best-effort 预热 `buildProtocolsFromPresets` / `buildClientTypesFromPresets`）
**namespace**: **core** ｜ **category**: frontend

### 10.2 preset 分支选择：`pickBranch`（两分支）vs `pickModelsBranch`（三分支）
**规则**：`endpoints` / `model_list` 走 `pickBranch`（`default` / `coding_plan`，cp 缺失回落 default）；`models` 走 `pickModelsBranch`（`default` / `coding_plan` / `peak`，**coding_plan 优先于 peak**——cp 是端点维度硬约束，peak 是时段维度软切换）。新增分支维度必须挑对 picker。
**证据**：`src/domains/platforms/defaults.ts:115-136`（两函数 + 优先级注释）
**namespace**: recall ｜ **category**: domain

### 10.3 前端 `isCurrentlyPeak` 与 Rust 判定跨层对称
**规则**：`src/utils/peakHours.ts` 的窗口判定（含 `start_minute` / `end_minute` / `days_of_month` / `models` / `start_at` / `end_at` 六个可选字段）必须与 Rust `gateway::peak_hours::is_in_peak_window` 逐字段对称，改一侧必改另一侧。`PeakWindow` TS 类型注释里逐字段标注了 Rust 对应字段。
**证据**：`src/domains/platforms/defaults.ts:10-41`（每字段带「与 Rust `PeakWindow.xxx` 对称」注释）；`src/utils/peakHours.ts` + `peakHours.test.ts`
**namespace**: recall ｜ **category**: cross-layer

### 10.4 半时区必须走绝对分钟运算，禁浮点 hour
**规则**：时区换算一律用绝对分钟（`hour*60+minute`）再拆回整数 `hour`/`minute`；禁按整小时换算产生 `start_hour: 8.5`——Rust 侧 `i32` 反序列化失败会 `.ok()?` **静默丢弃整个窗口**，用户配置无声失效。加载存量脏数据时在前端 parse 层归一（浮点 hour 拆成整数 hour+minute），不改后端 serde 类型。
**证据**：`src/domains/platforms/defaults.ts:19-24`（`start_minute` / `end_minute` 字段 + 向后兼容说明）；旧规则 `recall/frontend/time-zone-minute-arithmetic.md`、`recall/frontend/dirty-float-hour-normalization.md`
**namespace**: recall ｜ **category**: frontend

### 10.5 macOS WKWebView 禁 HTML5 `onDrop`
**规则**：Tauri 文件拖拽必须用 `getCurrentWebview().onDragDropEvent()`；macOS WKWebView 的 HTML5 `drop` 事件不触发。该事件是 **webview 级**，payload 不含 DOM target，需区分 modal 子区域时靠 HTML5 `onDragEnter` 标记 + ref（best-effort，不可靠）。
**旧规则位置**：`recall/frontend/auto-fix-downgrade-37.md`、`cpa-drag-import-22.md`
**⚠️ 需复核**：本次扫描未在 `src/` 找到 `onDragDropEvent` 调用点（未逐文件确认，可能在 CcSwitchImport / Sub2ApiImport 内）。规则本身是平台事实（不可从代码推导），建议保留但标 `protected`。
**namespace**: recall ｜ **category**: frontend

---

## 11. 旧规则 drift 汇总（archive 内路径 + 不符之处）

| archive 路径 | 不符之处 | 处置建议 |
|---|---|---|
| `recall/shadcn/rule-42.md` | 案例 `src/pages/Logs/primitives.tsx:374` 的 Pagination pageSize `String()/Number()` 映射在当前代码**已不存在** | 规则（Radix API 约束）保留，换取证或降 recall 弱化 |
| `recall/shadcn/rule-41.md` | 案例 `src/pages/Logs/primitives.tsx:12-13` 的 `NONE` 常量**已不存在**（`__none__` 现在三处：FieldRenderer / CodingToolsSettings / PlatformPicker） | 规则保留，换案例 |
| `recall/frontend/theme-dark-class-dead-code.md` | 规则**完全成立**，且列的 2 处残留（`field.tsx:120`、`alert.tsx:13`）**至今仍在**未清 | 提到 core，附「残留 2 处待清」 |
| `recall/frontend/shadcn-infra-31.md` | 描述 `applyTheme` + `setProperty` — 与 `src/themes/index.ts:14-17` 一致 | 保留，但与 7.1 重复，建议合并 |
| `recall/frontend/shadcn-infra-30.md`（CSS var alias 层） | `globals.css:120` 起的 legacy alias 层**仍在**（迁移未完成，别名未删） | 保留 |
| `recall/frontend/trellis-18.md`（"前端 conventions 强制规则"） | 仅有元描述（何时读/谁读/代价），**无实际规则正文** | 废弃，本次重建取代 |
| `recall/cross-layer/trellis-20.md` | 同上，纯元描述无正文 | 废弃 |
| `recall/i18n/trellis-19.md` | locale 标签跨层一致性 — 与 6.5 相符但未覆盖 docs 短码维度 | 更新，并入 6.5 |
| `recall/frontend/platform-creation-entry-consolidation.md` | 未验证（`src/pages/CliProxy/index.tsx` 存在，但未逐行确认「建平台行」按钮唯一性） | 需复核 |
| `recall/frontend/modal-state-architecture.md` | 引 `usePlatformForm` hook 的 `showPaste`/`setShowPaste` — 文件 `src/pages/platforms/usePlatformForm.ts` 存在，未逐行确认 | 需复核 |
| `recall/test/rule-48.md`（shadcn 迁移改行为断言） | **完全成立**，`PlatformCard.test.tsx:1-2` 文件头就是它 | 提到 core（= 本文 3.2） |
| **CLAUDE.md 自身 drift** | ① 「18 个测试文件」实为 27；② 「`src/themes/` 每主题 light/dark CSS 变量」已收敛为单一 mono；③ 「`services/api.ts`」已是目录；④ 项目结构段的 `pages/` 清单缺 `CliProxy` / `RequestLog` | 归 main 决定是否同步改 CLAUDE.md |
| **代码内 drift** | `vite.config.ts:62` coverage include `src/services/api.ts` 匹配 0 文件（应为 `src/services/api/**/*.ts`） | 建议开单独 fix task，本次不改 |
| **代码内 drift** | `scripts/check-i18n.mjs:24` 注释指向已不存在的 `.trellis/spec/frontend/conventions.md` | 同上 |

---

## 12. core 提名（软预算 1000 字符，共 7 条）

按「后续必再踩 + 命令式可执行」筛。建议正文压到一句话 + 一个证据锚点：

1. **[core/frontend]** 主题只走 `data-mode` inline CSS 变量，`dark:` utility 永不生效——判深色态看 `themes/mono.ts` 的 `dark` 块或 `:root[data-mode="dark"]`。（7.1）
2. **[core/frontend]** `globals.css` 补 CSS reset / UA 补丁必须包进 `@layer base {}`，裸写会反压 utilities 层让所有 `px-*` / `text-*` 失效。（7.3）
3. **[core/ts-rust-boundary]** `services/api/types/generated/**` 由 `yarn gen:types` 生成，禁手改；手写类型只进 `types/manual.ts`。（4.1）
4. **[core/arch]** 无 react-router：页面切换一律经 `requestNavigation()`，脏页用 `registerNavGuard`；禁原生 `confirm` / `beforeunload`。（4.2）
5. **[core/i18n]** 新 i18n key 必须 8 locale 齐 + 跑 `yarn check:i18n`（CI 不跑，靠自觉）。（6.1）
6. **[core/test]** 组件测试从 `src/test/render` 导入 `render`，断 i18n key 与行为（getByText/getByRole/userEvent），禁 `toHaveClass` 与快照。（3.1 + 3.2 合一条）
7. **[core/frontend]** `domains/platforms/defaults.ts` 5 个 getter 全 async 共享 `docPromise`，所有 caller 必须 await。（10.1）

**次选（若预算紧可降 recall）**：5.2 双入口（popover 漏配）——踩到概率中等但后果明显。

---

## 13. 本次未覆盖 / 需要后续确认

- `src/components/settings/editors/`（14 文件）与 `StatusLineSection/` 子树未逐文件读，`statusline-*.ts` 三件套（gen / runtime / segments）+ golden 测试机制未展开。
- `scripts/build-statusline-runtime.mjs --check` 与 `scripts/statusline-golden/build.mjs check` 的门禁语义未验证。
- `docs/`（Rspress）除 locale 结构外未深入；docs 侧写作约定（frontmatter / sidebar 生成）未扫。
- `src/components/SortableList.tsx` + `@dnd-kit` 拖拽约定未扫。
- 旧规则中标 `需复核` 的 3 条（10.5 拖拽、platform-creation-entry、modal-state-architecture）。
