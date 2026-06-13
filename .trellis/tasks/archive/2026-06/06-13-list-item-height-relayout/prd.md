# 增加平台/分组列表 item 高度并重排布局

## 设计依据（huashu-design 哲学）

本应用 = AI 网关管理器 → **高信息密度型**分型（Dashboard/Tracker 类）。
原则：每条卡片 ≥3 处差异化信息，加法优先于克制，但需呼吸感。
反 slop：不新增装饰性 icon/border/渐变，仅重构现有信息层次。

## 当前问题

两页 card header 都是**单行水平排列**，元素过多导致拥挤：
- Platforms: 拖柄 + logo + 名称 + 余额 + 预算 + coding tiers + 快操作（7+ 元素挤一行）
- Groups: 拖柄 + 图标 + 名称 + 统计 + 余额 + 复制/编辑/删除（8+ 元素挤一行）

padding=12，高度约 58px，信息拥挤难读。

## 设计方案：两行 header 布局

### Platforms 卡片
```
行 1（身份行）: [拖柄] [logo·健康点] [名称 + 协议·URL]  ←spacer→  [快操作]
行 2（指标行）: [余额bar] [手动预算] [coding tiers]  [使用统计]
```

### Groups 卡片
```
行 1（身份行）: [拖柄] [图标] [名称 + 路径·路由·平台数]  ←spacer→  [复制/编辑/删除]
行 2（指标行）: [统计 chips: tokens/cost/ok]  [余额bar]
```

### CompactCard 调整
- padding: 12 → 16（增加内呼吸感）
- header 区域从 `alignItems: center` 单行改为 `flexDirection: column; gap: 8` 两行

## 验收标准

- [ ] Platforms 卡片 header 两行布局，高度增加
- [ ] Groups 卡片 header 两行布局，高度增加
- [ ] 快操作（刷新/开关/复制/编辑/删除）固定在行 1 右侧
- [ ] coding plan tier 倒计时/余额/统计在行 2 清晰可读
- [ ] 不新增装饰性元素（反 slop）
- [ ] TSC 通过
