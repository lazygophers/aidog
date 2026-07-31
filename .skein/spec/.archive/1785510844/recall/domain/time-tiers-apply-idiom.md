---
name: time-tiers-apply-idiom
description: 时段价表多档选策略，按 start_at 最大档命中后整体替换价表再嵌套 context 分档
type: recall
category: domain
keywords: time_tiers, 定价分档, 嵌套价表, 时间维度
---

## 触发场景

模型定价加入时间维度（同一个模型不同时段不同价格）。需要表达二维定价：时间 + 内容长度。

## 陷阱 ❌ vs 正解 ✅

**陷阱1**：time_tiers 数组用顺序首命中
- ❌ `tiers[0]` 如果 start_at 符合就用，跳过后续（其他条目无机会）
- ✅ 遍历全部条目，按 `start_at <= now_ms` 筛出全部，再选最大 start_at 的一个
  - 理由：涨价日期可能乱序、多窗口覆盖、后向兼容性

**陷阱2**：time 档命中后追加 context 分档（扁平相加）
- ❌ `base_price = pd.context_tiers[i] + time_tiers[j]`（两张表独立选档再相加）
- ✅ `time_tiers[j]` **整体替换价表**，其内嵌 `context_tiers` 完全替代顶层
  - 理由：涨价日后长文档（32k 代币档）也涨价，新价在 time 条目内部表达，顶层过时
  - 形态：time 条目 = `{base 三价, context_tiers: [...新档...]}` ← 完整独立价表
  - 顺序：`apply_tiers(time→选档→换表)` → `apply_context_tier(context 分档从新表内读)`

**陷阱3**：time_tiers 只做模型级，不支平台级
- ❌ 价表只查 `pd.time_tiers`（无法对某平台实施单独时段价）
- ✅ scope（第 2 参）先查平台节点 `pricing[platform_type]`，无则回落 `pd`
  - 理由：glm_coding 涨价只限该协议，glm 普通端点无 time_tiers

## 反例

```rust
// ❌ 顺序首命中 + 扁平相加
let tier = tiers.iter().find(|t| t["start_at"] <= now_ms)?;  // 第一个匹配就停
let price = base + tier["input_cost"] + context_tiers[i]["input_cost"];

// ✅ 最大档选策略 + 整表替换
let tier = tiers.iter()
    .filter_map(|t| (t["start_at"] <= now_ms).then_some(t))
    .max_by_key(|t| t["start_at"])?;  // 选最大的，不是首个
let price = tier["input_cost"] + tier["context_tiers"][i]["input_cost"];  // time 条目内读新档
```

## 案例

**glm-5-turbo 时段+长文档**：
- base: 32k 档 = 2e-6 $/token（普通价）
- time_tier (start_at=2026-09-30): 32k 档 = 4e-6 $/token（涨 2×）
- 选策：`start_at <= now_ms` 最大 → 命中 time_tier
- 定价：读 `tier.context_tiers[32k]` = 4e-6，而非 base 2e-6（时间优先）

## 适用

- 模型单价时间分档（glm_coding 早高峰 ×3.0 + 0-24 ×2.0）
- 平台级时段价（某云商服务某时段翻倍）
- 跨时段长文档价（涨价日前后文档价不一样）

## 关联

[[rule-66]] [[rule-67]] [[bundled-models-fallback]]
