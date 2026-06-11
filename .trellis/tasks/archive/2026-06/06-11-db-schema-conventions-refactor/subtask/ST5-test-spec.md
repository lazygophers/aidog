# ST5: 测试 + spec 沉淀

- **目标**: 测试覆盖 + 规范固化（R6 后续遵守）
- **产出**:
  - Rust 单测: schema 约束（无 NULL 列）/ 时间戳 ms 读写 / 软删除过滤（deleted_at>0 不返回）/ 主键自增 / 默认值非 null / group.model_mappings JSON 往返
  - spec: 新建 `.trellis/spec/backend/index.md` + `conventions.md`（10 条 DB 规范命令式条款，经 trellisx-spec 风格）
- **验证**: `cargo test` 退出码 0；spec 命令式 + 死链 0
- **资源**: prd 规范清单、design.md、现有 .trellis/spec/frontend 范式
- **依赖**: ST2, ST4
- **失败处理**: 测试失败修代码非改测试
