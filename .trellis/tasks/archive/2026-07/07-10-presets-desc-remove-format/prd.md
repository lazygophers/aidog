# platform-presets.json 删 desc + peak_hours/models 单行压缩

## Goal

两件独立事，同改 `src-tauri/defaults/platform-presets.json` 真值源：

1. **删 per-protocol `desc` 字段**（64 协议 × 8 locale 全删）—— desc 非平台类型展示内容（name 才是），desc 在 UI 无实际消费价值，纯冗余体积。
2. **`peak_hours` + `models` 字段值单行紧凑存储** —— 当前 JSON pretty-print 整体展开（每数组元素/对象字段独立行），peak_hours/models 这两字段值改成单行紧凑（同一行内紧凑 JSON），减少行数 + 提升可读密度；整体文件仍 pretty-print（非整文件单行）。

**行为零变更**：JSON 内容语义不变（仅删 desc + 改 whitespace），Rust/前端解析结果与重构前等价（除 desc 缺失外）。

## 用户澄清决策（AskUserQuestion 已答）

| 问题 | 用户选 |
|---|---|
| desc 删除范围 | **仅 platform-presets.json**（client-types.json 的 desc 保留，不同语义） |
| 压缩粒度 | **字段级单行**（整体 pretty-print，仅 peak_hours/models 两字段值紧凑单行） |
| 前端 protocolDescription | **删函数 + UI 消费点全删**（非留返空） |

## Scope

### 改 platform-presets.json
- `src-tauri/defaults/platform-presets.json`：
  - 删每 protocol entry 的 `desc` 对象（64 协议，每协议 8 locale = 512 desc 字符串删）
  - `peak_hours` 字段值（数组）压成单行紧凑：`"peak_hours": [{"start_hour":0,"end_hour":6,"multiplier":0.5,"days_of_week":[1,2,3]}]`（同数组内紧凑，整体仍 JSON 合法）
  - `models` 字段值（数组）压成单行紧凑：`"models": ["model-a","model-b","model-c"]`
  - 其他字段（name/endpoints/model_list/base_url/...）保持现状 pretty-print

### 删前端 protocolDescription 消费
- `src/domains/platforms/defaults.ts`：删 `protocolDescription()` 函数（L190 附近，grep 定位）
- UI 消费点：grep `protocolDescription\|protocolDesc\|\.desc` 在 `src/` 全删（编辑表单描述展示 / 卡片描述 / 创建 modal 等所有点）
- **不删**：Rust 侧若仍解析 desc（defaults reader）可保留 Optional 字段（向后兼容远端 JSON 若仍带 desc），或一并删（grep Rust `desc` 定位）

### client-types.json（OUT OF SCOPE）
- desc 保留（不同语义：客户端类型描述，非协议平台描述）

## Requirements

### R1 写 format 脚本（手工 + 脚本辅助）
- platform-presets.json 是手维护真值源（CLAUDE.md「真值源 = platform-presets.json，手维护」），但压缩格式易手抖漂移
- **新增 `scripts/format-platform-presets.py`**：读 JSON → 删每 entry 的 `desc` → 对 `peak_hours`/`models` 字段值序列化紧凑单行（其他字段 pretty）→ 写回
- 脚本幂等可重入（跑两次结果一致）
- 运行：`python3 scripts/format-platform-presets.py src-tauri/defaults/platform-presets.json`
- 脚本入 CI/手动校验（可选，但至少 commit 前跑一次保证格式）

### R2 删 desc + 压缩（跑脚本）
- 跑 R1 脚本对 bundled `src-tauri/defaults/platform-presets.json`，落地：
  - 64 协议 desc 全删（grep `"desc"` 在 platform-presets.json = 0）
  - peak_hours/models 字段值单行（grep 验证：含 `"peak_hours":` 行后跟紧凑 JSON 而非换行展开）

### R3 删前端 desc 消费
- `src/domains/platforms/defaults.ts`：删 `protocolDescription()` fn + export
- grep UI 消费点（`protocolDescription\|protocolDesc`）全删调用
- 若展示描述的 UI 组件（如编辑表单/卡片）引用 desc，删展示位 + 相关 i18n key（若仅此用）

