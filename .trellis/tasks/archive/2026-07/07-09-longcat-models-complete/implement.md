# Implement: longcat 补 model_list + models.default

## 载体

- 单 subtask（protocols.longcat 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.longcat` 块
- 禁动其他无关协议块、顶层 version/last_updated、endpoints/desc/source_urls/name/homepage/logo_url

## 步骤

1. 读 `research/longcat-models.md`（确认单一模型 LongCat-2.0）
2. 读 `prd.md`（确认 endpoints 保留、仅改 model_list + models）
3. 读现有 `protocols.longcat` 块定位
4. 改：
   - `model_list.default`: `[]` → `["LongCat-2.0"]`
   - `models.default`: `{}` → `{"default":"LongCat-2.0"}`
5. 验证 JSON 合法
6. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['longcat'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"`
   - 期望输出：`1 {'default': 'LongCat-2.0'} 2`

## 验收（对齐 prd）

- `model_list.default = ["LongCat-2.0"]`
- `models.default = {"default":"LongCat-2.0"}`
- endpoints 数 = 2（未动）
- desc/source_urls/name 未动
- cargo build/clippy/test clean

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- cargo test 失败 → 检查是否误改其他协议块

## 禁

- 禁动其他无关协议块
- 禁用 model-id 空 obj（`{}`）
- 禁动 endpoints（研究文档的"去 /v1"建议本身错误，不符合项目 URL 构造约束）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
