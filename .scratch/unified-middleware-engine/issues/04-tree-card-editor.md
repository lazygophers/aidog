# 04: 条件树组卡片编辑器 + applies_to + 动作链编辑

**What to build:** 用户用可视化编辑器替代 JSON 编辑：递归条件组卡片（每组选 ALL/ANY、可加
子组/叶子，叶子 = target + field + match_type + pattern）、applies_to 三维多选
（platforms/groups/models，空 = 全部）、动作链有序编辑（六种动作、上下移动、参数表单）。
保存时混阶段校验拒绝并提示。

**Blocked by:** 02 CRUD + seed + 前端列表页（最小可用）

**Status:** ready-for-agent

- [ ] 组卡片递归增删改，叶子四要素齐全
- [ ] 混阶段保存被拒并提示
- [ ] applies_to 多选与动作链编辑提交的数据与引擎模型一致（组件测试断言提交 payload）
- [ ] yarn build / yarn test 全绿
