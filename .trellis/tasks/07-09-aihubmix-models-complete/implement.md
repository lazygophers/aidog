# Implement: aihubmix 补 gemini 端点 + models.default 6 档

## 载体

- 单 subtask（单文件 protocols.aihubmix 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.aihubmix` 块
- 禁动其他协议块、顶层 version/last_updated
- 禁 git commit

## 步骤

1. 读 `research/aihubmix-models.md`（确认 4 协议 line 56 + gemini_api 支持 line 67 + 14 模型有效 line 87）
2. 读 `prd.md`（确认改动 delta）
3. 读现有 `protocols.aihubmix` 块定位（`grep -n '"aihubmix"' src-tauri/defaults/platform-presets.json`）
4. 改动：
   - `endpoints.default`：追加 gemini 端点（2→3）
     ```json
     {"protocol":"gemini","base_url":"https://aihubmix.com","client_type":"default"}
     ```
   - `models.default`：从 `{}` 改为 6 档
     ```json
     {"default":"claude-sonnet-4-6","opus":"claude-opus-4-8","sonnet":"claude-sonnet-4-6","gpt":"gpt-5.5","coder":"kimi-k2.7-code","fast":"gemini-3.5-flash"}
     ```
   - `model_list.default` / desc / source_urls / name 不动
5. 验证 JSON 合法
6. 验证计数：
   ```bash
   python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['aihubmix'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"
   ```
   预期输出：`14 {'default': 'claude-sonnet-4-6', 'opus': 'claude-opus-4-8', 'sonnet': 'claude-sonnet-4-6', 'gpt': 'gpt-5.5', 'coder': 'kimi-k2.7-code', 'fast': 'gemini-3.5-flash'} 3`

## 验收（对齐 prd）

- [ ] model_list.default 14 项不变
- [ ] models.default 6 档 value 全 string
- [ ] endpoints.default 3 端点含 gemini（base_url=`https://aihubmix.com`）
- [ ] JSON 合法

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- gemini 端点 base_url 纠正：若 implement 阶段实测 `https://aihubmix.com` 不可达，试 `https://aihubmix.com/google`（research line 81 推测）

## 禁

- 禁动其他协议块
- 禁用 model-id 空 obj（value 必须 string）
- 禁动 model_list.default（14 项保留）
- 禁 git commit
