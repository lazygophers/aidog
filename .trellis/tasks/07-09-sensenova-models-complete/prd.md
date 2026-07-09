# 补全 sensenova model_list+endpoints 全部官方信息

## Goal

商汤日日新 SenseNova。preset 现 3 项（sensenova-6.7-flash-lite, deepseek-v4-flash 转发, sensenova-u1-fast）+ 2 端点。research 核实官方自研模型清单 + anthropic 端点路径。

## Research References

- [`research/sensenova-models.md`](research/sensenova-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 按 research 官方自研模型清单补全（sensenova 系列 + u1/fast 等）
### endpoints.default: 核实 anthropic 端点 https://token.sensenova.cn 路径正确性（research 标待确认）
### models.default: 补 default 档

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

- deepseek-v4-flash（第三方转发，非商汤自研，research 标推测）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- anthropic 端点 research 标待确认，implement 核实
- research 清单出处见 `research/sensenova-models.md` 结论摘要段
