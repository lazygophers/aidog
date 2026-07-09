# ctok 转发协议模型清单核实

## Goal

延续 07-09-platform-presets-overhaul。用户最早控诉 ctok model_list/models/endpoints 缺信息。ST7 仅补 default slot（claude-opus-4-8）+ model_list 加 claude-sonnet-5，但 ctok 作为聚合转发站，其**转发协议（OpenAI / Gemini）**对应的模型清单（gpt-5.5 / gemini-3-flash 等）未核实，是否还有遗漏未知。本轮专攻 ctok 全协议模型清单完整性。

## What I already know

- 真值源 `src-tauri/defaults/platform-presets.json` ctok 条目（ST7 后态）
- ST7 已补：models.default.default = claude-opus-4-8；model_list 加 claude-sonnet-5
- ctok = 聚合转发站，多协议（anthropic/openai/gemini）多模型
- ST7 research/ctok.md 标 `需要: CTok /v1/models 返回样例`（未拿到实测清单）

## Requirements

- R1: WebFetch CTok 官方文档（https://ctok.ai 内 docs / 模型列表页）核实全协议支持的模型清单
- R2: diff 现有 model_list vs 官方，列遗漏 / 过时 / 臆造 id
- R3: 核实 endpoints base_url（openai/gemini/anthropic 三协议）是否最新
- R4: 核实 models 各 slot（default/sonnet/opus/haiku/gpt）映射值是否合理
- R5: 产出 research/ctok-forward.md（source URL + diff 表 + 补齐建议），不改 JSON

## Acceptance

- [ ] research/ctok-forward.md 含官方 source URL
- [ ] 全协议（anthropic/openai/gemini）模型清单 diff
- [ ] base_url 三协议核实
- [ ] 遗漏模型逐个列（含官方 id 拼写）

## Out of Scope

- 改 platform-presets.json（下轮 implement task，依 research 拍板）
- 其他协议（仅 ctok）
- 价格同步（另 task）

## Open Questions

- OQ1: CTok 官方文档是否公开模型全清单？若无公开页，需用户提供 API key 实测 /v1/models（research agent 标 `需要:`）
