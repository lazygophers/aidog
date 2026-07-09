# Implement: dmxapi 删历史版本 + 补 models.default 6 档

## 载体

- 单 subtask（单文件 protocols.dmxapi 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.dmxapi` 块
- 禁动其他协议块、顶层 version/last_updated
- 禁 git commit

## 步骤

1. 读 `research/dmxapi-models.md`（确认 11 项核实 line 77-89 + caveats line 162-167 数据局限）
2. 读 `prd.md`（确认改动 delta + 保守策略理由）
3. 读现有 `protocols.dmxapi` 块定位（`grep -n '"dmxapi"' src-tauri/defaults/platform-presets.json`）
4. 改动：
   - `model_list.default`：删 `"claude-opus-4-5-20251101"`（11→10 项）
   - `models.default`：从 `{}` 改为 6 档
     ```json
     {"default":"claude-sonnet-4-6","opus":"claude-opus-4-8","sonnet":"claude-sonnet-4-6","gpt":"gpt-5.5","coder":"kimi-k2.7-code","fast":"gemini-3.5-flash"}
     ```
   - endpoints / desc / source_urls / name 不动（gemini 端点保守不加）
5. 验证 JSON 合法
6. 验证计数：
   ```bash
   python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['dmxapi'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"
   ```
   预期输出：`10 {'default': 'claude-sonnet-4-6', 'opus': 'claude-opus-4-8', 'sonnet': 'claude-sonnet-4-6', 'gpt': 'gpt-5.5', 'coder': 'kimi-k2.7-code', 'fast': 'gemini-3.5-flash'} 2`

## 验收（对齐 prd）

- [ ] model_list.default 10 项（无 claude-opus-4-5-20251101）
- [ ] models.default 6 档 value 全 string
- [ ] endpoints.default 2 端点不动（保守不加 gemini）
- [ ] JSON 合法

## 失败处理

- JSON 解析失败 → 检查删项后逗号/引号
- grep 找不到 dmxapi 块 → 用 python 定位 key 行号
- 若 implement 阶段可实测 gemini 端点可达（向 `https://www.dmxapi.cn` 发 gemini 协议测试请求返回非 404），可追加 gemini 端点（base_url=`https://www.dmxapi.cn` client_type=default）；否则保守不加

## 禁

- 禁动其他协议块
- 禁用 model-id 空 obj（value 必须 string）
- 禁用 research line 128-138 推荐的 `gemini`/`deepseek`/`glm`/`kimi` 等非合法 ModelSlot key
- 禁扩 model_list（research 数据局限，保守不补推测模型）
- 禁 git commit
