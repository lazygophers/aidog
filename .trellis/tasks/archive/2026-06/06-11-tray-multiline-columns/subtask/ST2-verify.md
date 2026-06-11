# ST2: 验证

- **目标**: 渲染正确 + 无回归
- **产出**:
  - cargo test + tsc 0
  - 列对齐逻辑测试（列宽估算/两行组装/range 着色 纯函数可测部分）
  - 单行模式（无 two_line item）无回归
- **验证**: cargo test 0；GUI 列对齐+垂直 用户 macOS 验
- **资源**: design.md
- **依赖**: ST1
- **失败处理**: GUI 不可机测 → 逻辑测 + 标注用户验
