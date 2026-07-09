# Implement: doubao(+byteplus) model_list 补全 + ID 格式修正 + source_urls 修正

## 载体

- 单 subtask（单文件 protocols.doubao + protocols.byteplus 两块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.doubao` 块 + `protocols.byteplus` 块
- 禁动其他无关协议块、顶层 version/last_updated

## 步骤

1. 读 `research/doubao-models.md`（重点 line 35 国内清单 + line 166 国际清单 + line 331-405 preset 建议）
2. 读 `prd.md`
3. 读现有 `protocols.doubao` 块 + `protocols.byteplus` 块定位（grep `"doubao":` / `"byteplus":`）
4. **doubao 块改动**：
   - `model_list.default`：从 11 → 18 模型（新增 7 项：evolving/2-1-pro/2-1-turbo/2-0-mini/seed-character/1.8/1.6）
   - `models.default`：从 `{"default":"doubao-seed-2-0-code"}` → `{"default":"doubao-seed-2-0-code","fast":"doubao-seed-2-0-mini","thinking":"doubao-seed-evolving"}`
   - `source_urls.pricing`：`https://www.volcengine.com/docs/6879` → `https://www.volcengine.com/docs/82379/1544106`
   - endpoints/desc/name 不动
5. **byteplus 块改动**：
   - `model_list.default`：从 4 → 9 模型，全部 `doubao-seed-*` → `seed-*` 前缀，新增 5 项（seed-1-8/seed-1-6/glm-5-2/deepseek-v4-pro/deepseek-v4-flash）
   - `models.default`：从 `{"default":"doubao-seed-2-0-pro"}` → `{"default":"seed-2-0-pro","fast":"seed-2-0-mini"}`
   - `source_urls`：docs `www.volcengine.com/docs/82379` → `docs.byteplus.com/en/docs/ModelArk`；pricing `www.volcengine.com/docs/6879` → `docs.byteplus.com/en/docs/ModelArk/1544106`
   - endpoints/desc/name 不动
6. 验证 JSON 合法（python3 json.load）
7. 验证：
   ```bash
   python3 -c "
   import json
   d=json.load(open('src-tauri/defaults/platform-presets.json'))
   db=d['protocols']['doubao']; bp=d['protocols']['byteplus']
   print('doubao list:', len(db['model_list']['default']), db['models']['default'])
   print('byteplus list:', len(bp['model_list']['default']), bp['models']['default'])
   # 检查 byteplus 无 doubao-seed-* 残留
   assert not any('doubao-seed' in m for m in bp['model_list']['default']), 'byteplus 仍有 doubao-seed-* 残留!'
   print('byteplus no doubao-seed-* residual: OK')
   "
   ```

## 验收（对齐 prd）

- [ ] doubao model_list.default = 18 模型
- [ ] doubao models.default = {default:doubao-seed-2-0-code, fast:doubao-seed-2-0-mini, thinking:doubao-seed-evolving}
- [ ] doubao source_urls.pricing = https://www.volcengine.com/docs/82379/1544106
- [ ] byteplus model_list.default = 9 模型，全部 seed-* 前缀（无 doubao-seed-*）
- [ ] byteplus models.default.default = seed-2-0-pro（非 doubao-seed-2-0-pro）
- [ ] byteplus source_urls = docs.byteplus.com 国际版
- [ ] endpoints 两块不动
- [ ] desc 两块不动

## 失败处理

- JSON 解析失败 → 检查逗号/引号（JSON 尾逗号、转义字符）
- byteplus ID 格式不匹配 → 再读 research line 162-164 确认 `seed-*` 格式
- `cargo test` 失败 → 检查是否误改了其他协议块

## 禁

- 禁动其他无关协议块（特别是 minimax/kimi/glm/deepseek 等独立协议）
- 禁用 model-id 空 obj（`{}`）
- 禁动 STATIC_MODEL_IDS（passthrough.rs）
- 禁动 coding_plan 子分支（per CLAUDE.md 2026-07-08 决策）
- 禁 git commit
