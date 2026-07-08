# 补全 shengsuanyun(盛算云) model_list+endpoints+models 全部官方信息

## Goal

`src-tauri/defaults/platform-presets.json` 中 `shengsuanyun` 协议的 `model_list` 仅 13 项精选旗舰，遗漏官方支持的大量模型（research 实测官方 API 返回 **172 个模型 / 19 家 provider**）。本 task 按用户选「全量」scope 补全 `model_list.default` 至官方全量 172 项，补 openai + gemini 端点（preset 现仅 anthropic 单端点），并补 `models.default` 三档默认。

## Research References

- [`research/shengsuanyun-models.md`](research/shengsuanyun-models.md) — 盛算云官方 API 实拉 172 模型清单（19 provider 分组）+ endpoints 核实（anthropic/openai/gemini 三协议路径）+ models.default 建议 + 认证方式

## Requirements

### model_list.default — 全量补全（13 → 172）

preset 现状 13 项 → 补全至官方 API `https://router.shengsuanyun.com/api/v1/models` 返回的全量 172 项（research line 39-247，按 provider 分组）：

**Anthropic (17)** / **OpenAI (32)** / **Google (10)** / **DeepSeek (9)** / **Ali/Qwen (26)** / **Bigmodel/GLM (19)** / **Moonshot/Kimi (6)** / **MiniMax (8)** / **Bytedance/Doubao (13)** / **x-ai/Grok (4)** / **Xiaomi/MiMo (5)** / **Tencent/Hunyuan (5)** / **Baidu/Ernie (5)** / **Intern (4)** / **Streamlake (3)** / **StepFun (2)** / **Meta (2)** / **Xai (1)** / **Longcat (1)**

**模型 id 命名格式**：`provider/model-name` 前缀格式（research line 251-262，API `api_name` 字段验证）。provider 前缀小写，模型名保留原始大小写，特殊后缀用冒号（`:thinking` / `:latest`）。

**完整清单**：照搬 research line 39-247 全量列表，按 provider 顺序排列，无去重无筛选（用户选全量 scope）。

**需核实项**（research caveats line 389）：
- `openai/gpt-5.3-codex` — preset 现有但 API 172 项中未找到。本 task 按全量 scope 以 API 返回为准，preset 现有项若不在 API 返回中则**移除**（preset 真值源 = 官方 API，非历史遗留）。

### endpoints.default — 三端点全补（1 → 3）

preset 现状仅 anthropic 单端点 → 补全三协议端点（research line 301-330，参照 openrouter preset 格式）：

```json
[
  {"protocol": "anthropic", "base_url": "https://router.shengsuanyun.com/api", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://router.shengsuanyun.com/api/v1", "client_type": "codex_tui"},
  {"protocol": "gemini", "base_url": "https://router.shengsuanyun.com/api", "client_type": "default"}
]
```

- anthropic: base_url `https://router.shengsuanyun.com/api` + `/v1/messages`
- openai: base_url `https://router.shengsuanyun.com/api/v1` + `/chat/completions`（含 `/v1` 版本前缀，符合项目 URL 构造约束）
- gemini: base_url `https://router.shengsuanyun.com/api` + `/v1beta/models/*`（原生 gemini 协议，client_type=default）

**URL 构造合规**（项目 CLAUDE.md 约束）：base_url 含版本前缀（openai 端点 `/api/v1`），`provider_api_path()` 只返回 `/chat/completions`，最终 URL = base_url + provider_api_path，禁额外拼接。

### models.default.default — 三档默认（空 → 三档）

preset 现状 `{}`（空）→ 补三档（research line 332-357）：

```json
{
  "default": "anthropic/claude-sonnet-4.6",
  "coder": "ali/qwen3-coder-plus",
  "fast": "google/gemini-3.5-flash"
}
```

## Acceptance Criteria

- [ ] `model_list.default` 含官方 API 全量 172 项（按 provider 分组顺序），无重复
- [ ] preset 原 13 项中不在 API 返回的项（如 `openai/gpt-5.3-codex` 若核实缺失）已移除
- [ ] `endpoints.default` 含三协议端点（anthropic + openai + gemini），base_url 路径符合项目 URL 构造约束
- [ ] `models.default.default` 含三档（default/coder/fast）
- [ ] JSON 合法（`python3 -m json.tool` 通过）
- [ ] 无重复 model id

## Definition of Done

- platform-presets.json 改动经 `cargo test`（defaults 相关 test）通过
- `cargo clippy` 无新 warning
- JSON 结构完整，`version` 不变（本 task 改内容非 schema）

## Out of Scope

- 认证方式字段补全（research caveats 标推测 Bearer，非 preset 字段）
- `peak_hours` / `coding_plan` 分支（本 task 仅 default 分支）
- pricing 字段补全（独立 task）
- preset 原 13 项中 `openai/gpt-5.3-codex` 的去留最终以 implement 阶段实拉 API 核实为准（若 API 实际有则保留，无则移除）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖）
- 改动范围：单文件 `protocols.shengsuanyun` 的 `model_list.default` + `endpoints.default` + `models.default.default` 三字段
- 无 Rust 代码改动（preset JSON 由 `get_defaults_json` 运行时读取）
- research 全量清单出处 = 官方 API `https://router.shengsuanyun.com/api/v1/models`（2026-07-09 实拉，research line 9-247）
- preset 膨胀风险：172 项为各聚合平台最大，高频腐化（月级模型上下架），标后续需周期性同步
