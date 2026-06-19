---
id: S2
slug: frontend-badge-event
deliverable: D1
parent-task: 06-19-platform-card-last-test-badge
status: planned
execution-layer: sub-agent
isolation: worktree
depends-on: [S1]
blocks: []
estimated-tokens: 12000
---

# S2 · 前端常驻徽章 + 全局事件 + 跨页刷新

## 目标

PlatformCard 常驻渲染「最近测试」徽章（ok/fail + 耗时 + 相对时间）；任一测试完成派发 `aidog-platform-test-completed` 事件；Platforms 页监听后单卡刷新；Groups 批量测试每平台完 / Platforms 快速测试完 / ModelTestPanel 测完均派发。

## 产出

- `src/components/platforms/usePlatformCards.ts`：新增 `lastTestMap` state + `refreshLastTest(platformId)` + 事件监听（mount 注册 `aidog-platform-test-completed`，unmount 卸载）
- `src/components/platforms/PlatformCard.tsx`：新增常驻徽章 UI（props 增 `lastTest?: LastTestResult`）
- `src/pages/Platforms.tsx`：`load()` 内随平台列表拉取 `lastTestResult` 填 `lastTestMap`；`handleQuickTest` 测完派发事件（或直接调 refreshLastTest）；将 `lastTestMap` + `refreshLastTest` 经 cards 透传到 PlatformCard
- `src/pages/Groups.tsx`：`handleTestGroup` 中 `testOne` 每平台测完（patchRow 成功/失败后）派发 `aidog-platform-test-completed` { platformId }
- `src/pages/ModelTestPanel.tsx`：单次测试 resolve 后派发 `aidog-platform-test-completed` { platformId }（仅 onSuccess/onFail 终态，不含中途）

## 验证

```bash
yarn build
yarn check:i18n
```

期望输出:
- yarn build: tsc + vite 退出码 0（exhaustive 类型核）
- yarn check:i18n: 全绿（新增 key 必须 8 locale 补全）

手验（开发者 dev 跑）:
- Platforms 卡片点快速测试 → 徽章即时 ok/耗时
- Groups 批量测试跑完 → 切 Platforms 页，被测各卡徽章 = 批量结果
- 重启应用 → 徽章仍显示历史最近测试

## 资源

- 独占文件: `src/components/platforms/PlatformCard.tsx` `src/components/platforms/usePlatformCards.ts` `src/pages/Platforms.tsx` `src/pages/Groups.tsx` `src/pages/ModelTestPanel.tsx`
- 共享文件（S1 已改，S2 只读消费）: `src/services/api.ts`（用 `LastTestResult` 类型 + `platformApi.lastTestResult`）
- 端口 / 服务: 无
- 环境: 无
- 审批槽位: 否

## 依赖

| 上游 | 需要的产出 | 等待方式 |
| --- | --- | --- |
| S1 | `platformApi.lastTestResult(id)` + `LastTestResult` 类型存在于 api.ts | S1 验收通过后启动 |

## 执行细节

### 事件契约

```ts
// 派发（任何测试终态后）
window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: p.id } }));
// 监听（usePlatformCards mount）
useEffect(() => {
  const handler = (e: Event) => {
    const ce = e as CustomEvent<{ platformId: number }>;
    if (ce.detail?.platformId != null) refreshLastTest(ce.detail.platformId);
  };
  window.addEventListener("aidog-platform-test-completed", handler);
  return () => window.removeEventListener("aidog-platform-test-completed", handler);
}, [refreshLastTest]);
```
> 事件名 `aidog-platform-test-completed` 统一常量，照 `aidog-groups-changed` 先例。

### usePlatformCards 改动

- 新增 `const [lastTestMap, setLastTestMap] = useState<Record<number, LastTestResult | null>>({});`
- 新增 `refreshLastTest = useCallback(async (platformId) => { const r = await platformApi.lastTestResult(platformId); setLastTestMap(prev => ({ ...prev, [platformId]: r })); }, [])`
- return 增 `lastTestMap` / `refreshLastTest`
- 事件监听 effect（上）
- `handleQuickTest` 测完（成功/失败两路）末尾派发事件（等价于直接 refreshLastTest(p.id)，但走事件统一入口，便于他页触发同卡刷新）

### Platforms.tsx 改动

