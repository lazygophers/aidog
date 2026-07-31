---
title: frontend
category: frontend
keywords: [脏数据,浮点,归一,Number.isInteger,splitFraction,兼容性,tauri,drag,drop,wkwebview,html5,ondragdropevent,modal,state,architecture,PlatformEditForm,usePlatformForm,CpaImportModal,SmartPasteModal,表单设计,状态管理,时区模式,单一真值源,多组件一致性,cli-proxy,平台创建,入口收敛,CliProxy,语义色,token,foreground,对比度,contrast,wcag,accent,时区,换算,分钟精度,半时区,+5:30,shiftClock,modulo,tailwind,cascade-layer,unlayered,layer,preflight,cascade,css,var,alias,live-resolution,migration,globals.css,shadcn,theme,runtime,setProperty,frontend,react,component,hook,crud,刷新链,api,i18n,domains]
status: active
inclusion: auto
---

## 脏数据入库时归一 — 浮点 hour 拆分为整数 hour+minute

系统升级或跨版本迁移中，存量数据可能包含脏数据。例如，旧版本按整小时换算时产生 `start_hour: 8.5`（半时区换算残留），新版本期望整数。前端 parse 层应负责吸收这类脏数据。

## 陷阱：后端 migration 改 serde 类型成本高，数据污染持久

旧版本：`peak_hours` 整小时换算，半时区用户产生 `start_hour: 8.5` 写入 JSON。后端声明 `start_hour: i32`，JSON 反序列化失败 → 静默丢弃窗口。

修复选项：
- ❌ 改后端存储类型为 f64 —— 一个不再产生的格式永久兼容，代价不成比例
- ✅ **正解**：前端 parse 层吸收 —— 加载时拆分，用户下次保存自动正规化

## 前端读取路径归一（关键）

### MUST 单点归一（parse 层）

```ts
/** 存量非整数 start_hour/end_hour（半时区旧逻辑产物）拆为 hour+minute。
 *  整数值原样不动。 */
export function normalizeWindow(w: PeakWindow): PeakWindow {
  let result = w;
  if (!Number.isInteger(w.start_hour)) {
    const { hour, minute } = splitFraction(w.start_hour, w.start_minute);
    result = { ...result, start_hour: hour, start_minute: minute };
  }
  if (!Number.isInteger(w.end_hour)) {
    const { hour, minute } = splitFraction(w.end_hour, w.end_minute);
    result = { ...result, end_hour: hour, end_minute: minute };
  }
  return result;
}

/** 纯函数：浮点时刻拆为整数 hour+minute。 */
function splitFraction(h: number, existingMinute: number | undefined): { hour: number; minute: number } {
  const hour = Math.floor(h);
  const extraMinutes = Math.round((h - hour) * 60);
  return shiftClock(hour, (existingMinute ?? 0) + extraMinutes, 0);
}
```

### MUST 调用路径（两处 parse 出口）

```ts
// src/services/api/platforms.ts:204-216
export function parsePlatformPeakHours(raw: unknown): PeakWindow[] {
  // ... schema 校验 ...
  return windows.map(normalizeWindow);  // ← 追加归一
}

// src/services/api/platforms.ts:272
export function parsePlatformTimeModels(raw: unknown): TimeModelRule[] {
  // ... 循环处理 rule ...
  return rules.map(r => ({
    ...r,
    windows: r.windows.map(normalizeWindow),  // ← 追加归一
  }));
}
```

## 单测覆盖（脏数据拆分规则）

- 整数 hour 原样不变
- 8.0 视为整数不变
- 8.5 拆分为 8:30
- 已有 start_minute 时叠加进位
- start 与 end 各自独立归一

## 反例 / 常见错误

| 错误                          | 为什么错                                        |
| ----------------------------- | ----------------------------------------------- |
| 后端改 serde 类型为 f64        | 永久兼容一个不再产生的格式，代价不成比例        |
| 直接 Math.floor(h)，丢失分钟 | 8.5 → 8，漏掉 30 分钟，显示错误               |
| 判据用 `h % 1 === 0`         | 浮点舍入误差覆盖不全    |
| 忘记叠加已有 start_minute   | 8.5 + 已有 40m → 应是 9:10，但只变成 8:30    |

