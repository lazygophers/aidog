# Implement: stepfun(+stepfun_en) 补 model_list 2 漏项 + models.default 补档

## 载体

- 单 subtask（同源镜像两协议块：`protocols.stepfun` + `protocols.stepfun_en`）
- trellis-implement 在 task worktree 内内联执行
- 无 subtask 拆分（轻量 inline，两块同改）

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.stepfun` + `protocols.stepfun_en` 两块（仅此两块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/stepfun-models.md`（文本主线 4 模型 + endpoints 双协议 + 已下线 9 模型迁移表 + 语音/图像排除项）
2. 读 `prd.md`（model_list 补 2 漏项 + models.default 补 default 档）
3. 读现有 `protocols.stepfun` + `protocols.stepfun_en` 块定位（current preset：endpoints 各 2 端点 ✅，model_list 各 2 模型 [step-3.7-flash, step-3.5-flash]，models.default 各 {"default": "step-3.7-flash"}）
4. **endpoints 不动**（2 端点已核验正确：openai /v1 + anthropic /step_plan）
5. 改 `model_list.default`（两协议同）：补 2 漏项 → `["step-3.7-flash", "step-3.5-flash", "step-3.5-flash-2603", "step-1o-turbo-vision"]`
6. 改 `models.default`（两协议同）：`{"default": "step-3.7-flash"}`（现有已正确，确认无需改）
7. 验证：`python3 -c "import json;json.load(open('src-tauri/defaults/platform-presets.json'))"` 通过
8. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['stepfun'];q=d['protocols']['stepfun_en'];print('cn',len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']));print('en',len(q['model_list']['default']),q['models']['default'],len(q['endpoints']['default']))"`

## 验收（对齐 prd）

- [ ] stepfun + stepfun_en 各 endpoints 2 端点保留（openai /v1 + anthropic /step_plan）
- [ ] model_list.default 各 4 模型（补 step-3.5-flash-2603 + step-1o-turbo-vision）
- [ ] models.default 各 {"default": "step-3.7-flash"}
- [ ] stepfun_en 域名全部 .ai（非 .com）
- [ ] desc / source_urls 保留不动
- [ ] JSON 合法
- [ ] 仅改 stepfun + stepfun_en 两块

## 失败处理

- JSON 解析失败 → 检查逗号/引号，修复重验
- 模型 id 拼写错 → 严格按 research 清单（`step-3.5-flash-2603` 含日期后缀，`step-1o-turbo-vision` 含 1o 非 10）
- 两块不同步 → stepfun_en 必须 100% 复用 stepfun 的 model_list + models.default，仅域名异

## 禁

- 禁动其他协议块
- 禁动 endpoints（已核验正确）
- 禁动 desc / source_urls（保留）
- 禁动 STATIC_MODEL_IDS
- 禁用 model-id 空 obj（必须档位名 key → string）
- 禁塞语音/图像/智能路由模型（非文本推理主线）
- 禁改 step_plan 路径（anthropic 端点专用，非 /v1）
- 禁 git commit（finish hook 处理）
