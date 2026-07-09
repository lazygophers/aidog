# Implement: APINebula 补全 model_list + desc + models.default

## 载体
- 单 subtask（单文件 protocols.apinebula 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 src-tauri/defaults/platform-presets.json 的 protocols.apinebula 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤
1. 读 research/apinebula-models.md
2. 读 prd.md
3. 读现有 protocols.apinebula 块定位（grep 行号）
4. 改 model_list（20 模型）/ models.default（三档）/ desc（8 语言改写）/ endpoints 保留不变 / source_urls 保留不变
5. 验证 JSON 合法
6. 验证：python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['apinebula'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"

## 验收（对齐 prd）
- endpoints 3（不变）
- model_list 20（8 Claude + 5 GPT + 6 Gemini + 1 custom）
- models.default = {"default":"claude-sonnet-4-6","opus":"claude-opus-4-8","haiku":"claude-haiku-4-5"}（档位名 key → string）
- desc 8 语言改写为 AI 模型聚合平台
- source_urls 保留
- JSON 合法
- 仅改 apinebula 块

## 关键改动 delta
- model_list.default：删 claude-opus-4-5（pricing 未发现）；新增 claude-fable-5 / claude-sonnet-5 / gpt-5.5 / gpt-5.4 / gpt-5.4-mini / gpt-5.5-openai-compact / gpt-image-2 / gemini-3.1-pro-preview / gemini-3.5-flash / gemini-2.5-pro / gemini-2.5-flash-lite / gemini-3-pro-image-preview / gemini-3.1-flash-image-preview / codex-auto-review（共 14 新增）；alias 模型保持裸 id 不加日期后缀
- models.default：从 {} 填入三档（档位名 key → model id string，对齐 Partial<Record<ModelSlot, string>>）
- desc：8 语言从「Claude 兼容模型」改为「AI 模型聚合平台」
- endpoints / source_urls：不动

## 失败处理
- JSON 解析失败 → 检查逗号/引号
- 字段定位错 → grep protocols.apinebula 核行号

## 禁
- 禁动其他协议块
- 禁用 model-id 空 obj（value 必须 string，档位名 key 才是正确格式，对齐 Partial<Record<ModelSlot, string>>）
- 禁加 id 日期后缀（alias 约定，claude-haiku-4-5 / claude-sonnet-4-5 保持裸 id）
- 禁删 endpoints（3 个全正确）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
