# ST1: Protocol::Mock + MockConfig + 三层配置解析

- **目标**: 加 mock 平台类型枚举 + 配置结构 + 三层覆盖解析（契约核心）
- **产出**:
  - models.rs Protocol enum 加 `#[serde(rename="mock")] Mock`（平台类型区）
  - adapter/mod.rs 注册 `pub mod mock;`
  - adapter/mock.rs 新建：`MockConfig` struct（`#[serde(default)]` 全字段：status_code/delay_ms/stream_override/response_text/finish_reason/input_tokens/output_tokens/cache_tokens/error_mode/chunk_count），从 platform.extra 的 `.mock` 反序列化（空 extra→全默认）
  - 三层覆盖解析 `fn resolve_mock_config(extra: &str, chat_req: &ChatRequest, body_json: &Value) -> MockConfig`：extra 默认 → message role 映射覆盖（role∈字段名时 content 为值）→ body.mock 覆盖（每字段独立回退）
- **验证**: cargo build 0
- **资源**: design.md（三层覆盖 + schema）、models.rs Protocol、research 文档
- **依赖**: 无
- **失败处理**: 编译错逐修
