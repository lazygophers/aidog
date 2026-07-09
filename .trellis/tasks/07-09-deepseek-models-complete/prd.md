# 补全 deepseek model_list+endpoints 全部官方信息

## Goal

DeepSeek API 平台仅 V4 系列（flash/pro）。research line 24 最终清单。历史 V3/R1/Coder/Math 为开源 GitHub 不在 API 销售。preset 含待弃用别名（deepseek-chat/deepseek-reasoner，2026/07/24 弃用）。

## Research References

- [`research/deepseek-models.md`](research/deepseek-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 按 research line 24 V4 系列清单（去待弃用别名 deepseek-chat/deepseek-reasoner）
### endpoints: 现状正确（含 /v1 + anthropic 路径）
### models.default: 补

## Acceptance Criteria

- [ ] model_list.default 按 research 推荐清单补全（JSON 合法 + 无重复）
- [ ] endpoints/models.default 按 Requirements 改动
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] `yarn build` clean（若前端无改动可跳）
- [ ] 不动 name/desc/source_urls/homepage/logo_url/client_type

## Definition of Done

- platform-presets.json 改动经 cargo test 通过
- cargo clippy 无新 warning
- JSON 结构完整

## Out of Scope

- V3/R1/Coder/Math（开源仓库非 API）、deepseek-chat/deepseek-reasoner（2026/07/24 弃用别名）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- endpoints 已正确不动
- research 清单出处见 `research/deepseek-models.md` 结论摘要段
