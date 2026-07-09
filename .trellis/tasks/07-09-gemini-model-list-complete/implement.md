# Implement: gemini model_list 补全 + models.default 补多档

## 载体

- 单 subtask（单文件 protocols.gemini 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.gemini` 块
- 禁动其他无关协议块、顶层 version/last_updated

## 步骤

1. 读 `research/gemini-models.md`（重点 line 21-33 模型表 + line 98-121 推荐清单）
2. 读 `prd.md`
3. 读现有 `protocols.gemini` 块定位（grep `"gemini":`）
4. 改动：
   - `model_list.default`：从 4 → 7 模型（新增 `gemini-3.1-flash-lite` + `gemini-3.1-pro-preview` + `gemini-3-flash-preview`），排序按 research（3.5-flash → 3.1-pro-preview → 3.1-flash-lite → 3-flash-preview → 2.5-pro → 2.5-flash → 2.5-flash-lite）
   - `models.default`：从 `{"default":"gemini-2.5-pro"}` → `{"default":"gemini-2.5-pro","fast":"gemini-2.5-flash","thinking":"gemini-3.5-flash"}`
   - endpoints/desc/source_urls/name 不动
5. 验证 JSON 合法（python3 json.load）
6. 验证：
   ```bash
   python3 -c "
   import json
   d=json.load(open('src-tauri/defaults/platform-presets.json'))
   g=d['protocols']['gemini']
   ml=g['model_list']['default']
   print('gemini list:', len(ml), ml)
   print('models.default:', g['models']['default'])
   # 检查无 shut down 模型
   banned = ['gemini-2.0-flash','gemini-2.0-flash-lite','gemini-3-pro-preview','gemini-3.1-flash-lite-preview']
   for b in banned:
       assert b not in ml, f'{b} (shut down) should not be in list!'
   print('no shut-down models: OK')
   "
   ```

## 验收（对齐 prd）

- [ ] gemini model_list.default = 7 模型（含 3 新增：3.1-flash-lite / 3.1-pro-preview / 3-flash-preview）
- [ ] gemini models.default = {default:gemini-2.5-pro, fast:gemini-2.5-flash, thinking:gemini-3.5-flash}
- [ ] gemini-3.5-flash 保留（非笔误）
- [ ] 无已 shut down 模型残留
- [ ] endpoints/desc/source_urls 不动

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- `cargo test` 失败 → 检查是否误改了其他协议块
- 对 Preview 后缀不确定 → 读 research caveat line 146-147，优先用 `-preview` 后缀

## 禁

- 禁动其他无关协议块（特别是其他协议中引用 `google/gemini-*` 的聚合商）
- 禁用 model-id 空 obj（`{}`）
- 禁动 STATIC_MODEL_IDS（passthrough.rs）
- 禁 git commit
