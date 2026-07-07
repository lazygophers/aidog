# implement.md: doubao preset endpoint 结构修复

> 配合 PRD。1 文件结构重组 + 验证。

## 执行层

- 载体: trellis-implement 内联直做（机械 JSON 重组，无 Rust/TS 代码改）
- worktree: 默认（flow 强制；active 集满，待 peak-disable finish 释放槽后再 start）
- 并行: 与 codex-responses-api 同改 platform-presets.json → **冲突串行**（调度自算）
- 门禁: json.tool + 全 protocol 同 protocol 重复扫 + cargo build/test

## 改动清单

### 步骤 1 — doubao endpoints 结构重组（D1）

`src-tauri/defaults/platform-presets.json` `protocols.doubao.endpoints`:

改前（单 default 6 条混塞）:
```json
"endpoints": {
  "default": [
    {"protocol":"anthropic","base_url":"https://ark.cn-beijing.volces.com/api/coding","client_type":"claude_code"},
    {"protocol":"openai","base_url":"https://ark.cn-beijing.volces.com/api/coding/v3","client_type":"codex_tui"},
    {"protocol":"openai_responses","base_url":"https://ark.cn-beijing.volces.com/api/coding/v3","client_type":"codex_tui"},
    {"protocol":"anthropic","base_url":"https://ark.cn-beijing.volces.com/api/plan","client_type":"claude_code"},
    {"protocol":"openai","base_url":"https://ark.cn-beijing.volces.com/api/plan/v3","client_type":"codex_tui"},
    {"protocol":"openai_responses","base_url":"https://ark.cn-beijing.volces.com/api/plan/v3","client_type":"codex_tui"}
  ]
}
```

改后（default = plan 三元，coding_plan = coding 三元 + coding_plan:true）:
```json
"endpoints": {
  "default": [
    {"protocol":"anthropic","base_url":"https://ark.cn-beijing.volces.com/api/plan","client_type":"claude_code"},
    {"protocol":"openai","base_url":"https://ark.cn-beijing.volces.com/api/plan/v3","client_type":"codex_tui"},
    {"protocol":"openai_responses","base_url":"https://ark.cn-beijing.volces.com/api/plan/v3","client_type":"codex_tui"}
  ],
  "coding_plan": [
    {"protocol":"anthropic","base_url":"https://ark.cn-beijing.volces.com/api/coding","client_type":"claude_code","coding_plan":true},
    {"protocol":"openai","base_url":"https://ark.cn-beijing.volces.com/api/coding/v3","client_type":"codex_tui","coding_plan":true},
    {"protocol":"openai_responses","base_url":"https://ark.cn-beijing.volces.com/api/coding/v3","client_type":"codex_tui","coding_plan":true}
  ]
}
```

仅重组结构。base_url / protocol / client_type / models / model_list / name / desc / source_urls 全不动。

### 步骤 2 — 门禁（D2）

```bash
# JSON 有效
python3 -m json.tool src-tauri/defaults/platform-presets.json > /dev/null

# doubao 结构正确（default 3 条 plan + coding_plan 3 条 coding 带 coding_plan:true）
python3 -c "
import json
d = json.load(open('src-tauri/defaults/platform-presets.json'))
eps = d['protocols']['doubao']['endpoints']
default = eps['default']
cp = eps['coding_plan']
assert len(default) == 3, f'default count {len(default)}'
assert len(cp) == 3, f'coding_plan count {len(cp)}'
assert all(e['base_url'].endswith('/plan') or '/plan/v3' in e['base_url'] for e in default), 'default must be plan'
assert all('/coding' in e['base_url'] for e in cp), 'coding_plan must be coding'
assert all(e.get('coding_plan') is True for e in cp), 'coding_plan branches need coding_plan:true'
# 无同 protocol 重复（每分支内）
for ct, lst in [('default', default), ('coding_plan', cp)]:
    protos = [e['protocol'] for e in lst]
    assert len(protos) == len(set(protos)), f'{ct} has dup protocol'
print('doubao structure OK')
"

# 全 protocol 扫无同 protocol 重复（仅 doubao 命中已修，其余应全过）
python3 -c "
import json
d = json.load(open('src-tauri/defaults/platform-presets.json'))
bad = []
for pid, p in d['protocols'].items():
    for ct, eps in p.get('endpoints', {}).items():
        seen = {}
        for i, ep in enumerate(eps):
            proto = ep.get('protocol','')
            if proto in seen:
                bad.append(f'{pid}[{ct}] {proto} #{i}+#{seen[proto]}')
            else:
                seen[proto] = i
assert not bad, f'still dup: {bad}'
print('no same-protocol dup across all protocols')
"

# Rust 编译 + 测试不回归（preset 读路径无代码改，验证 defaults.rs 不炸）
cd src-tauri && cargo build 2>&1 | tail -3
cd src-tauri && cargo test --quiet 2>&1 | tail -5
```

全 exit 0 + doubao structure OK + no same-protocol dup。

### 步骤 3 — 类似问题扫描（D3，已在 planning 阶段完成）

全 protocol 扫「同 endpoints key 内同 protocol 出现 ≥2 次」= 仅 doubao 命中（本 task 修）。无其他需修。

## 自检

`✅ lint=clippy无new warn type=cargo build过 test=cargo test不回归 TODO=0 验收物=doubao default 3 plan + coding_plan 3 coding(coding_plan:true) + 全 protocol 无同 protocol 重复`

## 失败处理

- JSON 无效: 仅重组 endpoints 子结构，禁动其他字段；用 python json 模块序列化保缩进
- cargo test 回归: 查 test_schema.rs / test_defaults 是否硬编码 doubao endpoint 顺序（推测无，doubao 非测试 fixture）；定点修
- coding_plan 字段未被消费: 查 `proxy/endpoint.rs:53-56` 路由逻辑确认 preset→DB→proxy 全链路读 coding_plan（已就绪，无代码改）
