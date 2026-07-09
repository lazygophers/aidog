# Implement: AtlasCloud 补全 model_list（114）+ openai 端点 + models.default

## 载体
- 单 subtask（单文件 protocols.atlascloud 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 src-tauri/defaults/platform-presets.json 的 protocols.atlascloud 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤
1. 读 research/atlascloud-models.md（第 35-179 行为全量 114 模型清单）
2. 读 prd.md
3. 读现有 protocols.atlascloud 块定位（grep 行号）
4. 改 endpoints（新增 openai 端点）/ model_list（114 模型全量替换）/ models.default（三档）/ desc 保留不变 / source_urls 保留不变
5. 验证 JSON 合法
6. 验证：python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['atlascloud'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"
   - 预期输出：114 {'default': 'openai/gpt-5.5', 'opus': 'anthropic/claude-opus-4.8', 'haiku': 'anthropic/claude-haiku-4.5-20251001'} 2

## 验收（对齐 prd）
- endpoints 2（anthropic 保留 + openai 新增）
- model_list 114（15 provider 全量，provider/ 前缀，大小写与 /v1/models 一致）
- models.default = {"default":"openai/gpt-5.5","opus":"anthropic/claude-opus-4.8","haiku":"anthropic/claude-haiku-4.5-20251001"}（档位名 key → string）
- desc 保留不变
- source_urls 保留
- JSON 合法
- 仅改 atlascloud 块

## 关键改动 delta
- endpoints.default：从 1 个（anthropic only）扩为 2 个（新增 openai /v1 codex_tui）
- model_list.default：从 11 项全量替换为 114 项（按 research 第 35-179 行逐条填入，provider/ 前缀格式）
- models.default：从 {} 填入三档（档位名 key → model id string，对齐 Partial<Record<ModelSlot, string>>）
- desc / source_urls：不动

## 模型清单填写要点
- 按 research 第 35-179 行的分组顺序填写（anthropic → bytedance → deepseek-ai → google → kwaipilot → meituan-longcat → minimaxai → moonshotai → openai → Qwen → qwen → tencent → xai → xiaomi → zai-org）
- provider 全小写，唯一例外 `Qwen/`（Q 大写）
- model-name 保留原始大小写（如 DeepSeek-V3.1、GLM-4.6）
- coding 后缀变体保留（claude-opus-4.8-coding 等）

## 失败处理
- JSON 解析失败 → 检查逗号/引号（114 项数组，易漏逗号）
- 字段定位错 → grep protocols.atlascloud 核行号
- 模型数不为 114 → 逐 provider 核对 research 清单

## 禁
- 禁动其他协议块
- 禁用 model-id 空 obj（value 必须 string，档位名 key 才是正确格式，对齐 Partial<Record<ModelSlot, string>>）
- 禁改 provider 大小写（Qwen/Q 大写是官方规范）
- 禁动 desc / source_urls
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
