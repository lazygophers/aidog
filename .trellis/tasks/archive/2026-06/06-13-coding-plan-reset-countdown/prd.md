# Coding plan 展示重置倒计时

## 问题

1. **预估侧 `resetsAt` 硬编码 `null`** — `computeQuotaDisplay` 预估路径已算出 `remainMs`（用于 level 配色），但未转为 `resetsAt` 字符串。导致展开区的 `formatResetCountdown(tier.resetsAt)` 对预估 tier 始终为空。
2. **紧凑 header tier badges 不展示重置时间** — 平台卡片折叠态只显示 `45% 5h` 形式的 badge，无任何倒计时信息。

## 修复

### 1. 预估侧填充 `resetsAt`（`Platforms.tsx:943`）

将 `resetsAt: null` 改为从 `remainMs` 计算的 ISO 时间戳：

```ts
resetsAt: remainMs != null ? new Date(Date.now() + remainMs).toISOString() : null,
```

### 2. 紧凑 header 加重置倒计时（`Platforms.tsx:1258-1280`）

在 tier badge 后追加最小倒计时文字（仅 level 非 neutral 时显示，避免信息过载）。

## 验收标准

- [ ] 预估 coding plan tier 展开区显示重置倒计时
- [ ] 真查 tier 倒计时不受影响
- [ ] 紧凑 header tier badge 显示简要倒计时
- [ ] 无 `resetsAt` 时不显示多余文字
