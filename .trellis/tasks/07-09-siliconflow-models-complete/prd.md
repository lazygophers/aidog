# 补全 siliconflow(+siliconflow_en) model_list+endpoints 全部官方信息

## Goal

硅基流动。国内 .cn + 国际 .com 同源。research line 222 模型总数估算。model_list 只含 chat 类型（排除 embedding/reranker/TTS/图像生成）。

## Research References

- [`research/siliconflow-models.md`](research/siliconflow-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### siliconflow + siliconflow_en model_list.default: 按 research chat 类型模型清单补全（两协议同清单，域名 .cn vs .com）
### endpoints: research 建议改 https://api.siliconflow.[cn|com]/v1 或路由层补 /messages
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

- embedding/reranker/TTS/图像生成（非 chat 对话模型，research 标）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 两协议同源，仅域名异
- research 清单出处见 `research/siliconflow-models.md` 结论摘要段
