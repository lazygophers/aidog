# ST2: 验证

- **目标**: 流式 token 修正 + 无回归
- **产出**:
  - cargo build + cargo test 全绿
  - 验证流式请求 proxy_log token 非 0（手测或单测构造 SSE [DONE] 流断言 token 写入）
  - 非流式分支不受影响；[DONE] upsert 仅一次（est_fired 守卫）
- **验证**: cargo test 0；流式 token 非 0
- **资源**: design.md
- **依赖**: ST1
- **失败处理**: 测试失败修代码
