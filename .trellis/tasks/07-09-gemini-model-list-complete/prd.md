# 补全 gemini model_list 全部官方模型

## Goal

Google Gemini。preset 现 4 模型全合法（gemini-2.5-pro/2.5-flash/2.5-flash-lite/3.5-flash）。research line 98 推荐最终清单补全。

## Research References

- [`research/gemini-models.md`](research/gemini-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 按 research line 98 推荐清单补全（Gemini 2.5/3/3.5 系列）
### endpoints: 保持
### models.default: 保持
### passthrough.rs STATIC_MODEL_IDS gemini 段同步（若有）

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

- 已退役/preview-only（research 标）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json + passthrough.rs STATIC_MODEL_IDS（grep 定位对应段）
- preset 现 4 模型 research 核实全合法
- research 清单出处见 `research/gemini-models.md` 结论摘要段
