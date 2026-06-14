# 修复 model_test 对 Mock 平台返回 502 builder error

## 根因

`lib.rs::model_test` 对所有协议统一走真实 reqwest HTTP（`req_builder.send()`），唯独漏了 Mock 分支。proxy.rs L881 对 Mock 走 `handle_mock` 本地生成响应，model_test 没复用此逻辑（[[model-test-proxy-parity]] 警告的路径不对齐）。

Mock 平台 base_url 通常为空/占位 → `format!("{}{}", base_url, api_path)` 产出非法 URL → `client.post(&url).send()` 失败 → reqwest "builder error" → 502。

日志佐证（88f1e3a4）：
- upstream URL 仅 `/chat/completions`（无 base_url 前缀）
- `model: ""`（mock 平台无默认模型）
- "upstream error: builder error"

## 修复

`model_test` 在构造 req_builder 之前加 Mock 分支：复用 `gateway::adapter::mock`（`resolve_mock_config` + `build_response` / `build_error_body`）本地生成，与 `handle_mock` 行为对齐（delay / error_mode: http_error/rate_limit_429/timeout / 正常响应）。

- 成功：提取 `response_text` + input/output token → `ModelTestResult { success: true, ... }`
- error_mode：按 status 返回失败结果，error 字段含 mock 标识
- 写 proxy log（source=test, target=mock）

## 验证

- `cargo test`（含新增 mock 分支测试）
- `cargo clippy` clean
- Mock 平台 model_test 返回 200 + 假响应（不再 502）

## 范围

仅 `src-tauri/src/lib.rs::model_test`。不改 proxy.rs / mock adapter。
