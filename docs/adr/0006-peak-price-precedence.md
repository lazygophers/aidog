# ADR 0006: 分时计费优先级——模型级绝对价 > 平台 peak_hours 倍率 > 默认价

日期: 2026-08-26
状态: accepted

## 背景

分时计费现状是 platform-presets 的平台级 `peak_hours` 倍率（×multiplier，落 `proxy_log.est_cost`）。
新 registry 引入 per-model 分时**绝对价**（如 GLM 高峰独立 input/output 价），两机制并存。

## 决策

1. 计费解析顺序：**模型级 peak 绝对价（命中窗口）→ 平台 peak_hours 倍率 → 默认价**。
2. **时间窗判定一份真值**：模型只带价格不带窗口，窗口复用 preset 的 `peak_hours` 判定
   （Rust `peak_hours.rs` 与前端 `utils/peakHours.ts` 已对称实现）。无 preset 窗口的平台，
   模型 peak 价不生效。
3. 渐进迁移：现有 glm ×3.0 倍率数据继续生效，各模型逐步补绝对价后自然接管。

## 后果

- 同一模型同一时刻只有一个生效价格来源，est_cost 计算无歧义。
- 模型 peak 价依赖平台配置了 peak_hours 窗口，文档需说明该前提。

## 备选方案

- 绝对价完全取代倍率：现有倍率数据需全部换算，一次迁移成本高，弃。
- 绝对价仅展示不计费：用户明确要计费生效，弃。
- 模型自带时间窗：与 preset 窗口冲突、维护成本高，弃。
