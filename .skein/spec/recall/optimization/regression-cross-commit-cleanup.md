---
title: regression-cross-commit-cleanup
layer: recall
category: optimization
keywords: [regression,cross-commit,intermediate-features,scope-cleanup,baseline]
status: active
inclusion: auto
created: 1722524400
---

## 跨长 commit 区间回归对比前必先排查独立无关功能

## 触发场景

在做「改前/改后」回归对比时，baseline 到 HEAD 跨越多个 commit（>50），其中包含了与本轮优化完全无关的功能变更（如新增定价模型 / 权限系统 / 序列化格式升级）。若不先排查出无关功能，后续观察到的数据差异会被误会为性能回归。

## 陷阱

❌ **直接对比 baseline 与 HEAD 的数据**

```bash
# baseline（commit ba6b7b22）
TOTAL 185MB, est_cost 1000

# HEAD（commit 294c7b8d，跨 149 commits）
TOTAL 184.5MB, est_cost 950

# 表面现象：TOTAL 降了，但 est_cost 也降了
# 误判：性能优化成功 OR 数据漂移 OR 定价改了
# 实际根因：中间混入了 commit 8ccccb41 新增 time_tiers 定价分级，与性能无关
```

## 正解

在跨越多 commit 做回归对比前，先用 `git log --oneline` 扫一遍这个区间，**按关键词分类找出中间落入的独立功能**：

```bash
# 1. 查看 baseline..HEAD 的全部 commit（按领域过滤）
git log --oneline ba6b7b22..294c7b8d | grep -E 'feat|fix'

# 2. 按问题域关键词找出无关功能落入
#    性能优化范围：memory / cpu / perf / bundle / render / footprint
#    无关功能例：pricing / auth / schema / feature-flag / ui-refactor
git log --oneline ba6b7b22..294c7b8d -- '*price*' '*billing*' '*estimate*'

# 3. 对于要对比的指标（如 est_cost），查关键变更点
git log -S 'est_cost' --oneline ba6b7b22..294c7b8d -- src-tauri/src/gateway/

# 4. 若无关功能存在，用该功能的引入 commit 为分割点，分别对比：
#    - baseline..功能引入前的 commit：衡量「性能优化」
#    - 功能引入..HEAD：衡量「功能本身的影响」
```

## 案例（perf-final-verification s4）

baseline `ba6b7b22` 到 HEAD `294c7b8d`，观察到 est_cost 4/6 条对不上（预期全对）。

未排查时假设：性能优化不稳定，某些逻辑漂移了。

排查后发现：commit `8ccccb41` 引入了独立功能 `model-price-time-tiers`（时间分段定价），仅改 `gateway/db/model_price.rs` / `price_sync.rs` / `billing.rs` 等定价路径，与本轮 8 个性能优化 task（全部在 `proxy.rs` / `router.rs` / 内存优化 / CPU profiling 等）**零重合**。改动点互斥 → est_cost 差异非性能回归，是独立定价功能导致。

**判决修正**：红线 2 从「逐条一致性 FAIL」改为「无性能回归 PASS」，两种口径明确区分。

## 验证清单

- [ ] 查询 `git log --oneline <baseline>..<HEAD> | wc -l` 确认跨度
- [ ] 按无关功能关键词过滤：`git log --oneline <baseline>..<HEAD> -- '<领域>*'`
- [ ] 逐条确认变更的源码文件范围（`git show --stat`）与本轮优化范围是否交集
- [ ] 若有无关功能，用分割点重新计算 baseline

## 适用

- 性能优化 task 的回归验证（跨越多个中间件 commit 时）
- 功能对比时跨越了版本功能迭代（新增模块/定价/权限等）
- 参数调整的前后对比（需排除同期其他改动影响）
