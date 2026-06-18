# PRD: 分组展开显示完整可展开平台卡片

## 现状（问题）

上任务 `merge-groups-platforms` 把分组内嵌进 Platforms 页，但分组卡片展开区里关联平台仍只显示** badge tag 列**（Groups.tsx 原始行为）。用户要求：

1. 分组展开里的平台要显示成**完整 PlatformCard**（同 Platforms 主列表那样：logo/状态/余额/quota/模型/操作按钮），不是 tag。
2. **点平台卡片就地展开详情**（PlatformCard 原生 expand 行为），不跳转、不进编辑。
3. 分组卡片头点击 = **展开/收起平台列表**，不是直接进分组编辑（编辑走已有的显式 edit 按钮）。

## 目标

### 抽共享层（ Platforms.tsx → 可复用 ）
- 把 `PlatformCard`（presentational，~L1305-2213）+ 其 per-platform state 与 handler 抽成可复用单元：
  - 方案：抽 `usePlatformCards({ platforms, onNavigate, onEdit? })` hook，封装 `quotaMap/usageMap/expandedIds/testingId/testResults/faviconFailed/quotaRealIds/quotaRefreshing` 8 个 state + `refreshQuota/toggleExpanded/handleQuickTest/handleToggle/handleViewLogs/handleDelete/handleEdit/onFaviconFailed` handler，返回 `{ states, actions, cardPropsFor(p, index) }`。
  - `PlatformCard` 组件 + `PlatformCardProps`/`PlatformCardActions` 类型 + 所需模块级 helper（`computeQuotaDisplay`/`healthStatus`/`allModelValues`/`getPlatformLogo` 等，已模块级）从 Platforms.tsx export（或抽到新文件 `src/components/platforms/PlatformCard.tsx` + `usePlatformCards.ts`）。
- Platforms 主列表改用抽出的 hook/组件，**行为零回归**（拖拽 reorder 保留在列表 wrapper 层，传 `isDragging/dragActive` 给 card）。

### GroupsEmbedded 展开区改造（ Groups.tsx ）
- 分组卡片展开内容：删 badge tag 列（L893-901），改为渲染 `<PlatformCardList platforms={group平台detail列表} onNavigate={...} />`（或循环 PlatformCard + hook）。
- 平台数据：GroupsEmbedded 已 `platformApi.list()` 拿全量 platforms（state `platforms`），按 group 的 `gp.platform.id` 映射出该组 platform 完整对象传入。
- 点平台卡片 → PlatformCard 原生 `onToggleExpanded` 就地展开（hook 提供），**不跳转**。
- 分组卡片头 `onClick`（L818）从 `openEdit` 改为 toggle 展开（CompactCard 已有 toggle；让 header 点击也触发 toggle，或直接靠 CompactCard toggle）。编辑分组走 quick action 区已有的 edit 按钮（L844）。
- onEdit（平台编辑）：hook 的 `handleEdit` 在 GroupsEmbedded 上下文 → `onNavigate('platforms', { platformId, platformName })` 跳 Platforms 编辑（保留可编辑路径）。

### 模型映射 / 中间件规则
- 分组展开区原有的模型映射 quick-add、中间件规则面板**保留**（它们是 group 级功能，非 platform 级），位置移到平台卡片列表下方。

## 不做（范围边界）

- ❌ 不改 PlatformCard 内部渲染逻辑（仅抽位置 + 接线）。
- ❌ 不改后端 / db / quota / usage API。
- ❌ 不改 Platforms 编辑表单。
- ❌ 不改分组编辑表单内部字段。
- ❌ 不引入平台拖拽到分组展开区内（拖拽仅 Platforms 主列表保留）。

## 改动范围

| 文件 | 改动 |
| --- | --- |
| `src/components/platforms/PlatformCard.tsx`（新）或 Platforms.tsx export | 抽 PlatformCard + 类型 + helper |
| `src/components/platforms/usePlatformCards.ts`（新）或 Platforms.tsx export | 抽 per-platform state hook |
| `src/pages/Platforms.tsx` | 主列表改用抽出层；零行为回归 |
| `src/pages/Groups.tsx` | GroupsEmbedded 展开区：badge→PlatformCardList；header 点击改 toggle；保留映射/中间件 |

## 验收

1. `yarn tauri dev`：Platforms 主列表视觉与交互**零回归**（拖拽/quota 刷新/展开/测试/编辑/删除/启停/查日志全正常）。
2. 分组卡片展开：见完整 PlatformCard（logo/状态/余额/quota/模型/操作），非 tag。
3. 点分组内平台卡片 → 卡片就地展开详情（endpoints/模型/quota tiers），不跳转。
4. 点分组头 → 展开/收起平台列表，不进分组编辑；编辑分组走 edit 按钮仍可进编辑表单。
5. 分组内平台编辑（卡片 edit 按钮）→ 跳 Platforms 编辑页定位该平台。
6. 分组增删平台后，展开区卡片同步。
7. 模型映射 quick-add + 中间件规则面板仍在分组展开区可用。
8. `npx tsc --noEmit` 0 error；`node scripts/check-i18n.mjs` 0 缺键。
9. 7 语言 + 亮/暗主题无违和。

## 风险

| 风险 | 缓解 |
| --- | --- |
| 抽 PlatformCard 破坏 Platforms 主列表（3000 行核心页） | 抽完先 tsc + dev 全量回归 Platforms 主列表再动 Groups；行为 diff 对照 |
| hook 抽取遗漏 state/handler | 逐项核对 8 state + 10 handler 全迁移；actionsRef 模式保留或重构 |
| Groups 展开区 per-platform quota/usage 额外请求量 | hook 复用同一 quota/usage 获取逻辑；platforms 跨分组共享时考虑缓存去重（同一 platform 多组展示） |
| 分组头点击改 toggle 后编辑入口丢失 | 保留 quick action edit 按钮（L844 已有）显式入口 |

## subtask（单一交付，inline 不拆 child）

S1 抽 PlatformCard + usePlatformCards hook → S2 Platforms 主列表接线回归 → S3 GroupsEmbedded 展开区用 PlatformCard + header toggle。串行（共享抽取层）。
