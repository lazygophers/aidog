# 菜单容器滚动支持: 高度不足时显示滚动条

## 范围 (用户确认全部)

1. **侧栏 Sidebar** — 纵向 nav 列表, 窗口矮 / 分组展开多时纵向溢出
2. **设置锚点 SectionAnchorNav** — 设置页内纵向锚点导航, 锚点多时纵向溢出
3. **设置 tab 栏 AppSettings** — 横向 tab (9 个), 窗口窄时横向溢出
4. **其他纵向菜单/列表** — hooks / perm / middleware 规则列表等纵向容器

## 根因

当前菜单容器多为固定布局或无 overflow 约束, 内容超出父容器高度时直接溢出/裁剪, 用户无法滚动查看全部项。窗口缩小或内容增多时不可用。

## 方案

通用: 菜单容器在父高度不足时 overflow 滚动, 内容全部可见。

- **纵向容器** (Sidebar / SectionAnchorNav / 规则列表): 外层 flex column, 列表区 `flex: 1; minHeight: 0; overflow-y: auto` 占满剩余高度, 溢出滚。或 `maxHeight: calc(100vh - <header offset>)` 约束 + `overflow-y: auto`。
- **横向容器** (AppSettings tab 栏): `overflow-x: auto; flex-wrap: nowrap` (强制单行, 溢出横向滚, 禁换行破坏布局)。
- `minHeight: 0` 是 flex 子项 overflow 生效的关键 (flex 默认 min-height:auto 不收缩)。

## 滚动条样式 (Liquid Glass 一致)

加全局 `::-webkit-scrollbar` 细滚动条 (宽 6-8px, 半透明, hover 加深), 与 glass 主题协调。避免默认粗滚动条破坏视觉。放 `src/index.css` 或主题 CSS。

## 验证

- 窗口高度缩小: Sidebar / SectionAnchorNav / 规则列表纵向可滚
- 窗口宽度缩小: AppSettings tab 栏横向可滚
- 正常尺寸: 外观无回归 (滚动条仅溢出时出现)
- `npx tsc --noEmit` 无错
- 滚动条样式各主题 (light/dark) 协调

## 不做

- 不重构菜单组件结构 (仅加 overflow/flex 约束 + 滚动条样式)
- 不改菜单交互逻辑
- 不加虚拟滚动 (项数有限, native overflow 足够)
