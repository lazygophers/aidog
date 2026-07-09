# 补全 modelscope model_list+endpoints 全部官方信息

## Goal

ModelScope 官方推理 API（api-inference.modelscope.cn）支持 55 个模型 / 18 provider。preset 现 12 项精选 + 仅 anthropic 单端点 + models.default 空。补全 model_list 至全量 55 项，补 openai 兼容端点，补 models.default 三档。

## Research References

- [`research/modelscope-models.md`](research/modelscope-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 12→55（官方推理 API 全量，research line 9-55 按 provider 分组）
### endpoints.default: 1→2（anthropic 现状 + 补 openai https://api-inference.modelscope.cn/v1 client_type=codex_tui）
### models.default.default: {}→三档（default=Qwen/Qwen3.5-397B-A17B, coder=Qwen/Qwen3-Coder-30B-A3B-Instruct, fast=deepseek-ai/DeepSeek-V4-Flash）

> 格式：档位名 key → model id string（对齐 aidog 真实约定 `Partial<Record<ModelSlot,string>>`，与 anthropic/gemini/glm 等 20 个官方 protocol 一致）。

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

- 社区数千开源模型全集（非官方推理 API 支持）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 模型 id 格式 Provider/Model 前缀（research 验证）
- research 清单出处见 `research/modelscope-models.md` 结论摘要段
