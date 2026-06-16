# 使用统计 — 模型筛选仅含有记录的模型

## 背景
Stats 页模型筛选下拉 `allModels` 当前来自配置：`groups.model_mappings.target_model` ∪ `platforms.available_models`。导致下拉列出大量「配置过但从未实际请求」的模型，用户筛选拿到空结果。

## 目标
模型筛选下拉只展示**当前筛选范围内（日期 + 分组 + 平台）实际有 proxy_log 记录**的模型名，选即有数据。

## 方案
- 后端 `StatsResult` 新增字段 `available_models: Vec<String>`。
- `query_stats_inner` 增一次 `SELECT DISTINCT` 查询：列表达式 `CASE WHEN actual_model != '' THEN actual_model ELSE model END`，WHERE 复用 date + filter_group + filter_platform（**不含 filter_model**，否则选中后下拉自缩）。
- 前端 `Stats.tsx`：`allModels` 改为从 `data.available_models` 取（保留兜底空数组），删除原 groups/platforms 派生。
- 平台筛选下拉不动（platforms 配置项有限且语义正确）。

## 验证
- 选某模型 → overview/dimension/buckets 仅该模型记录。
- 清空模型筛选 → 下拉仍列全部有记录的模型（不受当前选中影响）。
- 切日期/分组/平台 → 下拉模型列表随之收窄。
- cargo test：新增 `stats_available_models_excludes_unrecorded` 断言 available_models 只含插入过的模型。
