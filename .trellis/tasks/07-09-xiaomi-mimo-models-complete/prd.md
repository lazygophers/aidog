# 补全 xiaomi_mimo model_list+endpoints 全部官方信息

## Goal

小米 MiMo。research 核实现 preset model_list（mimo-v2.5-pro/v2-pro/v2.5/v2-omni/v2-flash）+ endpoints 配置正确。补全建议按 research。

## Research References

- [`research/xiaomi-mimo-models.md`](research/xiaomi-mimo-models.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### model_list.default: 按 research 建议清单核实/补全
### endpoints: research 核实配置正确（Pay-as-you-go），保持
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

- 无（research 核实配置正确）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 认证 api-key 请求头（非 Authorization）
- research 清单出处见 `research/xiaomi-mimo-models.md` 结论摘要段
