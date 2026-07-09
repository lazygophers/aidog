# Implement: glm(+glm_en) model_list 补全 + models.default 补 fast 档

## 载体

- 单 subtask（单文件 protocols.glm + protocols.glm_en 两块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.glm` 块 + `protocols.glm_en` 块
- 禁动其他无关协议块、顶层 version/last_updated

## 步骤

1. 读 `research/glm-models-endpoints.md`（重点 line 112-123 遗漏分析 + line 155-181 推荐清单）
2. 读 `prd.md`
3. 读现有 `protocols.glm` 块 + `protocols.glm_en` 块定位（grep `"glm":` / `"glm_en":`）
4. **glm 块改动**：
   - `model_list.default`：从 8 → 10 模型（新增 `glm-4.7-flashx` 插在 `glm-4.7` 后 + `glm-4.5-airx` 插在 `glm-4.5-air` 后）
   - `models.default`：从 `{"default":"glm-5.2"}` → `{"default":"glm-5.2","fast":"glm-4.7-flashx"}`
   - endpoints/desc/source_urls/name 不动
5. **glm_en 块改动**：
   - 与 glm 完全相同的 model_list.default 改动（8 → 10，新增同样 2 项）
   - 与 glm 完全相同的 models.default 改动
   - endpoints/desc/source_urls/name 不动
6. 验证 JSON 合法（python3 json.load）
7. 验证：
   ```bash
   python3 -c "
   import json
   d=json.load(open('src-tauri/defaults/platform-presets.json'))
   glm=d['protocols']['glm']; glm_en=d['protocols']['glm_en']
   print('glm list:', len(glm['model_list']['default']), glm['models']['default'])
   print('glm_en list:', len(glm_en['model_list']['default']), glm_en['models']['default'])
   # 检查两协议 list 一致
   assert glm['model_list']['default'] == glm_en['model_list']['default'], 'glm != glm_en!'
   # 检查不含即将下线模型
   for m in ['glm-4.5','glm-4.5-x','glm-4.5-flash']:
       assert m not in glm['model_list']['default'], f'{m} (即将下线) should not be in list!'
   print('glm == glm_en: OK')
   print('no deprecated models: OK')
   "
   ```

## 验收（对齐 prd）

- [ ] glm model_list.default = 10 模型（含新增 glm-4.7-flashx + glm-4.5-airx）
- [ ] glm_en model_list.default = 10 模型，与 glm 完全一致
- [ ] glm + glm_en models.default = {default:glm-5.2, fast:glm-4.7-flashx}
- [ ] 不含即将下线模型（glm-4.5 / glm-4.5-x / glm-4.5-flash）
- [ ] endpoints/desc/source_urls 两块不动

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- 两协议 list 不一致 → 确认 glm_en 改动与 glm 完全相同
- `cargo test` 失败 → 检查是否误改了其他协议块

## 禁

- 禁动其他无关协议块（特别是 doubao/byteplus 中聚合的 `glm-5.2` / `glm-5-2`）
- 禁用 model-id 空 obj（`{}`）
- 禁动 STATIC_MODEL_IDS（passthrough.rs）
- 禁动 coding_plan 子分支 / endpoints（per CLAUDE.md 2026-07-08 决策）
- 禁动 `glm-4.7-flash`（免费普惠版，保留，勿与 `glm-4.7-flashx` 高速版混淆）
- 禁 git commit
