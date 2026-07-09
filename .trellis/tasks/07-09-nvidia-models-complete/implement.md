# Implement: nvidia model_list+endpoints+desc 补全

## 载体

- 单 subtask（单文件 `protocols.nvidia` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.nvidia` 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/nvidia-models.md`（121 模型金标准 + 12 现有核对）
2. 读 `prd.md`（筛选规则 + 2 id 修正 + 三档）
3. 读现有 `protocols.nvidia` 块定位
4. endpoints 不动（1 openai 端点正确）
5. 改 `model_list.default`：
   - **删** `deepseek/deepseek-v3.2`（前缀+版本双错）
   - **改** `z-ai/glm-5.1` → `z-ai/glm-5.2`
   - 按 prd 清单补全主力（约 85-95），排除纯 embedding/rerank/safety/reward/translate/PII/科学计算/视频检测
6. 改 `models.default`：三档（nvidia/llama-3.3-nemotron-super-49b-v1.5 / nvidia/nemotron-3-ultra-550b-a55b / deepseek-ai/deepseek-v4-pro）
7. 改 `desc` 8 语言
8. 验证 JSON 合法
9. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));n=d['protocols']['nvidia'];print(len(n['model_list']['default']),list(n['models']['default'].keys()),'deepseek/deepseek-v3.2' not in n['model_list']['default'],'z-ai/glm-5.2' in n['model_list']['default'])"`

## 验收（对齐 prd）

- [ ] endpoints 1 保留
- [ ] deepseek/deepseek-v3.2 已删
- [ ] z-ai/glm-5.1 → z-ai/glm-5.2
- [ ] model_list 约 85-95
- [ ] models.default 三档
- [ ] desc 8 语言
- [ ] JSON 合法
- [ ] 仅改 nvidia 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- id 拼写卡壳 → 对照 research「全量模型清单」逐字复制
- desc 翻译卡住 → 参考现有平台对应语言风格

## 禁

- 禁动其他协议块
- 禁动 endpoints
- 禁保留 deepseek/deepseek-v3.2（错误 id）
- 禁保留 z-ai/glm-5.1（错误 id）
- 禁收录纯 embedding/rerank/safety/reward 模型
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
