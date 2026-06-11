# ST5: 测试 + spec 沉淀

- **目标**: 测试覆盖 + mock 平台类型规范固化
- **产出**:
  - Rust 单测：5 协议非流式 builder shape / SSE 序列 / 三层覆盖优先级（body>role>extra，每字段独立回退）/ error_mode 各分支（none/http_error/429/timeout）/ 假 token 填充 / stream_override
  - spec：`.trellis/spec/backend/` 加 mock 平台类型约定（何时用、配置 schema、三层覆盖、协议范围），或追加到 db-conventions/新建 mock-platform.md
- **验证**: cargo test 退出码 0；spec 命令式 + 死链 0
- **资源**: design.md、prd 需求、现有 spec 范式
- **依赖**: ST3, ST4
- **失败处理**: 测试失败修代码非改测试
