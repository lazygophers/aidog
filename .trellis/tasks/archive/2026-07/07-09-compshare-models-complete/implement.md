# Implement: Compshare 两块补全（ModelVerse 全量 model_list + Coding Plan models.default）

## 载体
- 单 subtask（单文件改两块：`protocols.compshare` + `protocols.compshare_coding`）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.compshare` 块（ModelVerse）+ `protocols.compshare_coding` 块（Coding Plan）
- 禁动其他协议块、顶层 `version` / `last_updated`、STATIC_MODEL_IDS

## 步骤
1. 读 `research/compshare-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.compshare` + `protocols.compshare_coding` 块定位
4. **compshare（ModelVerse）**：补 model_list.default（文本对话全量，剔除非对话）/ models.default（claude-sonnet-5 / claude-opus-4-8 / deepseek-v4-pro）/ desc 改写（endpoints + source_urls 保留）
5. **compshare_coding（Coding Plan）**：补 models.default（claude-sonnet-4-6 / claude-opus-4-8 / claude-haiku-4-5）（endpoints + model_list + desc + source_urls 全保留）
6. 验证 JSON 合法
7. 验证：
   ```bash
   python3 -c "
   import json
   d=json.load(open('src-tauri/defaults/platform-presets.json'))
   for k in ['compshare','compshare_coding']:
       p=d['protocols'][k]
       print(k, 'model_list=',len(p['model_list']['default']),'models.default=',p['models']['default'],'endpoints=',len(p['endpoints']['default']))
   "
   ```
   预期输出：
   ```
   compshare model_list= 110+ models.default= {'sonnet': 'claude-sonnet-5', 'opus': 'claude-opus-4-8', 'default': 'deepseek-v4-pro'} endpoints= 3
   compshare_coding model_list= 7 models.default= {'sonnet': 'claude-sonnet-4-6', 'opus': 'claude-opus-4-8', 'haiku': 'claude-haiku-4-5'} endpoints= 1
   ```

## 验收（对齐 prd）

### compshare（ModelVerse）
- endpoints.default = 3 端点（保留）
- model_list.default 含全量文本对话模型（剔除图像/视频/音频/TTS/嵌入/重排），id 格式保留混合原貌
- models.default = 3 档位名 key（sonnet / opus / default），value 为 model id string
- desc = 8 语言改写
- source_urls 保留

### compshare_coding（Coding Plan）
- endpoints.default = 1 端点（保留）
- model_list.default = 原 7 个 aidog alias 保留不变（数据局限）
- models.default = 3 档位名 key（sonnet / opus / haiku），value 为 model id string
- desc 保留
- source_urls 保留
- JSON 合法

## 失败处理
- JSON 解析失败 → 检查逗号 / 引号 / 末尾多余逗号（model_list 数组长，重点查）
- python 校验抛 KeyError → 块名拼写或路径错
- model_list 长度异常 → 复核是否误剔除非对话模型或漏家族

## 禁
- 禁动其他协议块
- 禁用 model-id 空 obj 作 models.default value（value 必须 string，对齐 `Partial<Record<ModelSlot, string>>`）
- 禁加 id 日期后缀（保留平台原貌，如 claude-haiku-4-5-20251001 是 ModelVerse API 真实返回的 id，保留；compshare_coding 中 claude-haiku-4-5 是 aidog alias，也保留）
- 禁动 STATIC_MODEL_IDS
- 禁臆造 compshare_coding 的模型 id（数据局限）
- 禁 git commit
