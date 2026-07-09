# Implement: kimi model_list 整组替换 + endpoints 修正 + models.default 更新

## 载体

- 单 subtask（单文件 protocols.kimi 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.kimi` 块
- 禁动其他无关协议块、顶层 version/last_updated

## 步骤

1. 读 `research/kimi-models.md`（重点 line 66-81 推荐清单 + line 95-141 endpoints + line 180-195 caveats）
2. 读 `prd.md`
3. 读现有 `protocols.kimi` 块定位（grep `"kimi":`）
4. 改动：
   - `model_list.default`：整组替换，从 4 → 10 模型（移除 `kimi-k2-thinking` + `kimi-latest`；新增 `kimi-k2.7-code` + `kimi-k2.7-code-highspeed` + `moonshot-v1-8k` / `moonshot-v1-32k` / `moonshot-v1-128k` / `moonshot-v1-8k-vision-preview` / `moonshot-v1-32k-vision-preview` / `moonshot-v1-128k-vision-preview`；保留 `kimi-k2.6` + `kimi-k2.5`）
   - `models.default`：从 `{"default":"kimi-k2.6"}` → `{"default":"kimi-k2.7-code","fast":"kimi-k2.7-code-highspeed"}`
   - `endpoints.default[1]`（anthropic 端点）：`base_url` 从 `https://api.moonshot.cn/anthropic` → `https://platform.moonshot.cn/anthropic`（per research line 113/141-142 Claude Code 集成指南官方域名）
   - `source_urls.pricing`：从 `https://platform.moonshot.cn/docs/pricing` → `https://platform.moonshot.cn/docs/pricing/chat`（research line 17 更精确路径）
   - desc/name 不动
5. 验证 JSON 合法（python3 json.load）
6. 验证：
   ```bash
   python3 -c "
   import json
   d=json.load(open('src-tauri/defaults/platform-presets.json'))
   k=d['protocols']['kimi']
   ml=k['model_list']['default']
   print('kimi list:', len(ml), ml)
   print('models.default:', k['models']['default'])
   print('endpoints:', [(e['protocol'], e['base_url']) for e in k['endpoints']['default']])
   # 检查无下线模型
   for m in ['kimi-k2-thinking', 'kimi-latest']:
       assert m not in ml, f'{m} (已下线) should not be in list!'
   print('no deprecated models: OK')
   "
   ```

## 验收（对齐 prd）

- [ ] kimi model_list.default = 10 模型（K2.7-code 系列 + K2.6/K2.5 + Moonshot V1 系列）
- [ ] 不含已下线模型（kimi-k2-thinking / kimi-latest）
- [ ] kimi models.default = {default:kimi-k2.7-code, fast:kimi-k2.7-code-highspeed}
- [ ] Anthropic 端点 base_url = https://platform.moonshot.cn/anthropic
- [ ] OpenAI 端点 base_url 不动（https://api.moonshot.cn/v1）
- [ ] desc/source_urls.docs 不动

## 失败处理

- JSON 解析失败 → 检查逗号/引号（特别是 model_list 数组逗号）
- Anthropic 端点域名不确定 → 读 research line 113 + line 117-122 + line 141-142，两个域名（`api.moonshot.cn` vs `platform.moonshot.cn`）在 research 中均有出现，默认用 Claude Code 集成指南的 `platform.moonshot.cn`；如 implement 能验证 `api.moonshot.cn/anthropic` 仍可用则可保留旧值
- `cargo test` 失败 → 检查是否误改了其他协议块

## 禁

- 禁动其他无关协议块（特别是 doubao/byteplus 中聚合的 kimi-k2.6 / kimi-k2.7-code）
- 禁用 model-id 空 obj（`{}`）
- 禁动 STATIC_MODEL_IDS（passthrough.rs）
- 禁 git commit
