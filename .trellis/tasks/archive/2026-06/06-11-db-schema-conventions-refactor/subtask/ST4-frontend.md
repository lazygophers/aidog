# ST4: 前端类型 + 页面

- **目标**: 前端类型对齐新契约
- **产出**:
  - services/api.ts: `id: string`→`number`（proxy_log 留 string）；`Platform.protocol`→`platformType`；`created_at/updated_at` string→number + `deleted_at`；删 `mappingApi`；ModelMapping 去 id/group_id
  - pages: Platforms.tsx（platformType）；Groups.tsx（mapping 改操作 group.model_mappings 数组，随 group save）；Logs/Stats（created_at number、时间显示 ms→Date）
- **验证**: `tsc --noEmit` 退出码 0 / `yarn build`
- **资源**: design.md、api.ts、pages/*.tsx
- **依赖**: ST3
- **失败处理**: 类型错误逐修
