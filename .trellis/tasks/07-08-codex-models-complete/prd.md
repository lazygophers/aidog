# 补全 codex model_list+endpoints 全部官方信息

## Goal

OpenAI Codex（codex_tui 客户端）。research line 119 推荐 model_list 最终清单 + line 131 endpoints 保持现状（单 endpoint，不增 ChatGPT/数据驻留）。model_list 仅下拉展示用非路由键。

## Research References

- [`research/codex-models-endpoints.md`](research/codex-models-endpoints.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 按 research line 119 推荐清单补全（GPT-5.x codex 系列）
### endpoints.default: 保持现状单 endpoint（research line 131 论证）
### models.default.gpt: 保持
### passthrough.rs STATIC_MODEL_IDS codex 段同步（若有）

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

- ChatGPT OAuth/数据驻留端点（research line 131 论证不增）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json + passthrough.rs STATIC_MODEL_IDS（grep 定位对应段）
- endpoints 保持现状（research 论证充分）
- research 清单出处见 `research/codex-models-endpoints.md` 结论摘要段
