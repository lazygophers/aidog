# statusline 脚本 sh→python (golden-output 回归)

## Goal

把 statusline + subagent statusline 生成脚本从 bash 改 Python（uv run + PEP723，缺 uv fallback python3），**输出字节级与现 bash 一致**（ANSI 颜色/多行对齐/逐任务 JSONL 渲染契约不变），用 golden-output 回归测试保证。承接 scripts-py-uv（notify 已转、ScriptInvoker 基建已就位）。

## 背景（已定位）

- statusline 脚本内容由**前端 `editors.tsx`** 按用户 segment 配置拼成 bash 字符串（97 处 jq/printf/ANSI/sed/awk），经 command 传后端 `generate_statusline_script(content)`(lib.rs:1886) 写 `~/.aidog/scripts/aidog-statusline.sh`（现保留 bash）。subagent statusline 同理（逐任务 JSONL 见 [[subagent-statusline-native]]）。
- 渲染契约严格：ANSI 转义、多行列对齐（LeftTabStop）、coding plan 配色、分隔符、逐 task JSONL。**字节级不可漂移**。
- ScriptInvoker(scripts.rs)/aidog_scripts_dir/uv 流程已由 scripts-py-uv 就位 → 本任务复用，只换 statusline 内容 + 文件名 .sh→.py + command 串走 invoker。

## ⚠️ 方法论（golden-output 回归，强制）

1. **先建 golden**: 用代表性输入集（多组 statusline JSON：不同 model/group/coding plan/超长截断/多 task/RTL 等边界）跑**现 bash 脚本**，捕获 stdout **逐字节** 存为 golden fixtures。
2. **再写 Python**: 重写生成器输出 Python 脚本，对同输入集产出 **与 golden 逐字节一致** 的 stdout。
3. **回归测试**: 自动化 diff（bash golden vs python 实际），**任一字节不符即 FAIL**。覆盖全部 segment 类型 + 边界。
4. 不确定的渲染细节（如 printf 宽度/locale/数字格式）以 bash 实际输出为准，不臆测。

## Requirements

### R1 — statusline 生成器 bash→Python
- `editors.tsx`（+ 任何拼 statusline bash 的前端处）：bash 字符串生成器 → Python 字符串生成器。stdlib only（json/sys/os/re；ANSI 手写转义）。PEP723 头。
- 输出契约逐字节保持（见方法论）。subagent statusline 同步转。

### R2 — 文件名 + 执行器 + 路径
- `aidog-statusline.sh`→`.py`、`aidog-subagent-statusline.sh`→`.py`，写 `~/.aidog/scripts/`。
- statusLine `command` 串走 `ScriptInvoker.command_for`（uv run / python3），复用 scripts-py-uv 基建。
- 旧 statusline .sh（root + scripts/ 下）清理。

### R3 — golden-output 回归测试
- 加测试（Rust 或脚本）：对输入集跑 python statusline，diff golden bash 输出，字节一致。
- golden fixtures 入库（任务目录或 tests/fixtures）。

## Acceptance Criteria

- [ ] statusline + subagent statusline 生成 Python(.py, PEP723)，写 ~/.aidog/scripts/
- [ ] 对代表性输入集，python 输出与现 bash **逐字节一致**（回归测试通过）
- [ ] statusLine command 走 invoker（uv/python3）
- [ ] 旧 statusline .sh 清理
- [ ] cargo clippy 无新 warning + cargo test 绿（含 golden 回归）；yarn build 绿；check-i18n 零缺失
- [ ] 全 segment 类型 + 边界（截断/多 task/coding plan 配色/RTL）覆盖

## Definition of Done

- golden 回归字节一致 + cargo clippy/test 绿 + yarn build 绿 + check-i18n 零缺失
- 改动落 worktree，闭环 check→commit(merge)→archive
- 更新 [[subagent-statusline-native]] / [[statusline-persistence-flow]]（statusline 转 Python）

## Technical Approach

- 先在 worktree 内提取现 bash 生成器输出（对 fixtures 跑），存 golden。
- Python 重写：逐 segment 渲染函数对应 bash 逻辑；ANSI/对齐/分隔符精确复刻。
- 复用 scripts.rs ScriptInvoker + aidog_scripts_dir + cleanup。
- ⚠️ 实施安全：worktree 内可跑 bash/python 脚本对 fixtures（不碰用户真实 ~/.aidog/真实 Claude Code）；禁真装 uv。

## Out of Scope

- statusline 功能/布局变更（纯语言转换，输出不变）
- notify hook（scripts-py-uv 已转）
- statusline 配置 UI 逻辑（仅换生成的脚本语言）

## Technical Notes

- generate_statusline_script(lib.rs:1886, 写 content) / editors.tsx 拼 bash(97 jq/printf/ANSI) / ScriptInvoker(scripts.rs) / aidog_scripts_dir(lib.rs)
- statusline 被非交互调用：读 stdin JSON → 输出渲染行；subagent 逐 task JSONL
- 高风险点：printf 宽度/对齐、ANSI 转义、数字/金额格式、截断、RTL
- 参考 [[subagent-statusline-native]] / [[statusline-persistence-flow]] / [[usage-color-single-source]]
