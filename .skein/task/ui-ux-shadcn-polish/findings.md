# 全量审计清单（5 只读 agent 并行产出）

合计 **P0=1 P1=30 P2=62 ≈ 93 处**。已按根因归 7 批。

## 批1 · 功能性 CSS bug（真 bug，必修）
- `src/pages/Home.tsx:338,361` `color:"var(--danger)"` 全项目未定义（仅 `--color-danger`），负增长红失效 → 改 `var(--color-danger)`
- `src/pages/platforms/formSectionsEndpoints.tsx:146-148` `"var(--color-success)15"` 拼 alpha 非法 CSS，绿底/边永不渲染 → `color-mix(in srgb, var(--color-success) 8%, transparent)`
- `src/pages/platforms/PlatformEditForm.tsx:234-236` `${colorMap[protocol]||"var(--accent)"}20` fallback 生成 `var(--accent)20` 非法 → color-mix
- `src/pages/Logs/ListView.tsx:71` `padding:"12px 16"` 缺 px → 内边距塌陷 → `"12px 16px"`
- `src/pages/Logs/DetailPanel.tsx:113` `padding:"12px 20"` 缺 px → `"12px 20px"`

## 批2 · 自研 `.toggle` → shadcn Switch（+ 键盘 a11y）
根因 `src/styles/globals.css:396 .toggle` + 共享 `src/components/settings/editors/_shared.tsx:16 Toggle`（仅 Settings 族内 ~11 文件引用 → 改共享底层自动迁移）。
- Settings 族 raw `.toggle` div: CcSwitchImport:350 / Sub2ApiImport:287 / MiddlewareRules:331,455,743,761 / MitmConfig:244,555 / NotificationEventList:177-222(×4) / NotificationSettings:256,296,383,430 / SchedulingSettings:84
- CodingToolsSettings:94,376,385,394 走共享 `<Toggle>`（改底层即迁）
- AppSettings 族 raw `.toggle`（**键盘不可操作 a11y**）: StartupSection:30,52,74,97 / LogSettingsSection:95,115,130,257 / ProxyStatusSection:98,180 / SystemMiscSection:180
- 其它: PricingTab:179 / TrayConfigTab:576 / PopoverConfigTab/CardEditor:42

## 批3 · 自研 modal/popover/dropdown → shadcn
- `TrayConfigTab.tsx:418-516` 弹窗手搓 fixed 未 createPortal（违居中规则）→ shadcn Popover
- `ModelTestPanel.tsx:116-125` 手搓 modal → Dialog
- `platforms/ModelsMatrixSection.tsx:214-260` 手搓 popover → Popover+Command
- `shared/FilterDropdown.tsx:54-96` 自研浮层 → Popover/Select
- `shared/CopyButton.tsx:161-196` 自研 portal 菜单 → DropdownMenu

## 批4 · legacy class 叠加 shadcn 清理
- `SettingsHeader.tsx:59-72,148` `btn btn-primary/ghost` 叠加 → variant
- `CodexSettings.tsx:150-166,203-208` `btn btn-primary/ghost` 叠加 → variant
- `Groups/GroupCreateModal.tsx:65,76` `className="input"` 叠加 shadcn Input → 删
- `SectionAnchorNav.tsx:42-64` Button inline 全覆盖 → 自定义 class
- `MitmConfig.tsx:563-572` Button 裸 ✕ → `variant=ghost size=icon`
- `platforms/formSections.tsx:387-391,627-631` `<Input type="checkbox">` → ui/Checkbox
- `CcSwitchImport.tsx:466-487` DimCheckbox → ui/Checkbox

## 批5 · hex/rgba fallback + 硬编码色 token 化（P2 大批）
删 `var(--x,#hex)` / `var(--x,rgba())` 兜底、`color:"#fff"`→`var(--accent-foreground)`、统一 `--border-color/--border-default`→`--border`。分布：
- A: Home:312 阴影 / About:173-526 多处 hex 兜底 / CopyButton:170
- B: PlatformListView / ModelsMatrix:318 / PlatformEditForm / MultiKeyPreview
- C: MitmConfig:259-547 / MiddlewareRules:665 / Groups(GroupEditPanel/GroupListView/GroupListItem) / CcSwitchImport:224,482 / Sub2ApiImport:251 / NotificationEventList:171
- D: ModelTestPanel:118,180 / Logs/primitives:28-304 / LogSettingsSection:233 / DetailPanel:102,128 / CliProxy:48
- E: SkillDetailView:138,232,252 / SkillInstallView:198,282 / CardEditor:52,136 / TrayConfigTab:473-669(color:#fff×8)
- TrayConfigTab:326-439 macOS 菜单栏模拟色 **刻意保留**（仅加注释）

## 批6 · 响应式布局
- `SkillsView.tsx:38` header 6 按钮无 flex-wrap → 加 wrap
- `CliProxy.tsx:436` `gridTemplateColumns:"1fr 1fr"` 固定 → `repeat(auto-fit,minmax(200px,1fr))`
- `platforms/MultiKeyPreview.tsx:60-67` 5 列 grid 无 overflow-x → 加 `overflow-x:auto`

## 批7 · 裸 `<table>` → ui/table
- `Stats.tsx:532-562,686`
- `PricingTab.tsx:279-316`

## 干净（无问题）
CompactCard/StatChip/BalanceBar/CostTrendChart/TestResultBody, WindowsEditModal(Dialog正例), Platforms.tsx, Settings.tsx, Groups.tsx, UnsavedChangesModal(正例), Skills.tsx/SkillModals(正例), Notifications.tsx, Mcp.tsx, Logs.tsx
