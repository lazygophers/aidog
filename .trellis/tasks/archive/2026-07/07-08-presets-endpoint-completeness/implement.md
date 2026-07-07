# Implement — 补全 preset 端点/模型/价格

## 工作目录

worktree: `.worktrees/07-08-presets-endpoint-completeness`（task.py start 后建）

## 文件

- `src-tauri/defaults/platform-presets.json`（endpoints + models + model_list）
- `src-tauri/defaults/models.json`（价格，generated_at 更新）

## 执行阶段（research-heavy，分批并行）

### Phase 1: research（4 批，并发 2，2 轮）

派 `trellis-research` subagent，每批输出 `research/batch-<N>.md`：

| 批 | 协议 | 重点 |
|---|---|---|
| 1 原厂 | stepfun/siliconflow/modelscope/qianfan/bailing/sensenova/bailian_coding/glm/glm_en/kimi/minimax/minimax_en/doubao/byteplus/longcat/xiaomi_mimo/deepseek | 补 openai/openai_responses 端点 + 模型 + 价格 |
| 2 聚合 | openrouter/aihubmix/dmxapi/novita/atlascloud/shengsuanyun/therouter/rightcode/packycode/cubence/aigocode/aicodemirror/nvidia/newapi | 同 |
| 3 中转 A-L | cherryin/pateway/ccsub/apikeyfun/apinebula/sudocode/claudeapi/claudecn/runapi/relaxycode/crazyrouter/sssaicode/compshare/compshare_coding | 尽力，查不到标 `需要: 用户` |
| 4 中转 M-Z | micu/ctok/eflowcode/lemondata/pipellm/opencode/opencode_zen 等 | 同 |

每协议研究字段:
- 支持协议 + base_url（含版本前缀）+ 来源 URL
- models.default.default + model_list.default（≥3 个当前可用模型）
- models.json 价格: input_cost_per_token / output_cost_per_token / cache_read / cache_write（可选）

### Phase 2: 汇总 + 改 JSON

main 汇总 4 批 research → 派 `trellis-implement` 改两 JSON：
- platform-presets.json: 补 endpoints / models / model_list
- models.json: 加新模型价格条目 + 更新 generated_at

### Phase 3: 验证

```bash
python3 -c "import json; json.load(open('src-tauri/defaults/platform-presets.json'))"
python3 -c "import json; json.load(open('src-tauri/defaults/models.json'))"
yarn build
cd src-tauri && cargo check
```

## 合理默认（执行时遵循）

1. 第三方中转价格查不到（加价转售不透明）→ 标 `需要: 用户`，不强凑
2. cache_read/cache_write 查不到 → 不加该字段（可选）
3. 模型 id 在 preset model_list 与 models.json key 必须一致
4. base_url 含版本前缀（/v1, /api/paas/v4），遵循 CLAUDE.md URL 构造规则
5. 无法核实的协议/字段列入 `需要: 用户` 清单交 main 转达

## 验收标准

见 prd.md Acceptance Criteria

## 失败处理

- research subagent 查不到 → 标 `需要: 用户`，不编造
- JSON 写回语法错 → git checkout 回滚
- build/check 报错 → 报告，禁自行改 Rust/TS
