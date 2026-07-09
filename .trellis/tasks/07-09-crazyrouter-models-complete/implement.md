# Implement: crazyrouter 补全 165 模型 + 三档 models.default

## 载体
- 单 subtask（单文件 protocols.crazyrouter 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.crazyrouter` 块
- 禁动其他协议块、顶层 version/last_updated

## 步骤
1. 读 `research/crazyrouter-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.crazyrouter` 块定位
4. 改 `model_list.default` → 165 模型（按 prd 家族顺序，裸 id，去重）
5. 改 `models.default` → 三档 `{"default":"claude-opus-4-8","gpt":"gpt-5.5","fast":"deepseek-v4-flash"}`（档位名 key → model id string）
6. endpoints / desc / source_urls 保留不动
7. 验证 JSON 合法
8. 验证命令：
```bash
python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['crazyrouter'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"
```
期望输出：`165 {'default': 'claude-opus-4-8', 'gpt': 'gpt-5.5', 'fast': 'deepseek-v4-flash'} 3`

## 验收（对齐 prd）
- model_list.default = 165（去重后家族求和 = 165）
- models.default 三档 档位名 key → string（default/gpt/fast）
- endpoints 3 端点不变
- desc / source_urls 保留
- JSON 合法

## 失败处理
- JSON 解析失败 → 检查逗号/引号
- model_list 计数 < 165 → 检查 grok-4-0709 是否被重复删除（应保留一份）、跨家族 gpt-image-2 / qwen-image-* 是否重复（应在原家族内只出现一次）

## 禁
- 禁动其他协议块
- 禁用 model-id 空 obj 作 models.default value（必须档位名 key → string）
- 禁加 id 日期后缀
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
