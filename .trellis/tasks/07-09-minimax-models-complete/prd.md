# 补全 minimax(+minimax_en) model_list+endpoints 全部官方信息

## Goal

MiniMax。research line 24 文本对话主线 M 系列最终清单。highspeed/lightning 变体是官方独立 id 应加入。abab 已废弃排除。

## Research References

- [`research/minimax-models.md`](research/minimax-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### minimax + minimax_en model_list.default: 按 research line 24 主线清单（M1/M2/M2.1/M2.5/M2.7/M3 + highspeed/lightning 变体）
### models.default: MiniMax-M3（research 确认推荐）
### endpoints: 两协议各自

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

- abab 系列（已废弃，被 M 系列取代）、遗留/非主线（research 标）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 两协议同源
- research 清单出处见 `research/minimax-models.md` 结论摘要段
