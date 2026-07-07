# PRD: platform preset 数据补全

## Goal

很多平台 preset 的 base_url / model 信息不全。扫 `src-tauri/defaults/platform-presets.json` 60 协议定位真缺失，补全让用户建 platform 时有合理默认 model + 完整 endpoint 字段。

## What I already know（auto-context 扫描发现）

### 真缺失（3 类）

**① model_list.default 空**（11 协议，用户建 platform 无模型可选）:
- siliconflow / siliconflow_en / bailian / bailian_coding / bailing / qianfan / longcat / therouter / compshare / opencode / newapi / gemini

**② models.default 空 dict**（无默认 model）:
- gemini `{"default": {}}`、siliconflow `{"default": {}}`（与 ① 重叠）

**③ endpoint 缺 client_type**（6 协议 default[2]）:
- gemini[0]、openrouter[2]、packycode[2]、cubence[2]、aigocode[2]、aicodemirror[2]
- 例: packycode[2] = `{protocol:"gemini", base_url:"https://www.packyapi.com"}` 缺 client_type

### 结构参考（正确范式）

- models = `{"<endpoint_branch>": {"<client_type>": "<model_id>"}}`（doubao `{"default": {"default": "doubao-seed-2-0-code"}}`；openai `{"default": {"gpt": "gpt-5.5"}}`）
- model_list = `{"<branch>": ["model_id", ...]}`
- endpoint 必含 protocol + base_url + client_type

### 非缺失（by design 可能）

- model_list 空的聚合平台（siliconflow/newapi/openrouter 类）模型海量动态，可能故意留空让用户自填 —— 待用户确认
- 维度 1/5 全过（endpoints / name / desc / source_urls 无缺）

## Decision (ADR-lite, best judgment, 用户打断 AskUserQuestion 按推荐推进)

**Context**: 「信息不全」scope 模糊（用户打断多选问）。auto-context 探明 + 最佳判断推进。

**Decision**:
- **① model_list 空（11 协议）= 主修**: 分类处理
  - **聚合平台**（siliconflow/siliconflow_en/newapi/therouter，模型海量动态）: 留空 by design（用户自填）
  - **非聚合平台**（gemini/bailian/bailian_coding/bailing/qianfan/longcat/compshare/opencode）: research 补关键模型 3-5 个旗舰
- **② default model 空（gemini/siliconflow models.default={}）= 主修**: research 查官网补旗舰作默认
- **③ client_type 缺（6 协议 endpoint 全 gemini protocol）= 降级可选**: platform.rs:121 `_ => ClientType::Default` + line 128 容错 deserialize 已 fallback Default，缺 client_type **非阻塞**。显式补 `"default"` 为数据完整性（机械，无需外部数据）
- **数据来源**: research subagent 并行查各平台官网/文档

**Consequences**:
- ①② 需 research（外部数据）；③ 机械补
- 聚合平台留空（by design）
- scope 若与用户预期不符，返工调整（标开放）

## Requirements

- ③ 6 协议 endpoint（gemini/openrouter/packycode/cubence/aigocode/aicodemirror 的 gemini protocol 端点）补 `"client_type": "default"` 显式
- ② gemini/siliconflow models.default 空: research 查官网补旗舰 default model
- ① 非聚合平台 model_list 空（gemini/bailian/bailian_coding/bailing/qianfan/longcat/compshare/opencode）: research 补关键模型 3-5 个
- 聚合平台（siliconflow/siliconflow_en/newapi/therouter）model_list 留空 by design（不动）
- 全 protocol 扫确认补全后无"主修"类缺失
- json 有效 + cargo build/test 不回归

## Subtask 拆分（exec 阶段）

- **ST1（机械，无外部数据）**: ③ 6 协议 client_type 补 `"default"` + 全 protocol 扫验证
- **ST2（research + 填充）**: ② default model + ① 非聚合 model_list，派 trellis-research 并行查各平台官网，汇总后填
- ST1 与 ST2 同文件（platform-presets.json）不同协议段 → git 3-way 可合，但保守**串行**（ST1 先机械快速，ST2 后 research 填）

## Requirements（evolving）

- 扫全 60 协议定位真缺失（已完成）
- 按用户确认的 scope 补全
- json 有效 + cargo build/test 不回归

## Acceptance Criteria（evolving）

- [ ] `python3 -m json.tool` 有效
- [ ] 用户确认的缺失类全补
- [ ] 无 endpoint 缺 client_type（若 scope 含③）
- [ ] cargo build/test 不回归

## Out of Scope

- doubao 结构错位（归 07-08-doubao-preset-dup-fix）
- codex protocol 改（归 07-07-codex-responses-api）
- 不改非缺失协议

## Technical Notes

- 改点: `src-tauri/defaults/platform-presets.json` 多协议条目
- 调度: 与 doubao/codex 同文件 → 串行（冲突自算）
- 数据来源: research subagent 并行查官网（若 scope 含需外部数据的维度）
- Protocol→ClientType 映射: `src-tauri/src/gateway/models/platform.rs:120`
