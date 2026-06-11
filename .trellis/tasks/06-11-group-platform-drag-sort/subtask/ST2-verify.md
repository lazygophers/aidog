# ST2: 验证

- **目标**: 拖拽排序持久化正确
- **产出**:
  - tsc 0 / yarn build
  - 验证：拖拽重排 → saveEdit → set_group_platforms priority(按新序) → 重开 group 顺序正确（get_group_platforms ORDER BY priority）
- **验证**: tsc 0；端到端顺序持久化
- **资源**: design.md
- **依赖**: ST1
- **失败处理**: 顺序不持久 → 查 saveEdit priority 映射
