# Skills 启用/关闭乐观更新 (去全列表刷新闪烁)

## Goal

Skills 页点击启用/关闭某 agent 时，改为**乐观更新**本地状态（立即翻转该行该 agent 的启用态），不再全列表 `loadInstalled()` 重载——消除每次操作的「刷新中」闪烁与等待，提升交互体验。

## 背景 / 根因（已定位）

- `Skills.tsx loadInstalled`(:55-67)：`setInstalled([])` 清空 + `setInstalledLoading(true)` → 列表整体闪 loading。
- `applyResult`(:97-100)：enable/disable 成功后 `await loadInstalled()` → 全量重载 + 闪烁 + 等待后端返回列表。
- `handleToggle`(:108-122)：每次点击都走 applyResult → 全列表刷新。
- 统计 counts 从 `installed` 派生(:79-80)，列表更新即同步。

## Requirements

### R1 — toggle 乐观更新
- `handleToggle`：点击后**立即**在本地 `installed` 状态翻转该 skill 的 `enabled_agents`（启用→去掉该 agent，未启用→加该 agent），UI 即时反映，无 loading 闪烁。
- 调 `skills_enable`/`skills_disable`。
- **成功**：保留乐观状态（不再 `loadInstalled()` 全量刷）。可选静默后台对账（不 set loading、不清空列表）。
- **失败**（res.success===false 或抛异常）：**回滚**该行乐观改动 + `setMessage` 弹错误。
- 保留 `busyKey` 防并发（仅禁该行/该操作，整页其他可交互）。

### R2 — loadInstalled 不闪烁（仅初次/scope/agent 切换用）
- 初次进页 / scope 切换仍可 `loadInstalled`，但避免不必要的 `setInstalled([])` 清空导致整页闪（首屏可保留 loading，后续刷新用「保留旧数据 + 静默更新」）。
- toggle 路径**不再调** loadInstalled。

### R3 — 统计同步
- counts 从乐观更新后的 `installed` 派生，自动跟随；无需独立全量 loadCounts 重算（若有独立 loadCounts 调用，toggle 路径同样去掉，改派生）。

## Acceptance Criteria

- [ ] 点击启用/关闭：该 agent 图标态**立即**翻转，无整页「刷新中」闪烁、无明显等待
- [ ] 后端成功：状态保持正确
- [ ] 后端失败：该行回滚到操作前 + 弹错误消息
- [ ] busyKey 仍防该行并发；整页不卡死
- [ ] 统计数字随翻转即时更新
- [ ] yarn build 绿；check-i18n 零缺失；无新增裸 key

## Definition of Done

- yarn build 绿；check-i18n 零缺失
- 仅前端 `src/pages/Skills.tsx`（无后端/契约变更）
- 改动落 worktree，闭环 check→commit(merge)→archive

## Technical Approach

- `handleToggle` 重写：先存 `prev = installed`，乐观 `setInstalled(installed.map(s => s===skill ? {...s, enabled_agents: 翻转} : s))`，再 await 后端；失败 `setInstalled(prev)` + setMessage(err)。
- 去掉 toggle 成功路径的 `loadInstalled()`/`loadCounts()` 全量调用。
- `loadInstalled` 刷新时不先 `setInstalled([])`（保留旧数据直到新数据到，避免闪）；首屏可保 loading。
- counts 已派生(:79-80)，无需改。

## Out of Scope

- 后端 enable/disable 逻辑（不动）
- 列表数据结构 / 契约
- 搜索/catalog（已无）

## Technical Notes

- 现状：loadInstalled(:55) / applyResult(:97) / handleToggle(:108) / counts 派生(:79) / busyKey(:37) / installedLoading(:34)
- 纯前端单文件，与并行任务 notif-hook-default-inject(后端)/scripts-py-uv(后端脚本) 无文件重叠 → 可并行
- 参考 [[skills-management-module]]
