# 补全 aihubmix model_list+endpoints 主流模型

## Goal

AiHubMix 聚合。裸 id 格式（无 provider 前缀）。4 种协议 endpoints（含 gemini_api）。用户选全量 scope。

## Research References

- [`research/aihubmix-models.md`](research/aihubmix-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 14→全量（裸 id 格式，research 全清单）
### endpoints.default: 按 research 4 协议补全（anthropic + openai + gemini_api + ?）
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

- 无（全量 scope）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 裸 id 格式（无 provider 前缀，research 验证）
- research 清单出处见 `research/aihubmix-models.md` 结论摘要段
