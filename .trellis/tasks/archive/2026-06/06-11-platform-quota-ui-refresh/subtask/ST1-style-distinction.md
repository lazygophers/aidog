# ST1: quota 区样式区分 + 位置

- **目标**: coding plan/余额 与 usage 视觉+位置区分
- **产出** (Platforms.tsx :1645-1662 区):
  - quota 区独立分组：加间距(marginTop:8)/细分隔 + 「额度」小标签（text-secondary）
  - quota badge 区别于 usage：包裹容器加浅边框/底色（glass-surface，Liquid Glass）或 StatBadge quota 变体
  - usage（:1636）保持，与 quota 拉开层级
- **验证**: tsc 0；视觉 usage/quota 分组分明
- **资源**: design.md、Platforms.tsx StatBadge/卡片渲染
- **依赖**: 无
- **失败处理**: 别窗口若同改该区 → 仅改 quota 区行，记录冲突
