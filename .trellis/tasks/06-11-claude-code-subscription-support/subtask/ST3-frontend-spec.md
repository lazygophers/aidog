# ST3: 前端配置 + spec

- **目标**: 前端支持选 claude_code 平台 + 配置 + 规范沉淀
- **产出**:
  - Platforms.tsx PROTOCOLS 加 `{value:"claude_code", label:"Claude Code 订阅（透传）", keywords:["claude code","订阅","透传","subscription"]}`；ENDPOINT_PROTOCOLS 不加
  - `platform_type==="claude_code"` 时：api_key 可空（客户端自带认证）、endpoints 隐藏、base_url 提示填 host 根（如 https://api.anthropic.com）；getDefaultEndpoints 返空，可预填 base_url 默认
  - spec：`.trellis/spec/backend/` 加 claude-code-passthrough 约定（纯透传语义/捕获点/header 剔除/不转换/记日志），或追加 mock-platform 同级新文件 + 入 index
- **验证**: tsc --noEmit 0；spec 命令式 + 死链 0
- **资源**: design.md（前端节）、Platforms.tsx PROTOCOLS/表单、现有 mock-platform.md 范式
- **依赖**: ST1
- **失败处理**: 类型错逐修，禁 any