## 适用

- 版本升级中的数据兼容性问题
- 存量脏数据前端吸收而非后端永久兼容

## 关联

[[time-zone-minute-arithmetic]]、[[modal-state-architecture]]

## Tauri 拖拽事件 API（macOS WKWebView 限制）

Tauri 前端实现文件拖拽导入时，必须使用 Tauri `onDragDropEvent`，禁用 HTML5 onDrop（macOS WKWebView 不支持）。

## MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop

macOS WKWebView 的 HTML5 `drop` 事件不触发。Tauri `getCurrentWebview().onDragDropEvent()` 绕过此限制。

## 范本

```typescript
useEffect(() => {
  let unlisten: (() => void) | undefined;
  let cancelled = false;
  getCurrentWebview()
    .onDragDropEvent((event) => {
      const { type } = event.payload;
      const paths = (event.payload as { paths?: string[] }).paths ?? [];
      if (type === "drop") {
        // paths[] 处理
      }
    })
    .then((fn) => { if (cancelled) fn(); else unlisten = fn; })
  return () => { cancelled = true; unlisten?.(); };
}, [isOpen]);
```

## event.payload.type

- enter/over: paths[] → 高亮判断
- drop: paths[] → 取目标文件
- leave/cancel: 清高亮

## 约束

- 禁混 HTML5 onDrop（macOS WKWebView 不触发）
- MUST unlisten（cleanup 调 unlisten()，否则泄漏）
- listener 依赖最小化（避免 state churn 致频繁 re-listen）

## 适用

Tauri 文件拖拽导入、跨平台拖拽

## 关联

[[modal-state-architecture]]

## PlatformEditForm Modal 架构模式

PlatformEditForm Modal 架构需要区分两类：直接灌表单 Modal 与跨表单 Modal，state 位置与传递方式完全不同。

## 两类 Modal 区分

### 直接灌表单 Modal（SmartPasteModal 模式）
- **State 位置**: `usePlatformForm` hook 内定义 `showPaste` + `setShowPaste`
- **传递方式**: 通过 `PlatformPasteCtx` 传递 `setShowPaste` 给 `applyPaste` 等函数
- **关闭时机**: `onClose` 直接调用 `setShowPaste(false)`（modal 组件内处理）
- **适用场景**: Modal 的 onApply 直接操作表单字段（灌入 name/apiKey/models 等）

### 跨表单 Modal（CpaImportModal 模式）
- **State 位置**: `PlatformEditForm` 组件本地定义 `showCpaImport`（**不在 hook 内**）
- **传递方式**: **不加进 PlatformPasteCtx**
- **关闭时机**: `onApplied` 回调内由调用方处理 `setShowCpaImport(false)`（modal 返回原始数据，父级决策）
- **适用场景**: Modal 的 onApplied 返回中间数据（如 `MappedPlatform[]`），由父级分派逻辑（单条灌表单 vs 多条批量创建）

## 架构原则

1. **Modal 直接操作表单字段 → state 放 hook，通过 PlatformPasteCtx 传 setter**
2. **Modal 返回中间数据由父级决策 → state 放组件本地，关闭由 onApplied 回调处理，不加 PlatformPasteCtx**

## 后续新 Modal 决策树

新 Modal (如 Sub2Api)
├─ onApply 直接填表单字段？
│  └─ 是 → SmartPasteModal 模式（state 在 hook + PlatformPasteCtx）
└─ 否（返回中间数据由父级决策）？
   └─ 是 → CpaImportModal 模式（state 在组件本地 + onApplied 关闭）

## 验收

- [ ] grep `showCpaImport` / `showPaste` 在 PlatformEditForm 组件本地定义
- [ ] grep `PlatformPasteCtx` 不含跨表单 modal 的 state setter
- [ ] 跨表单 modal 的 `onApplied` 回调包含关闭逻辑

## 关联

[[tauri-drag-drop-api]]、[[form-level-tz-state-sharing]]

