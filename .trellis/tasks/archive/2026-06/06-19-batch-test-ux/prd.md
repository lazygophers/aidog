# 批量测试 UX: 弹窗可中途关闭 + 并行数 3

## 目标
改进分组「一键测试本组全部平台」体验：① 测试进行中允许关闭结果弹窗；② 测试从串行改为并发上限 3。

## 现状（已调研）
- `src/pages/Groups.tsx:487 handleTestGroup`：**串行** for-loop，逐个 `await modelTestApi.test`。无并发。
- 结果面板由 `groupTest` state 驱动（480-483）；面板组件 ~111-160（TestPanel）。关闭按钮在测试 `running` 时被禁用/拦截 → 用户无法中途关。

## 需求
1. **弹窗可中途关闭**：测试 `running===true` 时，结果面板关闭按钮（及遮罩点击/Esc 若有）可用。关闭即 `setGroupTest(null)`。
   - 关闭后在途测试结果丢弃：`patchRow` 已是 `prev ? ...map : prev`，groupTest=null 后自动 no-op，无需额外 abort（在途 promise resolve 时 setGroupTest 仍 null → 安全）。确认无 stale 写回 / 无报错。
2. **并发上限 3**：`handleTestGroup` 串行 for-loop 改为**有界并发**（同时最多 3 个 `modelTestApi.test` 在跑），跑完一个补下一个，直到所有行完成。
   - 常量 `BATCH_TEST_CONCURRENCY = 3`（具名常量，便于调整）。
   - 每行状态仍实时 patchRow（testing→ok/fail），面板逐行刷新不变。
   - 全部完成后 `running=false`。
   - 若中途关闭（groupTest=null），在途任务完成时 patchRow no-op，不报错、不复活面板。

## 影响面
- 仅 `src/pages/Groups.tsx`（handleTestGroup 并发改造 + TestPanel 关闭按钮 ungate）。
- 可能 i18n：若关闭按钮文案/aria 新增 key 走 7 语言（一般复用现有关闭 key，无新增）。

## 验收标准
- 批量测试同时最多 3 个平台在测（并发），逐行实时刷新，结果与串行一致。
- 测试进行中可关闭结果弹窗；关闭后无报错、无 stale 写回、在途结果不复活面板。
- 关闭按钮在 idle 与 running 两态都可用。
- 成功/失败行展示（含失败原因，commit 8ded263）不回归。
- 门禁：`yarn build`（tsc && vite）绿无新 warning；`node scripts/check-i18n.mjs` 零缺失（若动文案）。

## 失败处理
- 并发实现用简单 worker-pool（index 游标 + N 个 worker await 循环），不引第三方库。
- 关闭后在途 setGroupTest 写回若有 race，用关闭时的 guard（如比对 groupId）确保不复活。

## 执行载体
单一 trellis-implement subagent（轻量，单文件），worktree 隔离 → check → finish。
