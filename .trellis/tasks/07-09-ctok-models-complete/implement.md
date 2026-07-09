# Implement: ctok 补 endpoints + models.default + 改 desc/source_urls（保守）

## 载体
- 单 subtask（单文件 protocols.ctok 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.ctok` 块
- 禁动其他协议块、顶层 version/last_updated

## 步骤
1. 读 `research/ctok-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.ctok` 块定位
4. endpoints.default 新增 openai（`https://api.ctok.ai/v1`，codex_tui）+ gemini（`https://api.ctok.ai`，default），保留现有 anthropic
5. models.default → 三档 `{"opus":"claude-opus-4-8","sonnet":"claude-sonnet-4-6","haiku":"claude-haiku-4-5"}`（档位名 key → model id string）
6. model_list.default 保留 7 alias 不动
7. desc 8 语言改为「多供应商网关 Claude/GPT/Gemini」
8. source_urls docs/pricing 改为 `https://api.ctok.ai/`（原 404）
9. 验证 JSON 合法
10. 验证命令：
```bash
python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['ctok'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"
```
期望输出：`7 {'opus': 'claude-opus-4-8', 'sonnet': 'claude-sonnet-4-6', 'haiku': 'claude-haiku-4-5'} 3`

## 验收（对齐 prd）
- endpoints.default = 3（新增 openai + gemini）
- model_list.default = 7（保留）
- models.default 三档 档位名 key → string（opus/sonnet/haiku）
- desc 多供应商定位（8 语言）
- source_urls 非 404
- JSON 合法

## 失败处理
- JSON 解析失败 → 检查逗号/引号
- desc 某语言漏改 → 对照 prd 8 语言全量替换

## 禁
- 禁动其他协议块
- 禁用 model-id 空 obj 作 models.default value（必须档位名 key → string）
- 禁臆造 GPT/Gemini 模型 id 加入 model_list（数据弱，待验证）
- 禁加 id 日期后缀
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
