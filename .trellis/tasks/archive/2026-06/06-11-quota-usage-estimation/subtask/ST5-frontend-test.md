# ST5: 前端展示预估值 + 测试

- **目标**: 前端展示预估 quota + 标识 + 接刷新校准
- **产出**:
  - api.ts Platform est_* 字段（ST1 已加类型）
  - Platforms.tsx quota 区：优先展示 platform.est_balance_remaining / est_coding_plan（预估值）+ 「预估」标识（区别真查值）；冷启动/无 est 显真查值
  - 刷新图标（已有 refreshQuota）触发后端真查校准（覆盖 est + 重置），刷新后展示真值
  - 整体 cargo test + tsc 回归
- **验证**: tsc 0 / yarn build；展示预估值 + 标识；刷新校准
- **资源**: research/frontend-integration.md、design.md、现有 quota 展示(Platforms.tsx:1645 区)
- **依赖**: ST4
- **失败处理**: 禁 any；与别窗口 Platforms.tsx 冲突仅改 quota 区
