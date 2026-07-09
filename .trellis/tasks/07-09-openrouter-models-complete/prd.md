# 补全 openrouter model_list+endpoints 主流旗舰模型

## Goal

OpenRouter 聚合 344 模型 / 57 provider。preset 现 15 项精选旗舰 + 三端点（含 gemini）。research 实证 OpenRouter 不支持 gemini 原生协议 → 删 gemini endpoint。用户选全量 scope。

## Research References

- [`research/openrouter-models.md`](research/openrouter-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 15→全量（research line 624 总数 344，按 provider 分组整组替换）
### endpoints.default: 3→2（删 gemini 端点，research 实证不支持 gemini 原生协议；保留 anthropic + openai）
### models.default: 保持现状或补 default 档

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

- gemini 原生端点（research 实证 OR 不支持）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 344 项 preset 膨胀风险，月级腐化
- research 清单出处见 `research/openrouter-models.md` 结论摘要段
