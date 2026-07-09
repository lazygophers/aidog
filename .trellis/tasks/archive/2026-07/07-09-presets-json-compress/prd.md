# platform-presets.json 激进压缩

## Goal

压缩 `src-tauri/defaults/platform-presets.json`（5088 行 → 目标 ~2800 行，-45%），把叶子层结构（model_list 数组 / endpoints 对象 / source_urls / name+desc locale map）折成单行。约束 = **git diff 局部化**（不全局改缩进，仅折叶子），**JSON 语义 + 字段顺序不变**，**Rust `include_str!` 解析 + 前端 `JSON.parse` 兼容**（标准 JSON，无注释/尾逗号）。

**为什么**：用户要「合理压缩，git 变更尽可能影响少的行 + 减少行数」。全文件缩进改（2→1）会 touch 全部 5088 行 = diff 噪音爆炸。折叶子层是性价比最高路径：每处 N 行折 1 行，diff 仅局部 +/-，结构层（protocols → protocol → section）保持可读易编辑。

## 现状

- 文件：`src-tauri/defaults/platform-presets.json`，5088 行，2-space indent
- 结构（per protocol）：
  ```
  "anthropic": {
    "client_type": "...",                    // 单行（已是）
    "endpoints": { "default": [ {多行 obj} ] },  // 端点对象多行
    "models": { "default": {多行 slot map} },     // 槽位 map 单行/多行混
    "model_list": { "default": [多行 str 数组] },  // ←最大头
    "name": { 8 locale 单行 },                   // ←中头
    "desc": { 8 locale 单行 },                   // ←中头
    "source_urls": {多行}, "homepage": "...", "logo_url": "..."
  }
  ```
- 真值源：手维护（CLAUDE.md「禁机器生成覆盖」）—— 本 task 是**一次性格式化重构**，非后续自动生成；改后仍手维护
- 跨层解析：Rust `defaults.rs::get_defaults_json` include_str! + serde_json；前端 `defaults.ts` getDefaultsJson → JSON.parse。两者都标准 JSON 解析，折叠空 白不影响

## 压缩目标（激进档，用户确认）

| 结构 | 现格式 | 压后 | 节省/协议 ×61 |
| --- | --- | --- | --- |
| `model_list.default[]` | 多行字符串数组 | **单行数组** | ~670 |
| `endpoints.default[]` 每对象 | 5 行对象 | **单行对象** | ~360 |
| `source_urls` | 3 行 | **单行对象** | ~120 |
| `name` locale map | 10 行（8 locale） | **单行对象** | ~550 |
| `desc` locale map | 10 行（8 locale） | **单行对象** | ~550 |
| **合计** | | | **~2250 行** |

目标：5088 → ~2800 行（-45%）。

**保持多行**（结构层 + 易编辑）：
- 顶层（version / last_updated / protocols）
- protocol 对象本身（`"anthropic": { ... }` 多行）
- section 键（endpoints / models / model_list / name / desc 作为 protocol 字段，section 开闭行保留）
- `models.default` slot map（槽位 map，5 key 单行已紧凑，或保多行——**实施时按现状**，倾向单行）
- `peak_hours` 窗口对象数组（字段多，保多行易编辑）
- `homepage` / `logo_url` / `client_type` / `is_coding_plan` 单行字段（已是）

## Requirements

### R1 折叠规则（确定性 + 可复现）

- R1.1 **model_list 数组**：`"model_list": { "default": [...10个string...] }` 整对象压单行（含 `model_list` 键 + default 键 + 数组）。或保留 `"model_list": {` + `"default": [单行]` + `}` —— **实施时选**，倾向**整个 model_list 值单行**（最大压缩）。
- R1.2 **endpoints 数组对象**：每个 endpoint 对象 `{protocol, base_url, client_type, coding_plan?}` 折单行；数组本身 `[{...}, {...}]` 多元素时各元素单行。
- R1.3 **source_urls 对象**：`"source_urls": {"docs": "...", "pricing": "..."}` 整值单行。
- R1.4 **name/desc locale map**：8 locale 键值对折单行（整对象值一行）。
- R1.5 **models slot map**：`models.default` / `models.coding_plan` 的 `{slot: model_id}` 折单行（对象值一行）。
- R1.6 **peak_hours 窗口**：**不折**（字段多，配置需编辑，保多行）。
- R1.7 **字段顺序不变**：折叠仅改空白/换行，不动 key 顺序。

### R2 实施方式（脚本，可复现）

- R2.1 写一次性 python 脚本 `scripts/presets_compress.py`（或 inline 在 task 内跑一次后删）：load JSON → 自定义 serializer → 折指定叶子 → dump。
  - 备选：`jq` + 过滤器（但 jq 折叠指定路径不灵活，倾向 python）
  - 备选：手 sed（脆弱，不推荐）