## 表单级时区状态共用 — 单一 state 透传避免口径漂移

同一表单内多个组件展示同一类数据不同维度时，需要单一 state 透传避免口径漂移。

## 陷阱：各组件独立 state 导致口径漂移

PlatformEditForm 编辑单个平台。peak_hours 与 time_models 都含「时段」结构，都需时区显示切换。若各自独立 state：
- 用户在 peak_hours 切到 UTC+0
- time_models 仍显示本地时区
- 同一时段在两处显示基准不同 → 用户困惑

❌ 各自独立 state（禁用）
```tsx
<PeakHoursSection tzMode={peakHoursTz} setTzMode={setPeakHoursTz} />
<ModelsMatrixSection tzMode={modelsTz} setTzMode={setModelsTz} />
```

## MUST 单一真值源（表单级 state）

✅ **表单级单一 state 透传**

```ts
// usePlatformForm.ts：表单级 hook
export function usePlatformForm(...): PlatformFormState {
  // ✅ 时区展示模式：表单级单一 state（默认本地）
  const [windowsTz, setWindowsTz] = useState<"local" | "utc">("local");
  
  return {
    // ...
    peakHours, setPeakHours, windowsTz, setWindowsTz,  // 单一 state 对外透传
  };
}
```

## 适用

- 表单内多个子组件需同步状态的场景
- peak_hours + time_models 编辑器一致性

## 关联

[[time-zone-minute-arithmetic]]、[[dirty-float-hour-normalization]]

## cli-proxy 平台创建入口唯一性

cli-proxy 平台的创建路径需要统一化，唯一入口是 CliProxy 页的「建平台行」按钮。

## 约束

cli-proxy 平台的唯一创建入口是 **CliProxy 页 src/pages/CliProxy/index.tsx 的「建平台行」按钮**。PlatformEditForm 新建态禁带「从 cli-proxy 添加」旁路入口。

## 正解

- 添加平台表单（PlatformEditForm）只用于编辑现有平台
- 创建新 cli-proxy 平台必须走 CliProxy 页的按钮
- 该页按钮负责维护平台创建的入口单一性

## 反例

❌ 在 PlatformEditForm 新建态混入「从 cli-proxy 导入」选项 → 创建路径分裂
❌ 允许多个地方可以触发 cli-proxy 平台创建 → 维护成本增加

## 适用

- CLI Proxy 平台管理流程设计
- 添加平台表单重构

## 关联

[[i18n-key-deletion-safety]]、[[modal-state-architecture]]

## 语义色 token 必须成对达标对比度

任何语义色 `bg-X` token 都必须配达标对比度的 `-foreground` token。本项目 `--accent` 被当品牌强调金色用，改坏本值会连带破坏多处依赖。

## MUST 约束

修对比度缺陷时**禁改 `--accent` 等语义色 token 的值本身**，只能改配对的 `-foreground` token。

## 陷阱

补 preflight 缺失的 UA reset 时若改了语义色 token 本值（如 `--accent` 色），会连带破坏 `.btn-primary` 渐变 / checkbox `accent-color` / `.badge-accent` 等多处依赖。

## 正解

逐处核对 `bg-X`/`-foreground` 组合对比度，修改 foreground 侧色值不修改 accent/primary 本值。

## 案例

frontend-compositing-purge task 对比度审计：dark `--accent-foreground` 1.8:1、light `--primary-foreground` 2.62:1，均改 foreground 侧修复。

## 关联

[[tailwind-cascade-layer-unlayered]]

## 时区换算硬约束 — 绝对分钟精度

前端时区显示/输入交互需与服务端一致，半时区用户（印度 UTC+5:30 等）填写时段时必须绝对分钟精度。

## MUST 换算公式（单位：分钟）

```ts
export function shiftClock(
  hour: number, 
  minute: number, 
  offsetMinutes: number
): { hour: number; minute: number } {
  // ✅ 绝对分钟计算：UTC 总分钟 + 偏移 → 模 1440 归一 → 拆回 hour:minute
  const m = (((hour * 60 + minute + offsetMinutes) % 1440) + 1440) % 1440;
  return { hour: Math.floor(m / 60), minute: m % 60 };
}

export function utcToDisplay(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, tzOffsetMinutes(mode));
}

export function displayToUtc(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, -tzOffsetMinutes(mode));
}
```

