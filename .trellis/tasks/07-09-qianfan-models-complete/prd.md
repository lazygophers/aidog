# 补全 qianfan model_list+endpoints 全部官方信息

## Goal

百度千帆。research line 87 主线文本对话清单。/v1 端点 404，千帆无公开 OpenAI 兼容端点，仅国内端点。

## Research References

- [`research/qianfan-models.md`](research/qianfan-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 按 research line 87 主线文本清单补全（ERNIE 4.5 Turbo 系列）
### endpoints: research 核实无 openai 兼容端点（/v1 404），保持现状或仅 anthropic
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

- 非主线（语音/图像等，research 标）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 无公开 openai 端点（research 实证 /v1 404）
- research 清单出处见 `research/qianfan-models.md` 结论摘要段
