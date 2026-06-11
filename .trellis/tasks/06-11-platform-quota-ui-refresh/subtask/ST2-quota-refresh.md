# ST2: 统一 quota 刷新

- **目标**: quota 内联刷新（balance+coding_plan 一起）
- **产出** (Platforms.tsx):
  - quota 分组标签旁内联刷新小图标（↻ SVG，btn-ghost 极小）
  - `quotaRefreshing: Record<number, boolean>` per-platform loading
  - `refreshQuota(p)`: baseUrl=getPrimaryBaseUrl(p.platform_type, p.endpoints)||p.base_url → quotaApi.query(baseUrl, p.api_key) → setQuotaMap[p.id]；loading 态图标转/禁用；catch → 错误 toast
  - 图标常驻（无 quota 数据也可触发首查），mock/claude_code 平台可隐藏（无 quota 意义）
- **验证**: tsc 0 / yarn build；点击刷新 balance+coding_plan + loading + 错误 toast
- **资源**: design.md、quotaApi.query(api.ts:545)、现有 toast/StatBadge
- **依赖**: ST1
- **失败处理**: tsc 错逐修禁 any
