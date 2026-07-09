# Implement: siliconflow(+siliconflow_en) 补 openai 端点 + 20 模型 + 3 档默认

## 载体

- 单 subtask（同源镜像两协议块：`protocols.siliconflow` + `protocols.siliconflow_en`）
- trellis-implement 在 task worktree 内内联执行
- 无 subtask 拆分（轻量 inline，两块同改）

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.siliconflow` + `protocols.siliconflow_en` 两块（仅此两块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/siliconflow-models.md`（双协议端点 + 60+ 模型清单 + 国际版复用结论）
2. 读 `prd.md`（endpoints 补 openai + model_list 20 主流 + models.default 3 档）
3. 读现有 `protocols.siliconflow` + `protocols.siliconflow_en` 块定位（research line 240-280 已贴现状：endpoints 仅 1 anthropic 端点，model_list/models.default 均空）
4. 改 `protocols.siliconflow`：
   - `endpoints.default`：补 openai 端点 `https://api.siliconflow.cn/v1`（codex_tui），保留现有 anthropic 端点 `https://api.siliconflow.cn`（claude_code）
   - `model_list.default`：填 20 主流对话模型（见 prd §2 精确清单）
   - `models.default`：3 档 `{"default": "Qwen/Qwen2.5-72B-Instruct", "coder": "Qwen/Qwen3-Coder-30B-A3B-Instruct", "fast": "deepseek-ai/DeepSeek-V4-Flash"}`
5. 改 `protocols.siliconflow_en`：完全复用 siliconflow 结论，仅 endpoints base_url 域名 .cn→.com（model_list + models.default 完全相同）
6. 验证：`python3 -c "import json;json.load(open('src-tauri/defaults/platform-presets.json'))"` 通过
7. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['siliconflow'];q=d['protocols']['siliconflow_en'];print('cn',len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']));print('en',len(q['model_list']['default']),q['models']['default'],len(q['endpoints']['default']))"`

## 验收（对齐 prd）

- [ ] siliconflow + siliconflow_en 各 endpoints 2 端点（anthropic host + openai /v1）
- [ ] model_list.default 各 20 模型（两协议同清单）
- [ ] models.default 各 3 档（default / coder / fast）
- [ ] siliconflow_en 域名全部 .com（非 .cn）
- [ ] desc / source_urls 保留不动
- [ ] JSON 合法
- [ ] 仅改 siliconflow + siliconflow_en 两块

## 失败处理

- JSON 解析失败 → 检查逗号/引号（20 模型数组易漏逗号），修复重验
- model_id 大小写错 → 严格按 research 清单（`Qwen/Qwen2.5-72B-Instruct` 非 `qwen/qwen2.5-72b-instruct`），厂商前缀大小写敏感
- 两块不同步 → siliconflow_en 必须 100% 复用 siliconflow 的 model_list + models.default，仅域名异

## 禁

- 禁动其他协议块
- 禁动 desc / source_urls（保留）
- 禁动 STATIC_MODEL_IDS
- 禁用 model-id 空 obj（必须档位名 key → string）
- 禁塞 embedding/reranker/TTS/图像生成/视频模型（仅 chat 对话）
- 禁臆造完整 60+ 清单（仅列 research 验证的 20 主流）
- 禁 git commit（finish hook 处理）
