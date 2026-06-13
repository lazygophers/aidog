# 前端 tsx 硬编码中文 i18n 化

## 背景
前端 28 个 tsx 含硬编码中文 (grep 中文字符计数: editors 2826 / Platforms 2266 / Groups 561 / Stats 449 / Logs 321 / 等)。i18n 框架已就绪 (8 locale JSON + react-i18next), 但部分页面未走 t()。

## 目标
用户可见 UI 文本 (JSX children + placeholder/title/label/aria prop + alert/toast 消息) 全部走 t()。注释/调试 log 不处理。

## 优先级 (用户可见度降序)
P0 核心导航/页面: Sidebar, Platforms, Groups, Logs, Stats
P1 设置: AppSettings, PricingTab, TrayConfigTab, PopoverConfigTab, CodexSettings, Settings
P2 设置子组件: NotificationSettings, SchedulingSettings, MiddlewareRules, shared/*
P3 深层: editors.tsx (Claude Code 设置编辑器, 最大), SettingsHeader, SectionAnchorNav, UnsavedChangesModal, icons

## 交付
- 每文件: 提取硬编码中文 → 加 i18n key (zh-CN + en-US + 6 locale 翻译) → 替换 t()
- key 命名: <page>.<context>.<name> 风格, 与现有 flat 点 key 一致
- 分批 commit (每 P 级一组)

## 验证
- yarn build (tsc + vite) 通过
- 无残留用户可见硬编码中文 (JSX 文本/字符串 prop, 注释除外)

## 不含
- 注释中文化保留 (不影响用户)
- icons.tsx 纯符号不处理
