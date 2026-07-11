# opencode / opencode_zen preset models + endpoints 补全

## Goal

`src-tauri/defaults/platform-presets.json` 的 `opencode`（OpenCode Go）与 `opencode_zen`（OpenCode Zen 免费版）协议模型清单严重缺失或过时，与 opencode.ai 实际 API 返回不一致。按官方 `/v1/models` 实测返回补全 `model_list` + `models` 默认映射。

## What I already know

- 真值源 = `src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖；本 task 按官方 API 返回手工校对入库）
- 实测 API 返回（2026-07-11，Authorization: Bearer $opencode 匿名）：
  - **opencode Go** (`https://opencode.ai/zen/go/v1/models`)：20 个国产模型
    - `minimax-m3, minimax-m2.7, minimax-m2.5, kimi-k2.7-code, kimi-k2.6, kimi-k2.5, glm-5.2, glm-5.1, glm-5, deepseek-v4-pro, deepseek-v4-flash, qwen3.7-max, qwen3.7-plus, qwen3.6-plus, qwen3.5-plus, mimo-v2-pro, mimo-v2-omni, mimo-v2.5-pro, mimo-v2.5, hy3-preview`
  - **opencode_zen** (`https://opencode.ai/zen/v1/models`)：55 个（claude/gemini/gpt/grok + 国产 + 6 个 `-free`）
    - 国际：`claude-fable-5, claude-opus-4-{8,7,6,5,1}, claude-sonnet-{5,4-6,4-5,4}, claude-haiku-4-5, gemini-3.5-flash, gemini-3.1-pro, gemini-3-flash, gpt-5.6-{sol,terra,luna}, gpt-5.5{,-pro}, gpt-5.4{,-pro,-mini,-nano}, gpt-5.3-codex{,-spark}, gpt-5.2{,-codex}, gpt-5.1{,-codex{,-max,-mini}}, gpt-5{,-codex,-nano}, grok-build-0.1, grok-4.5`
    - 国产：`deepseek-v4-{pro,flash}, glm-5.{2,1}, minimax-m{3,2.7,2.5}, kimi-k2.{7-code,6,5}, qwen3.{6,5}-plus`
    - 免费：`big-pickle, deepseek-v4-flash-free, mimo-v2.5-free, hy3-free, nemotron-3-ultra-free, north-mini-code-free`
- preset 现状：
  - `opencode.model_list.default = []`（**零模型**）；`models.default = {}`（空映射）
  - `opencode_zen.model_list.default = ["big-pickle","glm-4.7-free"]`（`glm-4.7-free` **已不在 API 返回**，应删；big-pickle 保留）
- endpoints 现状（API 验证 base_url 正确）：
  - `opencode.endpoints.default[0]` = `{protocol: openai, base_url: https://opencode.ai/zen/go/v1, client_type: codex_tui}` ✓
  - `opencode_zen.endpoints.default[0]` = `{protocol: openai, base_url: https://opencode.ai/zen/v1, client_type: default}` ✓
- 用户提到「endpoints 也缺少内容」—— API 验证 endpoint 连接配置正确；推测用户实际指 model_list 缺失（非 endpoints 字段）。本 task 主补 model_list + models 映射；endpoints 字段如无具体缺失说明不动。

## Requirements

- `opencode.model_list.default` 填入 20 个国产模型（按 API 返回顺序）
- `opencode.models.default` 给合理默认映射（default → glm-5.2 或 minimax-m3，择一作主力；其余 slot 据模型能力映射）
- `opencode_zen.model_list.default` 替换为 API 返回的 55 个（删 glm-4.7-free，保留 big-pickle，新增其余 53 个）
- `opencode_zen.models.default` 更新：default → big-pickle（免费匿名主力）；其余 slot 映射（gpt → gpt-5.4 / sonnet → claude-sonnet-4-6 / opus → claude-opus-4-8 / haiku → claude-haiku-4-5，或保留现状 default=big-pickle 单条）
- 不动 endpoints（API 验证正确）
- 不动 Rust / 前端代码（纯 preset 数据）

## Acceptance Criteria

- [ ] `opencode.model_list.default` 含 20 个 API 返回模型，零多余零缺失
- [ ] `opencode.models.default` 至少 `default` slot 有值
- [ ] `opencode_zen.model_list.default` 含 55 个 API 返回模型，`glm-4.7-free` 已删，`big-pickle` 保留
- [ ] `opencode_zen.models.default.default = "big-pickle"`（免费主力）
- [ ] JSON 合法（`python3 -m json.tool` 通过）
- [ ] `cargo build` 零 error（preset 编入 include_str!，需重编通过）
- [ ] `yarn build` 零 error（前端 defaults 派生层不崩）

## Definition of Done

- preset JSON 改完 + 合法性校验通过
- cargo build + yarn build 绿
- 不动 endpoints / Rust / 前端代码

## Out of Scope

- glm_coding 协议（独立 task `07-11-glm-coding-responses-peak-models`）
- peak_hours UI 预览行（独立 task `07-11-peak-hours-window-preview`）
- endpoints 字段改动（API 验证正确，无具体缺失说明不动）
- 远端同步链改动（仅改 bundled preset，同步机制不变）

## Open Questions

- #1 opencode（OpenCode Go）主力 default 模型选哪个（glm-5.2 / minimax-m3 / kimi-k2.7-code）？默认取 glm-5.2（与 glm_coding 主力一致）。
- #2 opencode_zen 除 default 外是否填 opus/sonnet/gpt/haiku slot？默认填（对齐其他协议 models 映射惯例）。

## Technical Notes

- 改动文件（仅 1）：`src-tauri/defaults/platform-presets.json`（opencode + opencode_zen 两 section）
- 校验：`python3 -c "import json; json.load(open('src-tauri/defaults/platform-presets.json'))"` + cargo build
- 与 `07-11-glm-coding-responses-peak-models` 共改同一 JSON 不同 section → **git 串行**（DAG 依赖边：本 task 与 glm_coding task 写文件冲突，不能并行派 subagent）