- R2.2 脚本逻辑：递归遍历 JSON tree，遇 `model_list` / `source_urls` / `name` / `desc` / `models` / endpoint 对象 → 标记 single-line；其他保 multi-line indent=2。
- R2.3 跑脚本生成新 platform-presets.json，**对比 diff 确认仅折叠无语义变**。
- R2.4 脚本跑完可保留（便于后续 preset 增改后重压）或删（一次性）。**倾向保留 + commit**（文档化折叠规则）。

### R3 验证（必跑）

- R3.1 **JSON 语义等价**：`python3 -c "import json; a=json.load(open(原)); b=json.load(open(新)); assert a==b"`（折叠前后 load 相等 = 零语义变）。
- R3.2 **Rust 编译**：`cargo build --lib`（include_str! + serde_json 解析新文件）过。
- R3.3 **Rust 测试**：`cargo test --lib` 全绿（含 defaults 解析相关测试）。
- R3.4 **前端**：`yarn build` 过（前端 getDefaultsJson → JSON.parse 新格式）。
- R3.5 **行数核**：新文件 `wc -l` ≤ 3000（目标 ~2800）。
- R3.6 **diff 抽查**：`git diff` 抽 2-3 协议确认仅折叠，无 key 丢失 / 顺序变。

### R4 commit

- R4.1 单 commit `style(presets): 折叠 platform-presets.json 叶子层（5088→~2800 行）`。
- R4.2 commit message 记折叠规则（model_list/endpoints/source_urls/name/desc/models 单行；peak_hours/结构层多行）。

## Acceptance Criteria

- [ ] 5088 → ~2800 行（-45%）
- [ ] JSON load 等价（折叠前后 `json.load` 相等，零语义变）
- [ ] cargo build + cargo test --lib 全绿
- [ ] yarn build 过
- [ ] diff 仅折叠（抽查无 key 丢失 / 顺序变）
- [ ] 压缩脚本保留 + commit（折叠规则文档化）
- [ ] 主仓零改动（worktree 内）

## Definition of Done

- platform-presets.json 折叠到目标行数
- JSON 语义零变（load 等价 + 跨层解析绿）
- 压缩脚本入库可复现
- journal 记折叠规则 + 行数对比

## Technical Approach

```python
# scripts/presets_compress.py
import json, sys
SINGLE_LINE_KEYS = {"model_list", "source_urls", "name", "desc", "models"}
# endpoint 对象识别：在 protocols[k].endpoints.{default,coding_plan}[] 内的对象

def serialize(obj, indent=0, key=None):
    sp = "  " * indent
    if key in SINGLE_LINE_KEYS:
        return sp + json.dumps(key) + ": " + json.dumps(obj, ensure_ascii=False, separators=(", ", ": "))
    # 递归处理 dict/list，结构层多行
    ...
# load → serialize → write
```

实际实施：用 python json.load + 自定义 dumps（控制 indent + 单行 key 集合），或 json.dumps 全 multi-line 后 regex post-process 折指定 key。后者更稳（json.dumps 保语义，post-process 仅改空白）。

## Decision (ADR-lite)

**Context**：5088 行 JSON 手维护成本高，用户要压缩 + diff 局部化。
**Decision**：
1. 折叶子层（model_list/endpoints/source_urls/name/desc/models）非改全局缩进 —— diff 局部化。
2. peak_hours / 结构层保多行 —— 配置编辑性 + 可读性。
3. 脚本入库可复现 —— 后续 preset 增改后重压，规则文档化。
4. JSON load 等价作硬门禁 —— 零语义变可证。
**Consequences**：
- name/desc 单行后翻译编辑需手 edit 单行长字符串（IDE 可 wrap，可接受）。
- 折叠规则进脚本 = 重复压缩一致（手 edit 后跑脚本重压）。
- model_list 单行后加模型需 edit 单行数组（中等不便）。

## Out of Scope

- 改 JSON 字段 / 语义 / key 顺序
- 改 preset 数据本身（仅格式化）
- 压缩 models.json / 其他 defaults 文件（本 task 仅 platform-presets.json）
- 自动 CI 钩子强制格式（手维护 + 按需重压，不强制 CI）

## Technical Notes

- 真值源手维护（CLAUDE.md），本 task 是**格式化一次性重构**，改后仍手 edit；脚本辅助按需重压。
- Rust include_str!（`src-tauri/src/commands/defaults.rs`）+ serde_json 解析 → 折叠空白不影响。
- 前端 `src/domains/platforms/defaults.ts:80` getDefaultsJson → JSON.parse → 同。
- 既有 guide：`.trellis/spec/guides/code-reuse-rules.md`（脚本入库可复现）。
