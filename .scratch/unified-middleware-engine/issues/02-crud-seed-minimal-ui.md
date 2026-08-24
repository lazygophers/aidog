# 02: CRUD + seed + 前端列表页（最小可用）

**What to build:** 用户在设置页看到重建后的中间件规则列表：可新建/编辑（JSON 级编辑
conditions/actions/applies_to）、启停、删除；内置规则只可启停不可编辑删除；旧模型残留显示为
Failed Rule 引导手删；启动时按 name upsert seed 内置规则（内容强制覆盖、保留停用态）；
8 类子开关删除只留总开关；「一键导入默认」前后端入口全删。

**Blocked by:** 01 统一引擎基座

**Status:** ready-for-agent

- [ ] CRUD command 在新表上工作，builtin 的 edit/delete 被拒、仅 toggle
- [ ] seed upsert 测试：内容覆盖、停用态保留
- [ ] Failed Rule 标记展示 + 删除引导
- [ ] 列表页组件测试：渲染、启停、builtin toggle-only、导入默认按钮不存在
- [ ] 子开关只剩总开关，invoke 名注册表零差集自比对无孤儿命令
- [ ] yarn build / yarn test / cargo test 全绿
