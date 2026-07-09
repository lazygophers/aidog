# 补全 glm(+glm_en) model_list+endpoints 全部官方信息

## Goal

智谱 GLM。glm（国内 bigmodel.cn）+ glm_en（国际）同源镜像。preset 现 model_list 单分支。research 查官方全谱（GLM-5/5.1/5.2 + 4.x 系列 + Z1 air）。

## Research References

- [`research/glm-models-endpoints.md`](research/glm-models-endpoints.md) — research 全文（model_list 最终清单 + endpoints 核实 + 排除项 + 结论摘要）

## Requirements

### glm + glm_en model_list.default: 按 research 全谱补全（两协议同清单，仅域名异 .cn vs .com）
### endpoints: 两协议各自保持（research line 3.4 比对）
### models.default: 保持或补

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

- GLM-5.2 1M 上下文（[1m] 后缀启用，非独立 id，research line 6772）、已退役系列
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 platform-presets.json
- 两协议同源，单 task 覆盖
- research 清单出处见 `research/glm-models-endpoints.md` 结论摘要段
