# ui-state-persist — PRD (主入口)

## 目标
持久化 3 层 UI 折叠/展开态,跨会话恢复:
1. **Groups 页分组折叠**(per-group,collapsedGroups: Set<number>)
2. **Platforms 页平台卡展开**(per-platform,expandedIds)
3. **GroupListItem 内卡展开**(per-platform,Groups 页内)

("打包"需求经澄清 = 笔误,实指折叠态,与上合一。)

## 用户价值
重启应用后无需重新展开/折叠,恢复上次视图状态。UX 连续性。

## 边界
- 存储:**仅 DB,作为 group.extra / platform.extra 的 _ui_* 键**(用户指定)
- 3 个 UI 键:
  - `group.extra._ui_collapsed`(bool,分组折叠)
  - `platform.extra._ui_expand_plat`(bool,Platforms 页卡展开)
  - `platform.extra._ui_expand_grp`(bool,Groups 页内卡展开)
- 各页展开态独立(W4):同一 platform 在 Groups 页内卡与 Platforms 页卡展开态**不共享**(两键并存)

## 非目标
- 不改 extra 现有业务键(peak_hours / coding_plan / breaker / time_models / disable_during_peak)
- 不持久化侧栏导航折叠态(Sidebar expandedNav/collapsedSection)— 用户未选
- 不做多设备分设备态(W3 单机接受 extra 共享)
- 不引入独立 ui_state 表(用户明确选 extra)

## 验收标准
- [ ] 重启应用后,3 层折叠/展开态恢复上次设定
- [ ] toggle 触发 debounce 300ms 后写 DB,非每 toggle 即写
- [ ] import/export 导出配置不含 _ui_* 键(strip);import 不恢复 UI 态
- [ ] 后端 extra 业务解析函数(peak_hours_for 等)行为不变
- [ ] 门禁:cargo clippy+test / yarn build+test+check:i18n

## 索引
- 详细设计: [design.md](design.md)
- 任务/子任务/调度: task.json (`skein.py subtask list ui-state-persist`)
- 契约: 8 条(`skein.py contract ui-state-persist`)
