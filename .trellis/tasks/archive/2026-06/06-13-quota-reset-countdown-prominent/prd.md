# PRD: 下拉额度详情突出展示重置倒计时

## 背景

平台列表(Platforms 页)点击下拉展开的"额度"区, 各 coding plan tier 用 `StatChip` 展示。当前重置倒计时 (`formatResetCountdown`, 如 "2d 3h") 被拼在 `StatChip` 的 `label` 后面 (`tierLabel + " · " + countdown`), 视觉隐蔽, 用户不易察觉。

数据链路已完整:
- 后端 `QuotaTier.resets_at: Option<String>` (quota.rs:61) — 仅 Kimi/GLM 上游 API 返回, 已解析
- 前端 `QuotaTier.resets_at` → `computeQuotaDisplay` 映射为 `resetsAt`
- `formatResetCountdown(resetsAt)` 已实现相对倒计时格式化 (Platforms.tsx:987)

## 目标

下拉额度详情里, 把重置倒计时从隐蔽的标签拼接改为**醒目展示**(独立成行 / 加重置图标), 让用户一眼看到"还有多久重置"。

## 范围

- 仅改 `src/pages/Platforms.tsx` 下拉额度区 (约 1344-1360 行) 的 tier 渲染。
- 保留现有相对倒计时格式 ("2d 3h" / "3h 20m" / "45m"), 不改 `formatResetCountdown` 逻辑。
- 只有 tier 有 `resetsAt` (Kimi/GLM) 时才显示倒计时行; 无数据 tier 不显示, 不留空行。
- 余额类平台(DeepSeek 等)无 coding plan tier, 不受影响。

## 非目标

- 不改后端 / TS 类型 / 数据查询。
- 不新增绝对时刻点展示(用户已选相对倒计时)。
- 不为无 resets_at 的平台补充 reset 数据来源。

## 验收标准

- Kimi/GLM 平台下拉展开, 各 tier 的重置倒计时独立、醒目可见(非拼在标签尾)。
- 无 resets_at 的 tier 不显示倒计时, 不破坏布局。
- i18n: 新增文案(如"重置")走 7 语言 key, 不硬编码。
- `yarn tsc` / lint 无新增 warning。

## UI 决策

- 形式: 相对倒计时(更醒目)。
- 具体呈现: tier chip 下方或内部加一行小字 + 重置图标(如 `IconRefresh`/沙漏), 文案 `{countdown}` 或 `重置 {countdown}`。最终样式实施时定, 遵循 Liquid Glass + 现有 `text-tertiary` 小字风格。
