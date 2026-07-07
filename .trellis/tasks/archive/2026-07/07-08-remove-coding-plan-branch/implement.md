# Implement — 删 coding_plan 分支

## ST1: preset JSON 删 cp 分支

文件: `src-tauri/defaults/platform-presets.json`
协议 (8): glm / kimi / minimax / minimax_en / bailian / qianfan / xiaomi_mimo / doubao

每协议删除键:
- `endpoints.coding_plan`（8 协议全删）
- `models.coding_plan`（除 doubao，7 协议删）
- `model_list.coding_plan`（除 doubao，7 协议删）

用 Python 脚本批量改（保证 JSON 合法 + 缩进一致）:

```python
import json
p='src-tauri/defaults/platform-presets.json'
d=json.load(open(p))['protocols']
for k in ['glm','kimi','minimax','minimax_en','bailian','qianfan','xiaomi_mimo','doubao']:
    d[k].get('endpoints',{}).pop('coding_plan',None)
    d[k].get('models',{}).pop('coding_plan',None)
    d[k].get('model_list',{}).pop('coding_plan',None)
# 顶层 protocols 回写 + 保持顶层其他字段
```

注意：preset 顶层结构 = `{ "protocols": {...}, ... }`，写回时保留外层。

## ST2: CLAUDE.md 更新

`### 平台默认配置 (platform-presets.json)` 段，coding_plan 子段改述：
- preset JSON **默认不带** `coding_plan` 分支（endpoints/models/model_list）
- `coding_plan` 机制保留在 Rust/TS 代码 + 用户级 `platform.extra` / endpoint flag（用户可手工启用）
- `pickBranch` / `endpoint.rs` 缺 cp 分支自动回落 default

## ST3: 验证

```bash
python3 -c "import json; json.load(open('src-tauri/defaults/platform-presets.json'))"
grep -c '"coding_plan":' src-tauri/defaults/platform-presets.json  # 期望 0
yarn build
cd src-tauri && cargo check
```

## 工作目录

worktree: `.worktrees/07-08-remove-coding-plan-branch`（task.py start 自动建）

## 验收标准

见 prd.md Acceptance Criteria

## 失败处理

JSON 写回失败 → 回滚（git checkout preset JSON），报告。
tsc/cargo 报错 → 报告错行，禁自行改 Rust/TS 代码（本任务范围外）。
