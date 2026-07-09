# 补全 kimi model_list+endpoints 全部官方信息

## Goal

Moonshot Kimi。research line 19/66 给最终 model_list 清单。preset 现状补全至官方全谱（K2/K2.5/K2.6/K2.7-code 系列）。

## Research References

- [`research/kimi-models.md`](research/kimi-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 按 research line 66 推荐清单整组替换
### endpoints.default: 按 research 推荐核实/补全
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

- 研究/preview 模型（research 标）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 
- research 清单出处见 `research/kimi-models.md` 结论摘要段
