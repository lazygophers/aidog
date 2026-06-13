# Model-test 补齐 coding_plan 处理 + endpoint 选择逻辑

## 背景

Model-test（平台模型测试功能）与 proxy 的请求构建路径存在差异，导致 coding plan 端点（Kimi、GLM、百炼等）测试时缺少必要字段，可能测试失败。

## 问题清单

1. **endpoint 选择盲取 `[0]`** — `lib.rs:793` 固定取第一个 endpoint，不按协议匹配。若平台有多个 endpoint（如 normal + coding），可能取错。
2. **忽略 `coding_plan` flag** — 不读取 endpoint 的 `coding_plan`，不调用 `inject_coding_plan_fields`。Kimi coding plan 需要 `prompt_cache_key`，缺失可能影响行为。
3. **忽略 `override_coding_plan_path`** — 当前为空函数（预留），但应与 proxy 保持一致调用。

## 修复方案

对齐 `lib.rs:model_test()` 与 `proxy.rs` 的请求构建逻辑：

1. 读取 endpoint 的 `coding_plan` 字段
2. 若 `coding_plan == true`，调用 `inject_coding_plan_fields` 注入平台特有字段
3. 若 `coding_plan == true`，调用 `override_coding_plan_path`（当前 no-op，保持对齐）
4. endpoint 选择优先取含 `coding_plan: true` 的 endpoint（model-test 场景下，测试 coding plan 端点更有意义），回退到 `endpoints[0]`，再回退到平台主配置

## 验收标准

- [ ] model-test 读取 `coding_plan` 并在 `true` 时调用 `inject_coding_plan_fields`
- [ ] endpoint 选择逻辑优先 coding_plan endpoint
- [ ] cargo check 通过
- [ ] 不影响非 coding plan 平台的 model-test 行为
