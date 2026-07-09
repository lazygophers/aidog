# 补全 longcat model_list+endpoints 全部官方信息

## Goal

美团 LongCat 自研平台，当前仅 LongCat-2.0 一个模型（1.6T MoE, 1M 上下文）。preset 现 model_list 空 + models.default 空。补全单模型 + endpoint 路径修正。

## Research References

- [`research/longcat-models.md`](research/longcat-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: []→["LongCat-2.0"]
### models.default.default: {}→{"default":"LongCat-2.0"}
### endpoints.default: 修正 OpenAI 端点路径（research 标现 https://api.longcat.chat/openai/v1 疑多 /v1，需与 provider_api_path() 交叉验证）；anthropic 端点正确

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

- 无（单模型平台）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- OpenAI endpoint base_url 路径需与项目 URL 构造约束交叉验证（provider_api_path 只返回 /chat/completions）
- research 清单出处见 `research/longcat-models.md` 结论摘要段