### R4 Rust 侧 desc 解析（轻量处理）
- grep Rust `desc` 在 `crates/aidog_core/src/gateway/`（defaults reader / preset struct）
- 若 struct 有 `desc` 字段：删（Option 字段向后兼容，但既已删 JSON 值，结构清理一致）
- 若 reader 不解析 desc（仅 name）：无改动

### R5 验证
- `cargo build --workspace` + `cargo test --workspace`（baseline 1382+，无回归）
- `cargo clippy --workspace --all-targets` 无新 warning
- `yarn build` 全绿（前端 desc 删后类型/调用全清）
- `yarn check:i18n`（若有，验 desc 相关 i18n key 删后无悬空）
- format 脚本幂等：连跑两次 git diff 空
- platform-presets.json 仍合法 JSON（`python3 -c 'import json; json.load(open(...))'`）
- grep `"desc"` 在 platform-presets.json = 0
- grep peak_hours/models 单行紧凑格式生效

## Acceptance Criteria

- [ ] `scripts/format-platform-presets.py` 落地（删 desc + peak_hours/models 单行紧凑，幂等）
- [ ] platform-presets.json 经脚本处理：0 desc / peak_hours/models 单行紧凑
- [ ] 前端 `protocolDescription()` fn + UI 消费点全删（grep `protocolDescription` 在 src/ = 0）
- [ ] Rust 侧 desc 解析清理（grep `desc` 在 preset reader，按实际结构定）
- [ ] cargo build/test/clippy --workspace 全绿，无新 warning
- [ ] yarn build 全绿（前端零残留 desc 引用）
- [ ] format 脚本幂等（连跑两次 git diff 空）
- [ ] 主仓零改动（worktree 内）

## Out of Scope

- client-types.json 的 desc（保留）
- platform-presets.json 的 name 字段（协议展示内容，保留不动）
- peak_hours 的语义/行为（仅改格式，不改逻辑）
- 远端同步链（desc 删后远端 JSON 若带 desc，reader 向后兼容即可，不改同步逻辑）
- platform-presets.json 整文件紧凑（用户选字段级，非整文件）
- platform-presets.json 路径/加载逻辑（不动 reader 路径，仅改内容/格式 + 删 desc 前端消费）

## Technical Notes

- 真值源约定（CLAUDE.md）：`src-tauri/defaults/platform-presets.json` 手维护，禁机器生成覆盖 —— 本 task 加 format 脚本是「格式归一工具」非「内容生成」，删 desc 是内容删（手决策），脚本只执行格式化（合规）
- format 脚本实现思路（Python）：
  ```python
  import json, sys
  path = sys.argv[1]
  doc = json.load(open(path))
  for proto, entry in doc["protocols"].items():
      entry.pop("desc", None)  # 删 desc
      # peak_hours/models 字段值紧凑（其他字段 pretty）
  # 用自定义 encoder：整体 pretty，但指定字段值 to_string 紧凑后嵌入
  # 实现：先整体 json.dumps pretty，再 regex 替换 peak_hours/models 行
  # 或：逐 protocol 手工拼接（更可控）
  ```
  具体 implement agent 定实现（regex 替换 / 手工拼接 / 第三方 lib 均可），验收只看格式结果 + 幂等
- desc 前端消费点（summary 记录）：`defaults.ts:190 protocolDescription()`，UI 消费点 grep 定位（编辑表单 / 卡片 / 创建 modal 可能引用）
- Rust 侧（summary 记录）：preset reader 在 `crates/aidog_core/src/gateway/` 某处（grep `desc` 定位是否解析）
- 跨层契约（spec guides/cross-layer-rules.md）：删 desc 不破坏 serde 契约（desc 非公共契约层字段，仅展示用；前端 TS 类型若有 desc 字段同步删）

## 数据流（强制）

```
platform-presets.json（删 desc + peak_hours/models 单行）
  ↓ include_str! bundled
Rust preset reader（解析，desc 缺失 Option/不解析）
  ↓ invoke
前端 defaults.ts（protocolDescription fn 删，UI 不展示 desc）
  ↓
三处 UI 无 desc 展示（编辑表单/卡片/创建 modal）
```