- `load()` 内 `list.forEach` 中除 usageStats 外追加 `platformApi.lastTestResult(p.id)` → `setLastTestMap`（有值才填，null 不填以省渲染）
- PlatformCard 渲染处增 `lastTest={lastTestMap[p.id] ?? undefined}`
- 透传 `lastTestMap` / `refreshLastTest` 经 cards hook

### PlatformCard 徽章 UI

- 位置：卡片角落或名字行尾（与 health 点不重叠；health 点已在 top:-3 right:-3，徽章放其下或名字旁）
- 内容：成功 → 绿 ✓ + `${duration_ms}ms` + 相对时间（"刚刚"/"3分钟前"，用现有 formatters / 简易相对时间）；失败 → 红 ✗ + 截断 error
- 无 lastTest → 不渲染
- 样式走主题 CSS 变量（`--color-success` / `--color-danger`），禁硬编码色值，禁 rgba fallback（见 memory theme-css-var-names）

### Groups.tsx 改动

`handleTestGroup` 内 `testOne`（Groups.tsx:663）的 try/catch 末尾、patchRow(ok|fail) 之后追加：
```ts
window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: gp.platform.id } }));
```
> 不在 Groups 自身维护 lastTestMap（那是 Platforms 页消费侧职责），Groups 只派发。

### ModelTestPanel.tsx 改动

单次测试 Promise resolve（拿到 ModelTestResult 后）派发事件 { platformId }。批量/随机模式逐条 resolve 派发。

### i18n

新增 key（如 `platform.lastTestOk` / `platform.lastTestFail` / `platform.lastTestAgo` 等，按实际用到补）须 8 locale 全补，跑 `yarn check:i18n` 全绿。若徽章文案复用既有 key（如 platform.testOk/testFail + 纯数值）则免新增。

### Dispatch Prompt

> ⛔ isolation: worktree

```
Active task: .trellis/tasks/06-19-platform-card-last-test-badge
# 派发参数: isolation: worktree

## 目标
前端常驻最近测试徽章 + aidog-platform-test-completed 全局事件 + 跨页单卡刷新（见 subtask 文件执行细节）。

## 已知
- 上游产出: platformApi.lastTestResult(id): Promise<LastTestResult|null>（S1 已实现，消费 src/services/api.ts）
- LastTestResult 字段: success/status_code/duration_ms/created_at/error
- model_test 已落 proxy_log，事件派发后 Platforms 监听刷新即可
- 先例: aidog-groups-changed CustomEvent 跨页通信
- health 点位 top:-3 right:-3（PlatformCard.tsx:143），徽章避让
- 约束: 禁硬编码色值走主题 CSS 变量；禁 rgba(255,255,255) fallback（memory theme-css-var-names）

## 工作目录与范围
- cwd: worktree 根
- 可改文件: src/components/platforms/PlatformCard.tsx, src/components/platforms/usePlatformCards.ts, src/pages/Platforms.tsx, src/pages/Groups.tsx, src/pages/ModelTestPanel.tsx, src/locales/*.json（仅当新增 i18n key）
- 禁改文件: **/dist/**, .trellis/**, src-tauri/**（属 S1）, src/services/api.ts（只读消费）

## 输出格式
- 类型: diff + yarn build / yarn check:i18n 输出尾部
- 行数上限: 验证输出截断

## 验收标准
yarn build + yarn check:i18n 全绿（见 subtask 验证节）；手验三项（见上）。

## 失败处理
- 工具瞬时错误 → 重试 1 次
- 业务阻塞 → 输出 `需要: <问题>` 并停
- 资源不可用 → 报 Blocked

## Sub-agent 自防护
你已是 trellis-implement，直接做，禁再 spawn。
```

## 回滚

- 触发条件: yarn build / check:i18n 红 / 手验徽章不刷新
- 步骤: `git checkout -- <改动文件>` 或 worktree 整体丢弃

## 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 事件名拼写两端不一致致刷新失效 | 徽章不更新 | 抽常量统一引用 |
| 批量测试高频派发致 N 次后端查询 | 短时 N 次 lastTestResult 调用 | 可接受（本地查询，量小）；必要时 debounce |
| 徽章与 health 点视觉重叠 | UI 错乱 | 徽章独立位置，手验调整 |
| 新 i18n key 漏某语言 | check:i18n 红 | 跑 yarn check:i18n 把关 |

## 历史
- 2026-06-19: created
