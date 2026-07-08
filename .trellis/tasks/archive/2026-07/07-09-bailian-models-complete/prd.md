# 补全 bailian(+bailian_coding) model_list+endpoints 全部官方信息

## Goal

`src-tauri/defaults/platform-presets.json` 中 `bailian` 协议的 `model_list` 仅 6 项（qwen3.7-max/plus, qwen3.6-flash, qwen3.5-omni-plus, qwen3-coder-plus/flash），遗漏官方在售大量模型（VL/Math/MT/Doc/开源系列等）。本 task 按 research 全谱结论补全 `bailian` 与 `bailian_coding` 两协议 `model_list.default` 至官方全部在售模型，endpoints 现状经 research 确认可用保持不变。

## Research References

- [`research/bailian-models.md`](research/bailian-models.md) — 阿里云百炼全谱模型（主线文本/Coder/VL/Audio/Math/MT/Doc/开源）+ endpoints（OpenAI 兼容 + Anthropic 兼容 + 国际站）+ bailian_coding 套餐子集 + 认证方式

## Requirements

### bailian（主协议）model_list.default — 全谱补全

preset 现状 6 项 → 补全至官方在售全谱（去日期快照去重，research line 67-300）：

**主线文本对话**（Max/Plus/Flash/Turbo/Long/Omni）：
- `qwen3.7-max`, `qwen3.7-max-preview`, `qwen3.6-max-preview`, `qwen3-max`, `qwen3-max-preview`, `qwen-max`
- `qwen3.7-plus`, `qwen3.6-plus`, `qwen3.5-plus`, `qwen-plus`, `qwen-plus-latest`
- `qwen3.6-flash`, `qwen3.5-flash`, `qwen-flash`
- `qwen-turbo`
- `qwen-long`, `qwen-long-latest`
- `qwen3.5-omni-plus`, `qwen3.5-omni-flash`

**Coder 系列**：
- `qwen3-coder-plus`, `qwen3-coder-flash`, `qwen-coder-plus`, `qwen-coder-turbo`

**VL 视觉理解**：
- `qwen3-vl-plus`, `qwen3-vl-flash`, `qwen-vl-max`, `qwen-vl-plus`, `qwen3.5-ocr`, `qwen-vl-ocr`, `qwen-vl-ocr-latest`

**Math 数学**：
- `qwen-math-plus`, `qwen-math-turbo`

**MT/Doc/Research 专用**：
- `qwen-mt-plus`, `qwen-mt-flash`, `qwen-mt-lite`, `qwen-mt-turbo`, `qwen-doc-turbo`, `qwen-deep-research`

**开源系列**（社区模型托管，research line 249-272）：
- `qwen3.6-35b-a3b`, `qwen3.6-27b`
- `qwen3.5-397b-a17b`, `qwen3.5-122b-a10b`, `qwen3.5-27b`, `qwen3.5-35b-a3b`
- `qwen3-next-80b-a3b-thinking`, `qwen3-next-80b-a3b-instruct`, `qwen3-235b-a22b-thinking-2507`, `qwen3-235b-a22b-instruct-2507`, `qwen3-30b-a3b-thinking-2507`, `qwen3-30b-a3b-instruct-2507`

**排除**（research line 274-287）：
- Qwen-Audio 系列（`qwen-audio-turbo` 等）— 官方已退役，推荐迁移 Qwen-Omni
- 日期快照版本（`-2026-xx-xx` 后缀）— 与主版本能力相同，去重
- `tongyi-xiaomi-analysis-flash` — research 标小米分析，非百炼主线（research line 247）

### bailian_coding（套餐）model_list.default — 套餐子集补全

preset 现状 3 项 → research 确认套餐 = 普通严格子集（Coder + 通用推理，无 VL/Audio/专用模型，research line 456-529）：

- `qwen3-coder-plus`, `qwen3-coder-flash`, `qwen3.7-max`
- 补 `qwen-coder-plus`, `qwen-coder-turbo`（旧版 Coder，research line 481 标未决但属 Coder 系列，套餐应支持）

**排除**：VL/Math/MT/Doc/开源 — 套餐端点不支持（research line 526）

### endpoints — 不变

preset 现状经 research 确认可用：
- bailian: `https://dashscope.aliyuncs.com/compatible-mode/v1`（OpenAI 兼容）+ `https://dashscope.aliyuncs.com/apps/anthropic`（Anthropic 兼容）— 旧域名仍可用（research line 64）
- bailian_coding: `https://coding.dashscope.aliyuncs.com/apps/anthropic`（编程专用域名）

**未决（不阻塞本 task）**：国际站域名（dashscope-us/intl）+ WorkspaceId 新域名 — 标后续迭代（research caveats 2/3）。

### models.default.default — 不变

- bailian: `qwen3.7-max`（research line 180 确认主推）
- bailian_coding: `qwen3-coder-plus`（research line 485 确认套餐主力）

## Acceptance Criteria

- [ ] `bailian` 协议 `model_list.default` 含上述全谱（主线文本 + Coder + VL + Math + MT/Doc/Research + 开源），去日期快照/去 Audio/去 tongyi-xiaomi
- [ ] `bailian_coding` 协议 `model_list.default` 含 Coder 系列（qwen3 + qwen 旧版）+ qwen3.7-max，无 VL/Math/专用模型
- [ ] `endpoints` 两协议保持现状（dashscope.aliyuncs.com + coding.dashscope.aliyuncs.com）
- [ ] `models.default.default` 两协议保持现状（qwen3.7-max / qwen3-coder-plus）
- [ ] JSON 合法（`python3 -m json.tool` 通过）
- [ ] 无重复 model id

## Definition of Done

- platform-presets.json 改动经 `cargo test`（defaults 相关 test）通过
- `cargo clippy` 无新 warning
- JSON 结构完整，`version` 不变（本 task 改 model_list 内容，非 schema）

## Out of Scope

- 国际站域名（dashscope-us/intl）+ WorkspaceId 新域名迁移（research caveats 2/3，后续迭代）
- Anthropic 兼容端点实测验证（research caveats 1，需 API key）
- `bailian_intl` 新协议（无此需求）
- `peak_hours` / `coding_plan` 分支（本 task 仅 default 分支 model_list）
- pricing 字段补全（独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 `protocols.bailian.model_list.default` + `protocols.bailian_coding.model_list.default` 两数组
- 无 Rust 代码改动（preset JSON 由 `get_defaults_json` 运行时读取）
- research 全谱清单出处 = 阿里云百炼计费页（2026-07-07 更新，research line 326）
