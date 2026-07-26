# 批1 高频核心页萤火虫迁移 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] Home/Groups/Platforms/Stats 4 高频页组件级结构按 example 萤火虫规范重写(非仅 CSS 变量)
- [ ] KPI 数字 useCounter 滚动 + 卡片 reveal 入场 + 按钮 ripple + hover 流光描边
- [ ] 复用已重构 shared(CompactCard/StatChip/BalanceBar commit 564b074),零重复造轮
- [ ] 趋势图/排名表萤火虫配色 + 末点 glow

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: src/pages/{Home,Groups,Stats}.tsx + Groups/ 子目录 + platforms/ 7组件 + pages/platforms/ 12文件
- [ ] 范围外: settings 子件(批2) / Logs/Mcp/Skills(批3) / PricingTab/CodexSettings 等(批4)
- [ ] 约束: 8 语言 i18n key 不删不改; modal createPortal; navGuard 不动; 内联 padding 统一走 var(--space-*) 或语义类
- [ ] 约束: PlatformCard 53KB 巨石只改视觉层(卡片签名/状态徽标/hover),不动数据流/表单逻辑
- [ ] 约束: 各页子目录(ListView/DetailPanel/McpView 等)在本批外则批3处理

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] yarn tsc --noEmit 0 error
- [ ] yarn test 全 pass(现有 281 + 本批改的测试文件)
- [ ] yarn build 成功
- [ ] Home: KpiCell 4 指标均 counter 动画(进入视口 1.2s 滚动); 趋势主图末点 drop-shadow glow; 快捷操作按钮 ripple
- [ ] Groups: GroupListItem 列表 reveal stagger(revealDelay 递增); StatChip/BalanceBar 新 pill+glow 可见
- [ ] Stats: 统计卡 hover-lift + reveal; 时间筛选 segmented 萤火虫激活态
- [ ] Platforms: PlatformCard hover-lift + 状态徽标萤火虫语义色; 批量操作弹窗 createPortal(document.body) 核查通过
- [ ] 无 console error/warning(React key/ref/forwardRef)

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list firefly-b1-core`)
