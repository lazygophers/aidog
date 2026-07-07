# implement.md: platform preset 数据补全

> 配合 PRD。ST1 机械补 client_type，ST2 research 补 model。

## 执行层

- 载体: trellis-implement（ST1 机械内联 + ST2 research 派 trellis-research 后填）
- worktree: 默认（flow 强制；等 doubao/codex finish 释放 active 槽后 start）
- 并行: 与 doubao/codex 同文件 platform-presets.json → 物理不同协议段 git 3-way 可合，但保守串行（doubao/codex 先,本 task 后）
- 门禁: json.tool + 全 protocol 扫 + cargo build/test

## 改动清单

### ST1 — ③ client_type 机械补（无外部数据）

`src-tauri/defaults/platform-presets.json` 6 协议 endpoint（全 gemini protocol，缺 client_type）补 `"client_type": "default"`:

- gemini[default][0]（base https://generativelanguage.googleapis.com）
- openrouter[default][2]（base https://openrouter.ai/api）
- packycode[default][2]（base https://www.packyapi.com）
- cubence[default][2]（base https://api.cubence.com）
- aigocode[default][2]（base https://api.aigocode.com）
- aicodemirror[default][2]（base https://api.aicodemirror.com/api/gemini）

值 `"default"`（platform.rs:121 `_ => ClientType::Default`，fallback 一致；显式补为数据完整性）。

### ST2 — ② default model + ① model_list research 补

派 trellis-research 并行查各平台官网/文档:

**② default model 空（models.default={}）补旗舰**:
- gemini: 查 Google AI Studio 旗舰（如 gemini-2.5-pro）
- siliconflow: 聚合平台，models.default 是否补（按聚合留空 or 补默认如 Qwen/Qwen2.5-72B）

**① 非聚合 model_list 空（补 3-5 旗舰）**:
- gemini / bailian / bailian_coding / bailing / qianfan / longcat / compshare / opencode

**聚合平台留空**（siliconflow / siliconflow_en / newapi / therouter）: 不动。

research 汇总后填 platform-presets.json 对应条目。

### 门禁

```bash
# JSON 有效
python3 -m json.tool src-tauri/defaults/platform-presets.json > /dev/null

# 无 endpoint 缺 client_type（ST1 后）
python3 -c "
import json
d = json.load(open('src-tauri/defaults/platform-presets.json'))
bad = []
for pid, p in d['protocols'].items():
    for ct, eps in p.get('endpoints', {}).items():
        for i, ep in enumerate(eps):
            if 'client_type' not in ep:
                bad.append(f'{pid}[{ct}][{i}]')
assert not bad, f'still missing client_type: {bad}'
print('all endpoints have client_type')
"

# 非聚合平台 model_list 非空（ST2 后）
python3 -c "
import json
d = json.load(open('src-tauri/defaults/platform-presets.json'))
non_agg = ['gemini','bailian','bailian_coding','bailing','qianfan','longcat','compshare','opencode']
for pid in non_agg:
    ml = d['protocols'][pid].get('model_list', {}).get('default', [])
    assert len(ml) >= 3, f'{pid} model_list too short: {len(ml)}'
print('non-aggregate model_list all filled')
"

# cargo 不回归
cd src-tauri && cargo build 2>&1 | tail -3
cd src-tauri && cargo test --quiet 2>&1 | tail -5
```

全 exit 0。

## 自检

`✅ lint=clippy无new warn type=cargo build过 test=cargo test不回归 TODO=0 验收物=6 endpoint client_type 补全 + 非聚合 model_list/default model 补旗舰 + 聚合留空 + 全 protocol 无主修类缺失`

## 失败处理

- research 查不到某平台旗舰: 标 `需要: <平台> 旗舰 model`，跳过该平台（不强填臆测）
- cargo test 回归: 查 test_defaults / test_schema 是否硬编码 model_list 长度；定点修
- JSON 无效: 仅改目标字段，保缩进
