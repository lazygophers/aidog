# 主题系统

## 架构

每主题提供 **light + dark** 两组 CSS 变量。切换主题时替换整个变量集。

### 文件结构

```
src/themes/
├── index.ts         # 主题注册入口
├── types.ts         # ThemeConfig 类型定义
├── liquidGlass.ts   # 默认主题
├── nord.ts          # 北极蓝调
├── dracula.ts       # 暗色系
├── catppuccin.ts    # 温柔配色
└── solarized.ts     # 经典科学
```

### CSS 变量示例

```css
/* Liquid Glass Dark */
:root[data-theme="liquidGlass"][data-mode="dark"] {
  --bg-primary: rgba(30, 30, 30, 0.8);
  --text-primary: #ffffff;
  --accent: #007aff;
  /* ... */
}
```

## 主题切换

1. 用户在设置中选择主题
2. 前端更新 `data-theme` 和 `data-mode` 属性
3. CSS 变量自动切换

## Light / Dark 模式

- 跟随系统设置（`prefers-color-scheme`）
- 手动切换覆盖系统设置
- 每个主题独立定义两组颜色

## 创建自定义主题

1. 复制现有主题文件
2. 修改 CSS 变量值
3. 在 `index.ts` 注册
4. 重新构建应用

## UI 风格偏好

项目偏好 **Liquid Glass** 风格：
- 玻璃拟态（backdrop-filter: blur）
- 半透明背景
- 柔和阴影
- 圆角卡片
