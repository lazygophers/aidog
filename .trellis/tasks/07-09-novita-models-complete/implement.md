# Implement: novita 补 139 模型 + openai 端点 + 三档 models.default

## 载体
- 单 subtask（单文件 protocols.novita 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.novita` 块
- 禁动其他协议块、顶层 version/last_updated

## 步骤
1. 读 `research/novita-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.novita` 块定位
4. endpoints.default 新增 openai（`https://api.novita.ai/v3/openai`，codex_tui），保留现有 anthropic
5. model_list.default → 139 模型（`provider/model` 前缀，按 prd provider 分组顺序）
6. models.default → 三档 `{"default":"zai-org/glm-5.2","coder":"moonshotai/kimi-k2.7-code","fast":"deepseek/deepseek-v4-flash"}`（档位名 key → model id string，符合 aidog `Partial<Record<ModelSlot, string>>` 约定）
7. desc / source_urls 保留不动
8. 验证 JSON 合法
9. 验证命令：
```bash
python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['novita'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"
```
期望输出：`139 {'default': 'zai-org/glm-5.2', 'coder': 'moonshotai/kimi-k2.7-code', 'fast': 'deepseek/deepseek-v4-flash'} 2`

## 验收（对齐 prd）
- endpoints.default = 2（新增 openai `/v3/openai`）
- model_list.default = 139（`provider/model` 前缀）
- models.default 三档 档位名 key → string（default/coder/fast）
- desc / source_urls 保留
- JSON 合法

## 失败处理
- JSON 解析失败 → 检查逗号/引号（139 行长数组易漏逗号）
- model_list 计数 != 139 → 对照 prd 各 provider 块逐项核对，注意 qwen 35 / deepseek 21 等大块
- models.default value 不是 string（写成空 obj / 数组）→ 违反 aidog 约定，改为档位名 key → model id string

## 禁
- 禁动其他协议块
- 禁用 model-id 空 obj 作 models.default value（必须档位名 key → string）
- 禁加 id 日期后缀
- 禁动 STATIC_MODEL_IDS
- 禁改 `/v1/models`（404 不可用，openai 端点用 `/v3/openai`）
- 禁 git commit
