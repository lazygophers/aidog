# SubagentStatusLine 首段动态化

## Goal

把 aidog 子代理状态行（SubagentStatusLine）默认布局的首段从固定字面量 `[Agent·●]` 改为 **type·status·model 动态驱动**：渲染 `[{type_label}·{status_symbol}·{model}]`，type 走 `local_agent→Agent` 映射、status 走符号+颜色映射、model 取任务/顶层模型。移植 ccplugin `scripts/subagent_statusline.py` 的 `_STATUS_MAP` + `_status_seg` + type_label 逻辑到 aidog 的 segment→bash 生成机制。完成后：子代理行首段随任务 type/status/model 变化，而非恒为 `[Agent·●]`。

## What I already know
### 现状
- aidog 子代理状态行 = 配置驱动 segment → bash 脚本（`src/components/settings/editors.tsx`）。
- 默认布局 `DEFAULT_SUBAGENT_SEGMENTS`（editors.tsx:2642）首段 `sa-prefix` = separator `options.char="[Agent·●]"`（字面量，editors.tsx:2643-2644）。
- segment 生成 bash 见 editors.tsx「Script generation from segments」(~2703起)；custom 段用 jq expr（如 `.agent.name // .session_name`）。`bashEscapeDq` 转义字面量。
- 数据字段参考 STATUSLINE_DATA_FIELDS（editors.tsx:2672）—— 需确认子代理 payload 是否含 task type/status/model 字段（Claude Code 子代理状态行 spec: tasks[].{type,status,model}）。
### 参考实现（权威逻辑源）
- `/Users/luoxin/persons/lyxamour/ccplugin/scripts/subagent_statusline.py`（Python）：
  - `_STATUS_MAP`（status→符号+catppuccin 色）：running/in_progress=●黄, pending/queued=○灰, completed/succeeded/success=●绿, failed/error=●红, cancelled/canceled=◌灰, 默认 ◆蓝。
  - `_status_seg(status)` → `·{symbol}` 上色加粗。
  - type_label：`{"local_agent":"Agent"}.get(type.lower(), type)`，type 为空则不渲染整个 badge。
  - 结构：`[{type_label}` mauve + `·{status_symbol}` 状态色 + `·{model}` 蓝 + `]` mauve bold。

## Assumptions (temporary)
- aidog statusline 走 bash+jq，status→符号/色映射需在 bash 生成端实现（新增段类型或 jq case 映射）。
- 子代理 payload 提供 type/status/model（按 Claude Code 子代理状态行 spec）；若 aidog 子代理 payload 字段名不同，按实际可用字段适配。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D1 | DEFAULT_SUBAGENT_SEGMENTS 首段动态化 + 必要的段类型/生成支持 | diff | yarn build；生成脚本首段随 type/status/model 变化（样例 payload 验证）| P0 |

## Requirements
- R1 首段从字面量 `[Agent·●]` 改为动态：`[{type_label}·{status_symbol}·{model}]`，与 ccplugin 语义一致（type 空则不渲染 badge）。
- R2 status→符号+颜色映射移植 `_STATUS_MAP`（含默认 ◆蓝）；type_label `local_agent→Agent`。
- R3 优先复用现有 segment 机制：若 custom jq expr 能表达则用之；status 符号/色映射若 jq/bash 难表达，新增专用段类型（如 `agent-badge`）在 bash 生成端内置映射。
- R4 model 取 task 级 model 优先、回退顶层 model（对齐 ccplugin `model_label`）。
- R5 保持其余段（name/ctx/tokens/duration）不变。

## Acceptance Criteria
- [ ] yarn build（tsc && vite build）通过。
- [ ] 生成的子代理 bash 脚本首段随 type/status/model 动态变化（构造样例 payload 跑脚本验证，非恒 `[Agent·●]`）。
- [ ] status 各态符号/颜色与 ccplugin `_STATUS_MAP` 一致；type 空时不渲染 badge。
- [ ] 现有其余段行为不变。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交；worktree 合并+移除；动态段实现方式落 cortex（若新增段类型）；bump 版本（用户可见）。

## Out of Scope
- 改 ccplugin（其已实现，仅作参考源）。
- 主状态行 DEFAULT_SEGMENTS（仅改子代理默认）。
- 通知/中间件/调度树范围。

## Technical Notes
### 文件位置
- `src/components/settings/editors.tsx`：DEFAULT_SUBAGENT_SEGMENTS（2642）+ segment→bash 生成（~2703）+ 可能新增段类型 + STATUSLINE_DATA_FIELDS 补子代理字段。
- 参考逻辑：ccplugin/scripts/subagent_statusline.py（只读）。
### 资源边界
- 仅碰 editors.tsx（前端）。与运行中 C2 后端、已归档 C5 无文件交集；与未启动 N3/GB 前端注意 editors.tsx 是否被它们碰（目前不碰）。可独立并行。
### 验证命令
```bash
cd <worktree> && yarn build
# + 构造样例 subagent payload JSON 跑生成的脚本验证首段
```
