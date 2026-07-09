# 补全 doubao(+byteplus) model_list+models 全部官方信息

## Goal

字节豆包。doubao 国内 + byteplus 国际。research line 35/166 两份清单（国内 Seed 系列 + 国际子集）。国际版是国内子集，仅聚合 GLM/DeepSeek，不含 MiniMax/Kimi。

## Research References

- [`research/doubao-models.md`](research/doubao-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### doubao model_list.default: 按 research line 35 国内 Seed 系列补全（Seed 1.6/1.8/2.0/2.1 + character/evolving）
### byteplus model_list.default: 按 research line 166 国际子集（模型 id 格式 seed-* 非 doubao-seed-*，research caveat 7）
### endpoints: 套餐端点现状保持

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

- 非主线（research 标）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- byteplus 国际版模型 id 格式可能需 seed-* 非 doubao-seed-*（research caveat 7 待 implement 核实）
- research 清单出处见 `research/doubao-models.md` 结论摘要段
