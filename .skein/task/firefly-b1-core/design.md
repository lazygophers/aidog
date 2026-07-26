# 批1 高频核心页萤火虫迁移 — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json):

## 共用层(已就位, commit 564b074)
- `utils/motion.ts`: useReveal(stagger) / useCounter(target,decimals,durMs) / makeRipple
- `globals.css`: .counter / .ripple / .reveal.in / .hover-lift / .progress-fill.striped + prefers-reduced-motion 兜底
- CompactCard / StatChip / BalanceBar / CostTrendChart 已萤火虫化

## Home.tsx 改动面
- KpiCell 签名: 从 `{ value: string }` → `{ numeric: number, format: (n)=>string, decimals? }` 内部 useCounter
  - ponytail: 保留 `value` 作 fallback(numeric 缺省时直接显),向后兼容
- 4 KpiCell 各传 numeric(cost/tokens/requests 缓存率); 趋势主图末点 circle 加 drop-shadow(同 CostTrendChart 模式)
- 快捷操作 Button 加 className="ripple" + onClick makeRipple
- 顶部状态条 + KPI 带 + 趋势区 + 双栏 reveal 分组(stagger 0/80/160ms)

## Groups 改动面(已大量用 shared, 增量小)
- GroupListItem: 列表 map 时传 revealDelay={i*60} 给 CompactCard
- GroupCreateModal/GroupEditPanel: glass-surface 内联 padding 统一(20px), 不动表单逻辑
- StatChip/BalanceBar 已自动获益, 仅核查视觉

## Stats.tsx 改动面
- 4 glass-surface 统计卡包 useReveal(stagger) + className hover-lift
- 时间筛选(如有 segmented)激活态走 var(--primary) + accent-subtle
- 图表区 CostTrendChart 模式末点 glow

## Platforms 改动面(PlatformCard 53KB 巨石)
- PlatformCard: 卡片根 className 加 hover-lift + reveal; 状态徽标(status 三态)用萤火虫语义色
- 批量操作弹窗(BatchDelete/BatchMoveGroup/BatchOverrideModels/BatchSetStatus/Share/SmartPaste): createPortal(document.body) 核查(违反则修, 见 memory modal-window-center-rule)
- pages/platforms/ 12文件: 列表项 reveal stagger, 不动数据流

## 取舍
- ponytail: KpiCell 改签名会波及 Home 内 4 调用点 — 同文件内, 安全
- ponytail: PlatformCard 巨石只加 className/不动 JSX 结构 — 最小 diff 避免回归
- 共享 useReveal/useCounter 复用, 不在页内重写 IntersectionObserver

## 风险
- forwardRef: CompactCard 已接 ref(useReveal 内部), Card forwardRef 已确认
- prefers-reduced-motion: 动效类已兜底, 无障碍 OK
- 测试: KpiCell 改签名若被测试断言 value 文本, 需同步改测试(grep 确认)