## 陷阱：按整小时换算产生非整数

半时区下 UTC `8:00` 换到本地是 `8 + 5.5 = 13.5 小时`，被写进 JSON 后后端解析失败 → 静默丢弃。

- ❌ 按整小时换算：UTC 8:00 + 5:30 = 13:30 → 截断为 hour=13, 丢失分钟
- ❌ 直写 hour=13.5 → JSON 解析炸裂

## 适用

- 前端时区显示/输入交互（peak_hours / time_models 编辑器）
- 任何跨时区时刻换算

## 关联

[[dirty-float-hour-normalization]]、[[form-level-tz-state-sharing]]

## Tailwind cascade layer: 裸写规则反压 layer 内 utility

Tailwind v4 项目里若分层导入 CSS，任何裸写（不在 `@layer` 块内）的规则优先级都高于 layered utility（CSS cascade layer 规范）。

## 陷阱

补 preflight 缺失的 UA reset（如 button/input/select 色继承）时若裸写在 globals.css 顶层，会反压 utilities 层 —— 所有 `text-*-foreground` utility class 失效。

## 正解

补 UA reset 规则必须包进 `@layer base {}` 块，与 globals.css 顶部声明的 layer 顺序对齐，禁裸写元素选择器规则。

## 检查

globals.css 顶部若见 `@layer <names>;` 声明 + `@import ... layer(...)`，改动前先确认新增规则是否包在对应 `@layer` 块内。

## 案例

frontend-compositing-purge task：commit c3f9515e 裸写 UA reset 引入 button 文字色失效 → ce3d5dd5 改为 `@layer base {}` 包裹修复。

## 适用

Tailwind v4 + cascade layer 项目，补 preflight/UA reset 规则时。

## 关联

[[semantic-token-foreground-pairing]]

## CSS var live resolution 别名层

CSS 变量改名迁移时，用 :root 别名层实现 live resolution，替代批量 sed 替换。

## 正解

1. 在 :root 定义别名：`--legacy: var(--shadcn);`
2. 所有引用用旧名 `--legacy`，实际指向新名 `--shadcn`
3. 迁移完成后删别名行（自动失效）

## 对比

| 方式 | 改动量 | 误伤风险 | 回滚 |
|------|--------|---------|------|
| sed 批量替换 | 700+ 行 | 高（误伤类似变量名） | 难 |
| 别名层 | 10 行 | 无（CSS 引用透明） | 易（删别名） |

## 案例

shadcn-infra task: 主题变量改名用别名层，globals.css 加 10 行 vs sed 700+ 行

## 适用

CSS 变量迁移、主题重构、大型 CSS 重构中间状态

## 关联

[[theme-token-runtime-switch]]

## shadcn token 运行时切换

shadcn 主题 token 在运行时动态切换时，用 `setProperty` inline 方式，无需 !important 覆盖。

## 正解

1. applyTheme 函数直接设置 CSS var：
   ```ts
   document.documentElement.style.setProperty('--background', 'new-value')
   ```
2. 或用 @theme inline :root 兜底（避免 !important 级联爆炸）

## 陷阱

- **陷阱**: 用 !important 强制覆盖 → 级联爆炸、难以维护
- **陷阱**: 依赖 @import 静态切换 → 不支持运行时

## 反例

❌ 用 !important 覆盖所有 token → 优先级混乱
❌ 依赖静态 @import → 运行时无法切换

## 案例

shadcn-infra task: 运行时主题切换用 setProperty inline，避免 !important

## 适用

shadcn 主题运行时切换、动态主题系统、CSS var 运行时更新

## 关联

[[css-var-alias-layer]]

## 前端 conventions 强制规则

前端代码变更必须遵循约定，确保与现有模式一致，减少增量成本。

## Directory Structure (MUST)

