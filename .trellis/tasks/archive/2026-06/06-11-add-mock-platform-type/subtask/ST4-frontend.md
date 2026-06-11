# ST4: 前端 Protocol + mock 配置 UI

- **目标**: 前端支持选 mock 平台类型 + 编辑 mock 配置（写 platform.extra）
- **产出**:
  - api.ts Protocol union 加 `| "mock"`
  - Platforms.tsx PROTOCOLS 加 `{value:"mock",label:"Mock（本地模拟）",keywords:["mock","测试","调试","假数据"]}`；ENDPOINT_PROTOCOLS 不加；getDefaultEndpoints mock 返空
  - `platform_type==="mock"` 时：base_url/api_key 去必填校验、隐藏 endpoints 编辑、显示 mock 配置编辑器（表单字段：status_code/delay_ms/stream_override/response_text/finish_reason/input_tokens/output_tokens/cache_tokens/error_mode/chunk_count），读写 platform.extra 的 mock 子对象（JSON round-trip）
- **验证**: tsc --noEmit 0 / yarn build
- **资源**: design.md（前端节 + extra schema）、Platforms.tsx PROTOCOLS/表单、api.ts
- **依赖**: ST1（Protocol 契约）
- **失败处理**: 类型错逐修，禁 any
