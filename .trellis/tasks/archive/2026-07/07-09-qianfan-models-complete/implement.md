# Implement: qianfan 保守补 6 模型 + default 档

## 载体

- 单 subtask（`protocols.qianfan` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.qianfan` 块
- 禁动其他无关协议块、顶层 version/last_updated、endpoints/desc/source_urls/name/homepage/logo_url/client_type

## 步骤

1. 读 `research/qianfan-models.md`（确认数据强度弱、仅 6 控制台确认模型、其余推测）
2. 读 `prd.md`（确认保守策略：仅 6 模型 + 仅 default 档）
3. 读现有 `protocols.qianfan` 块定位
4. 改：
   - `model_list.default`：`[]` → `["ernie-5.1","ernie-5.0","ernie-4.5-turbo-vl","ernie-4.5-turbo","ernie-x1-turbo","ernie-x1.1-preview"]`
   - `models.default`：`{}` → `{"default":"ernie-4.5-turbo"}`
5. 验证 JSON 合法
6. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['qianfan'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"`
   - 期望输出：`6 {'default': 'ernie-4.5-turbo'} 1`

## 验收（对齐 prd）

- model_list.default = 6 模型（仅控制台确认项，无推测子型号）
- models.default = `{"default":"ernie-4.5-turbo"}`（仅 default 档，不强扩）
- endpoints 数 = 1（未动）
- desc/source_urls/name 不动
- cargo build/clippy/test clean

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- id 大小写疑问 → 全小写连字符（`ernie-4.5-turbo`），与 research 推测的 API 调用格式一致；如后续 API 实测发现官方用大写（如 `ERNIE-4.5-Turbo`），单独修 task 更正
- cargo test 失败 → 检查是否误改其他协议块

## 禁

- 禁动其他无关协议块
- 禁用 model-id 空 obj（`{}`）
- 禁动 endpoints（research 实证无 openai 端点，仅保留 anthropic `/anthropic/coding`）
- 禁将推测性 id（ernie-4.5-128k-preview / ernie-speed-pro-128k 等）写入 model_list（research 标"需验证"，等 API key 实测）
- 禁强扩 models.default 多档（id 未验证，避免误导路由）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