- 新页面必须放 `src/pages/<PascalCase>.tsx`
- 共享组件放 `src/components/`，分为 `shared/`（跨页）、`ui/`（shadcn 原语）、`settings/`（设置域）、`platforms/`（平台域）
- **业务派生逻辑放 `src/domains/<domain>/`** 各带 `index.ts` barrel（如 `domains/platforms`、`domains/groups`、`domains/shared`）
- 服务层 API 放 `src/services/api/` 目录，每个 resource 一个 namespace 文件（`platforms.ts`、`groups.ts` 等），由 `src/services/api/index.ts` barrel 统一导出
- i18n JSON 放 `src/locales/<locale>.json`
- Context provider 放 `src/context/`

## Component Patterns (MUST)

- 页面组件必须 `export function <PascalCase>()`，用 named export
- 共享组件同理: `export function <PascalCase>(props: <Name>Props)`
- Props interface 必须紧跟组件定义之后、函数签名之前
- 组件样式必须用 inline `style={{}}` + CSS class（glass/glass-surface/btn/input）
- 禁 CSS Modules / styled-components / CSS-in-JS — 本项目仅用 inline style + 全局 CSS class
- 列表渲染必须带 `key={item.id}`，禁用 index 作 key

## State Management (MUST)

- 全局设置（locale / theme）必须走 `AppContext` + `useApp()` hook
- 禁新建全局 store / Zustand / Redux
- 组件本地状态用 `useState`
- 设置持久化必须走 `localStorage` key `"aidog-settings"`
- 异步数据获取必须用 `useEffect(() => { load() }, [])` + `useState<boolean>` loading pattern

## CRUD 刷新链契约 (MUST)

- **全入口扫描**: `platformApi.delete` 等后端真删的 CRUD 入口的全调用点 MUST grep 扫齐，确认无遗漏
- **受影响 state 必刷**: 每入口 MUST 触发受影响 state 全量刷新链（platforms + epoch）
- **禁仅刷关联 state** 致被删实体 stale 残留
- **独立信号优先**: 跨 hook 协作刷新链 MUST 用独立 callback（如 `onPlatformDeleted`），禁复用宽语义 callback
- **hook 级回归测试**: 每入口写 hook 级 renderHook 回归测试

## API Layer (MUST)

- invoke 契约需严格遵守（泛型标注 / 集中 api/ 目录 / 字段名 snake_case）
- API namespace 必须按 resource 拆分（`platformApi` / `groupApi` / `proxyApi` 等）
- 入参类型必须用独立 interface
- 错误处理: try/catch 包裹，禁静默丢弃

## i18n (MUST)

- 所有用户可见文案必须用 `t("key")`
- 新增任一 key 必须 8 locale 全补（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）
- `t(变量)` 路径（labelKey/group 属性）新增时同步补 key
- **check 前必须跑 `node scripts/check-i18n.mjs` exit 0**

### Protocol Metadata 多语言 (MUST)

- metadata (name/desc) 多语言放 `platform-presets.json` 内嵌 `{name: {<locale>: "..."}, desc: {...}}`，非 `src/locales/`
- `src/domains/platforms/constants.ts` 的 `PROTOCOLS[].label` 硬编码保留为 fallback
- 用户可见 UI 派生本地化 label via `getProtocolLabel` 等 async helper

## Large File Split — facade 模式 (MUST)

>800 行文件统一走 facade + 子目录模式：

- **facade 保留同名 export**: 拆后 `<Xxx>.tsx` 退化为编排 facade（仅 mount hook + 子组件），MUST 保留原 `export function <PascalCase>` 签名
- **子目录 `<Xxx>/`**: 拆出的 hook + JSX 子组件放 `src/pages/<Xxx>/` 或 `src/components/.../<Xxx>/`
- **单 hook 抽 state+actions**: 抽一个 `use<Xxx>Data` hook 收全部 state
- **纯 .ts 数据表外迁用 re-export barrel（唯一例外）**

## 关联

[[tauri-drag-drop-api]]、[[modal-state-architecture]]、[[i18n-key-eight-locales]]
