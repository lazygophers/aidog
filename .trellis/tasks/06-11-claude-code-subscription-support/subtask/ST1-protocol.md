# ST1: Protocol::ClaudeCode 变体

- **目标**: 加 claude_code 平台类型（后端 + 前端契约）
- **产出**:
  - models.rs Protocol enum 加 `#[serde(rename="claude_code")] ClaudeCode,`（平台类型区）；`default_for_protocol`/ClientType `_` 兜底无需改；lib.rs `platform_fetch_models` 等 match Protocol 处补 ClaudeCode 分支（返空/unreachable，参照 Mock 处理）
  - 前端 api.ts Protocol union 加 `| "claude_code"`
- **验证**: cargo build 0
- **资源**: design.md、models.rs Protocol、Mock 变体先例
- **依赖**: 无
- **失败处理**: match 非穷尽编译错 → 补 ClaudeCode 分支
