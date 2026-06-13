# PRD: 单平台分组 logo 跟随平台

## 背景

Groups 页(分组与路由)每个 group 卡片左侧 group icon (Groups.tsx:740-748) 当前固定显示 `group.path.slice(0, 3)` 文字框。

Platforms 页平台 logo 方案: `getPlatformLogo(p.platform_type)` (来自 `src/assets/platforms`) + 无 logo 时 `getFaviconUrl(p)` favicon fallback (Platforms.tsx:1149-1150, 1192-1193)。

## 目标

group **只关联 1 个 platform** 时(`gps.length === 1`), group icon 显示该 platform 的 logo(与 Platforms 页一致: getPlatformLogo + favicon fallback), 而非 path 文字框。关联 0 或 ≥2 个平台时保持现有 path 文字框。

## 范围

- 仅改 `src/pages/Groups.tsx` group icon 渲染块(740-748)。
- 复用 Platforms.tsx 既有 `getPlatformLogo` / `getFaviconUrl` 逻辑(import from `../assets/platforms`), 禁重复实现。
- `gps` (group platforms, Groups.tsx:718 row.detail.platforms) 已可得, `gps[0]` 即唯一平台对象。

## 非目标

- 不改 Platforms 页 logo 逻辑
- 不改后端 / 数据结构
- 不改 ≥2 平台 / 0 平台的 group icon(保持 path 文字)

## 验收标准

- group 关联恰好 1 platform: icon 显示该 platform logo(有预设 logo 用 logo, 否则 favicon, 都无则回退 path 文字或中性)
- group 关联 0 或 ≥2 platform: 保持现有 path 文字框
- logo 加载失败(favicon 404)graceful 回退(复用 Platforms 的 faviconHasFailed 模式)
- yarn tsc 0 error 无新增 warning
- 复用 getPlatformLogo/getFaviconUrl, 无重复实现

## 编排

单一交付, 单 worktree, main 在 worktree 内 inline 实施(轻量模式, 不派 agent)。
