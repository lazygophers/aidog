# 补全 stepfun(+stepfun_en) model_list+endpoints 全部官方信息

## Goal

阶跃 StepFun。research line 20 文本主线最终清单。preset 现 step-3.7-flash + step-3.5-flash。补全文本主线 + step-3.5-flash-2603（Step Plan 但 API 可调）。

## Research References

- [`research/stepfun-models.md`](research/stepfun-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### stepfun + stepfun_en model_list.default: 按 research line 20 文本主线清单补全
### endpoints: 两协议各自
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

- 语音 TTS/ASR、图像文生图/编辑（非文本推理，research 标）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- step-3.5-flash-2603 是否并入 research 标未决，implement 据 research 结论定
- research 清单出处见 `research/stepfun-models.md` 结论摘要段
