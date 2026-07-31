# 移除添加平台表单里的「从 cli-proxy 添加」入口 — PRD (主入口)

> 禁写具体文件路径与代码片段 (会很快过期) —— 例外: prototype 产出的能精确编码决策的片段 (状态机/schema/type shape) 可内联, 且须注明来自 prototype。

## 目标
- [ ] 添加平台表单（新建态）不再有「从 cli-proxy 添加」按钮与 provider 选择弹窗
- [ ] cli-proxy 平台的创建入口收敛到 CliProxy 页自身的「建平台行」按钮，一条路径不两处入口
- [ ] 已存在的 cli-proxy 平台仍可正常编辑，继承字段只读展示不受影响
- [ ] 用户价值：添加平台流程去掉一个语义重叠的旁路入口，减少「该从哪加」的选择困惑
## 边界
- [ ] 范围内：PlatformEditForm.tsx 删除「从 cli-proxy 添加」按钮、provider picker Dialog、showCliProxyPicker / cliProxyProviders state 与其拉取 effect
- [ ] 范围内：usePlatformForm.ts 的 createCliProxyPlatform 函数及其向下传递的 prop 链路，确认无其它调用点后一并删除
- [ ] 范围内：8 个 locale 清理仅此入口使用的 key（platform.cliProxy.addFromProvider / pickerTitle / pickerHint / pickerEmpty），仍被编辑态引用的 key 保留
- [ ] 范围外：不动 CliProxy 页 (src/pages/CliProxy/index.tsx) 的「建平台行」按钮与 handleCreatePlatform —— 那是反方向入口，是本次保留的唯一创建路径
- [ ] 范围外：不动编辑 cli-proxy 平台时的继承字段只读展示 (isCliProxyEditing 分支及 platform.cliProxy.inherited* / provider / wireProtocol / baseUrl / models 等 key)
- [ ] 范围外：不动后端 cli_proxy_cmd 的 create_platform command —— CliProxy 页仍在用
- [ ] 约束：删 i18n key 前必须 grep 确认无其它引用点，误删仍在用的 key 会让编辑态露裸 key
## User Stories
极其详尽地穷举, 覆盖功能各方面 (含边界情况) —— 穷举本身就是逼出边界情况的机械手段:
1. As a <actor>, I want <feature>, so that <benefit>

## 验收标准
- [x] 新建平台表单页头只剩「智能识别」按钮，无「从 cli-proxy 添加」按钮
- [x] provider picker Dialog 及其 state / effect 已从 PlatformEditForm.tsx 移除，无残留死代码
- [x] createCliProxyPlatform 及其 prop 链路已清理，或经 grep 确认仍有其它调用点而保留（二选一，需在改动说明中给出依据）
- [x] CliProxy 页「建平台行」按钮功能不变，仍能建出 cli-proxy 平台行
- [x] 编辑已有 cli-proxy 平台时，继承字段只读区展示正常，无裸 i18n key
- [x] 8 个 locale 中仅此入口使用的 key 已删干净且无遗漏语言；仍被引用的 key 未被误删
- [x] scripts/check-i18n.mjs 通过、yarn build 通过、yarn test 通过
## Testing Decisions
什么算好测试 (只测外部行为不测实现细节) / 测哪些模块 / codebase 内的同类测试先例:
- [ ] 不新建测试接缝：本 task 是纯删除，无新增逻辑分支值得单测，写测试等于给「代码不存在」写断言
- [ ] 唯一实质风险是「误删仍被编辑态引用的 i18n key」，由 `scripts/check-i18n.mjs` 直接覆盖（key 8 语言对齐 + 无引用点指向已删 key），这是复用的现有接缝也是最高接缝
- [ ] `yarn build` (tsc) 覆盖 state / prop 删干净后无悬空引用；`yarn test` 跑现有 18 个测试文件确认没碰坏别处
- [ ] 一条人工确认：编辑一个已有 cli-proxy 平台，继承字段只读区正常显示、无裸 key —— 自动化覆盖不到「视觉上是否露 key」

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list remove-cliproxy-add-entry`)
