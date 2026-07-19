# PRD — peak models 分支 review 修复

## 背景

code-review `ce041727` (peak models 分支路由层切换) 发现 3 项. 经用户裁定:

- **发现 1 (peak 覆盖用户定制)**: 行为**保持现状** (peak 总覆盖 `platform.models`). 但当前仅 `resolve_effective_models` 注释隐含表达, 未来读者易误判为 bug → 需**显式文档化设计意图** (代码注释 + CLAUDE.md).
- **发现 2 (model_list 缺 peak 分支)**: 修. `models` 已有 `{default, peak}` 双分支, `model_list` 仍只有 `{default}` → 高峰期模型下拉冷启动缺 peak 模型.
- **发现 3 (CLAUDE.md 未更新)**: 修. `peak_hours` 小节只记 cost multiplier, 漏 `models.peak` 模型切换机制; "前端 4 函数" 列表过期.

## 目标

1. `resolve_effective_models` 加显式注释: peak 分支**设计上覆盖用户定制** (preset 级硬约束, 同 coding_plan 端点维度优先级). 非行为变更.
2. `platform-presets.json` glm_coding 加 `model_list.peak`; 前端 `getDefaultModelList` 支持 `isPeak` 第 3 参, 走 `pickModelsBranch`; 所有 caller 审 (PlatformCard / formSections).
3. CLAUDE.md: `peak_hours` 小节补 `models.peak` 机制段; "前端 4 函数" → 5 函数 (加 `getDefaultPeakHours`) + 标注 `getDefaultModels`/`getDefaultModelList` 的 `isPeak` 第 3 参.

## 非目标

- 不改 peak 覆盖行为本身 (用户裁定保持现状).
- 不重构 serde-name 提取 (REFUTED, 既有 idiom).
- 不缓存 `default_peak_models` 反序列化 (REFUTED, 与 `default_peak_hours` 同模式).

## 验收

- `cargo test -p aidog_core` 绿 (含新 model_list.peak 用例若加).
- `yarn build` 绿 (TS 编译捕获 caller 漏 await / 签名错).
- CLAUDE.md grep: `models.peak` / `getDefaultPeakHours` 命中.
- platform-presets.json parse OK, glm_coding.model_list.peak 存在.

## 资源

- 真值源: `src-tauri/defaults/platform-presets.json` (手维护, 禁机器覆盖).
- 前端 cross-layer 对称: `utils/peakHours.ts::isCurrentlyPeak` ↔ Rust `is_in_peak_window`.

## 依赖

无 (独立小修).
