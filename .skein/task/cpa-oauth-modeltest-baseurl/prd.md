# CPA OAuth platform base_url 回填 — PRD

## 目标
- CPA OAuth provider 导入时按 oauth_type 回填静态上游 base_url，禁透传空值到 DB
- model-test + 代理 forward 加 base_url.is_empty() guard，空时返友好错误（非 reqwest builder error）
- 解 request f77e2282（cpa-grok platform 303 base_url 空 → model-test 502）

## 边界
**范围内**:
- `parser.rs::parse_oauth_json` / `mapper.rs::map_provider` OAuth 分支按 oauth_type 回填静态 base_url
- CpaOAuthType 加 `default_base_url()` 方法
- `model_test.rs::prepare_http_request` + `forward.rs`（两处对称）加 base_url 空 guard
- 回填范围：Xai(`https://api.x.ai`)/Aistudio(`https://generativelanguage.googleapis.com/v1beta`)/Antigravity(`https://cloudcode-pa.googleapis.com`)；Vertex 不回填（用户导入后表单 endpoints 区填 region-specific base_url）；Codex/Claude/Kimi research 未覆盖上游，不回填（guard 兜底）

**范围外**:
- 不改前端（Vertex 走现有 platform 编辑表单 endpoints 区填 base_url）
- 不改 cpa-import-multi-edit（多 provider 编辑模式，另 task）
- 不改已入库数据（输出修复 SQL 给用户自跑）
- 不加 Vertex 专门 region 输入 modal

**关键约束**:
- base_url 不含 path 段（path 在 converter：CpaGrok `/v1/responses`，base_url 只 `https://api.x.ai`，禁 `/v1/v1/responses` 重复）
- base_url 格式须对齐各 adapter 期望（openai_responses / gemini）—— s1 实现时读 converter/request.rs + gemini adapter 确认
- guard 两处对称（model_test + forward 同构）

## 验收标准
- [ ] Xai OAuth provider 导入 → platform.base_url = `https://api.x.ai`（非空）
- [ ] Aistudio/Antigravity 同理回填正确静态 base_url
- [ ] Vertex/Codex/Claude/Kimi OAuth 导入 base_url 仍空（不回填），但 model-test 时空 base_url 返友好错误（"base_url 缺失，请填 endpoints"非 builder error）
- [ ] cpa-grok model-test 能正常发上游（platform 303 改 base_url 后或新导入的）
- [ ] cargo clippy + cargo test（parser/mapper/model_test/forward 相关）过
- [ ] 已入库 platform 303 修复 SQL 输出（UPDATE base_url）

## 索引
- 设计: [design.md](design.md)
- 根因调研: [research/root-cause.md](research/root-cause.md)
- task.json: `skein.py subtask list cpa-oauth-modeltest-baseurl`
