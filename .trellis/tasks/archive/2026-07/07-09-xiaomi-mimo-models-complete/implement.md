# Implement: xiaomi_mimo model_list 移除 3 已弃用 + source_urls 修正

## 载体

- 单 subtask（单文件 `protocols.xiaomi_mimo` 块，无 xiaomi_mimo_en 镜像）
- trellis-implement 在 task worktree 内内联执行
- 无 subtask 拆分（轻量 inline）

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.xiaomi_mimo` 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/xiaomi-mimo-models.md`（弃用公告 + 端点核实 + 在售 2 款 + ASR/TTS 排除项）
2. 读 `prd.md`（model_list 移 3 已弃用 + source_urls 修正 + models.default 确认）
3. 读现有 `protocols.xiaomi_mimo` 块定位（current preset：endpoints 2 ✅，model_list 5 含 3 已弃用，models.default {"default": "mimo-v2.5-pro"} ✅，source_urls 指向 mimo.xiaomi.com 主页）
4. **endpoints 不动**（2 端点已核验正确：anthropic /anthropic + openai /v1）
5. 改 `model_list.default`：移除 3 已弃用（mimo-v2-pro / mimo-v2-omni / mimo-v2-flash）→ `["mimo-v2.5-pro", "mimo-v2.5"]`
6. **models.default 不动**（`{"default": "mimo-v2.5-pro"}` 现有已正确）
7. 改 `source_urls`：mimo.xiaomi.com → platform.xiaomimimo.com/docs（docs）+ platform.xiaomimimo.com/docs/en-US/quick-start/summary/model（pricing 暂用模型列表页）
8. 验证：`python3 -c "import json;json.load(open('src-tauri/defaults/platform-presets.json'))"` 通过
9. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['xiaomi_mimo'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']),p['source_urls'])"`

## 验收（对齐 prd）

- [ ] endpoints 2 端点保留
- [ ] model_list.default 2 模型（移除 mimo-v2-pro / mimo-v2-omni / mimo-v2-flash）
- [ ] models.default = {"default": "mimo-v2.5-pro"}（保留）
- [ ] source_urls 修正为 platform.xiaomimimo.com/docs
- [ ] desc 保留
- [ ] JSON 合法
- [ ] 仅改 xiaomi_mimo 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号，修复重验
- 误删 mimo-v2.5（在售）→ 仅删 v2-pro/v2-omni/v2-flash 三款，v2.5-pro + v2.5 必须保留
- source_urls 路径不确定 → 按 research line 9-13 官方文档源，docs=platform.xiaomimimo.com/docs

## 禁

- 禁动其他协议块
- 禁动 endpoints（已核验正确）
- 禁动 desc
- 禁动 STATIC_MODEL_IDS
- 禁用 model-id 空 obj（必须档位名 key → string）
- 禁删 mimo-v2.5 / mimo-v2.5-pro（在售，仅删 v2-pro/v2-omni/v2-flash）
- 禁塞 ASR/TTS/voiceclone/voicedesign（非对话）+ mimo-v2.5-pro-ultraspeed（内测）
- 禁臆造国际版域名（research 标无）
- 禁 git commit（finish hook 处理）
