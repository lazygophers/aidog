# Design: 平台 quota 展示区分 + 刷新

## 渲染改动（Platforms.tsx 卡片 :1645-1662 区）

### 区分样式 + 位置（需求1）
- quota 区从紧贴 usage（marginTop:4）改为**独立分组**：
  - 加分组容器：上方细分隔线或更大间距（marginTop:8）+ 左侧小标签「额度」/「Quota」（text-secondary 小字）
  - quota badge 用区别于 usage 的视觉：如外层加浅边框/不同背景透明度（Liquid Glass glass-surface），或 StatBadge 加 `variant="quota"` 风格（边框/底色），与 usage 的纯 badge 拉开
  - 位置：usage（吞吐统计）一组，quota（账户额度）另一组，视觉层级分明
- 保留 StatBadge 复用，但 quota 组加包裹容器 + 标签 + 刷新图标，形成独立视觉单元

### 统一刷新（需求2+3）
- quota 分组标签旁加内联刷新小图标按钮（↻ SVG，btn-ghost 极小尺寸）
- per-platform loading state：`const [quotaRefreshing, setQuotaRefreshing] = useState<Record<number, boolean>>({})`
- handler：
  ```
  const refreshQuota = async (p: Platform) => {
    setQuotaRefreshing(s => ({...s, [p.id]: true}));
    try {
      const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url;
      const q = await quotaApi.query(baseUrl, p.api_key);
      setQuotaMap(s => ({...s, [p.id]: q}));   // 同时更新 balance+coding_plan
    } catch (e) { setToast({text: `${p.name}: 刷新额度失败`, ok:false}); }
    setQuotaRefreshing(s => ({...s, [p.id]: false}));
  };
  ```
- 刷新中图标转动/禁用；无 quota 数据时也可触发首次查询（图标常驻）
- 注意：quota 仅对有 balance/coding_plan 能力的平台有意义；图标可对所有非 mock/claude_code 平台展示（或仅 quotaMap 有值/可查的）

## 不改
- 后端（quotaApi.query / platform_query_quota 已合查 balance+coding_plan）
- usage 区（保持原样，仅与 quota 拉开区分）

## 验证
- tsc --noEmit 0 / yarn build
- 视觉：usage 与 quota 分组区分明显；刷新图标点击刷新 + loading + 错误 toast
