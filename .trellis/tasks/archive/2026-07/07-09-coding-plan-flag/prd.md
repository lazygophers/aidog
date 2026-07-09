# 协议层 is_coding_plan 字段 + 跨层消费

## Goal

platform-presets.json 协议层加显式 `is_coding_plan: bool` 字段标记 coding plan 套餐协议（glm_coding / bailian_coding / compshare_coding），消除靠键名 `_coding` 后缀启发式识别的脆弱。PlatformCard 据此数据驱动显示「Coding Plan」徽标（现 constants.ts PROTOCOLS 硬编码 `codingPlan: true` 仅 glm_coding 一条，非数据真值）。

## What I already know

- 3 协议确认为真 coding plan 套餐（base_url 各走 coding 子域）：
  - `glm_coding`：智谱编码套餐 `https://open.bigmodel.cn/api/coding/paas/v4`
  - `bailian_coding`：阿里云百炼编程 `https://coding.dashscope.aliyuncs.com/apps/anthropic`
  - `compshare_coding`：Compshare 编程套餐 `https://cp.compshare.cn`
- 现有 endpoint 级 `coding_plan: bool` flag（glm.default=False / glm_coding.default=True / minimax 显式 false）= 端点路由级标记，语义不同于协议层（标识整个协议是 cp 套餐 vs 标识某端点走 cp 路径）。两者并存。
- 前端硬编码：`src/domains/platforms/constants.ts:14` PROTOCOLS `codingPlan: true` 仅 glm_coding；bailian_coding / compshare_coding 缺标记。
- Rust PlatformPreset struct 在 `src-tauri/src/gateway/models/` 下（需定位）

## Decisions

| # | 决策 |
|---|---|
| D1 | 字段名 `is_coding_plan`（bool），与 endpoint 级 `coding_plan` flag 区分（避免重名混淆） |
| D2 | 标记范围：3 协议 `is_coding_plan: true`（glm_coding / bailian_coding / compshare_coding）；其他协议 absent（serde default = false，向后兼容） |
| D3 | 跨层对称：JSON + Rust struct (`#[serde(default)]`) + TS 类型 + PlatformCard 数据驱动徽标 |
| D4 | UI 消费：PlatformCard 徽标读 `preset.is_coding_plan`（数据真值），constants.ts PROTOCOLS `codingPlan` 保留前端下拉用但注释指向 preset 为真值源 |

## Requirements

- R1: platform-presets.json 3 协议（glm_coding / bailian_coding / compshare_coding）加 `is_coding_plan: true`
- R2: Rust PlatformPreset struct 加 `#[serde(default)] pub is_coding_plan: Option<bool>`（或 bool default false）
- R3: TS 类型（defaults.ts 或 api types）加 `is_coding_plan?: boolean`
- R4: PlatformCard 据 `preset.is_coding_plan` 显「Coding Plan」徽标（数据驱动，非硬编码协议键名）
- R5: last_updated 更新；3 协议标记后前端 getDefaultPreset 读到 is_coding_plan=true

## Acceptance

- [ ] JSON 3 协议含 `is_coding_plan: true`（python grep 验证）
- [ ] Rust struct 字段 + serde default（cargo build OK）
- [ ] TS 类型字段（tsc 0 错）
- [ ] PlatformCard 徽标数据驱动（grep 无硬编码协议键名判断）
- [ ] cargo test 0 fail / clippy 0 新增 / yarn build 0 错
- [ ] 跨层对称（is_coding_plan 字段名三处一致）

## Out of Scope

- endpoint 级 coding_plan flag 机制（保留，不动）
- 协议键名重命名（glm_coding 键名保留，仅加字段）
- 新增 coding plan 协议（仅标记现有 3 个）
- 下拉列表重构（PROTOCOLS 保留前端枚举）

## Technical Notes

- 字段命名禁与 endpoint 级 `coding_plan` 混 → 用 `is_coding_plan`
- 跨层 guide：`.trellis/spec/guides/cross-layer-rules.md`
- Rust struct 定位：先 grep `struct PlatformPreset` 或 defaults 解析处
- PlatformPreset struct 改需 cargo build（非 command 改，无需重启 dev，但 check 时 cargo test 验）
