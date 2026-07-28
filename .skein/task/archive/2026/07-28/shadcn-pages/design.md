# 前端页面全量迁移至 shadcn — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json):

## 前置

依赖 shadcn-primitives 完成 (ui/ 组件库可用)。

## 域拆分 (并行 subtask)

按业务域拆,各 subtask 独立验收,彼此不互挂依赖 (只依赖 primitives):

| 域 | subtask | 文件 |
|---|---|---|
| settings | `migrate-settings` | AppSettings.tsx + Settings.tsx + CodexSettings.tsx + TrayConfigTab.tsx + PopoverConfigTab.tsx + components/settings/* (SchedulingSettings/MiddlewareRules/CodingToolsSettings/MitConfig/CcSwitchImport/NotificationSettings/SectionAnchorNav/editors/SettingsHeader/Sub2ApiImport/NotificationEventList/UnsavedChangesModal + editors/SandboxSection/ImportDiff/_shared) |
| platforms | `migrate-platforms` | Platforms.tsx + pages/platforms/* + components/platforms/* (PlatformCard/SmartPasteModal/BatchDeleteModal/BatchOverrideModelsModal/BatchMoveGroupModal/BatchSetStatusModal/ShareModal) |
| groups | `migrate-groups` | Groups.tsx + pages/Groups/* |
| logs | `migrate-logs` | Logs.tsx + RequestLog.tsx + pages/Logs/* |
| stats | `migrate-stats` | Stats.tsx + PricingTab.tsx + components/shared/CostTrendChart |
| skills-mcp | `migrate-skills-mcp` | Skills.tsx + SkillDetailView.tsx + SkillInstallView.tsx + Mcp.tsx + pages/Skills/* + pages/Mcp/* |
| misc | `migrate-misc` | Home.tsx + About.tsx + Notifications.tsx + ModelTestPanel.tsx + CliProxy.tsx + components/shared (StatChip/BalanceBar/CompactCard/CopyButton/TestResultBody/FilterDropdown/Modal — 后者删/换 Dialog) |
| top-level | `migrate-toplevel` | App.tsx + components/Sidebar.tsx + PopoverCards.tsx + UpdatePromptModal.tsx + SortableList.tsx (dnd-kit 保留,内 button 换) |

8 subtask。可全并行 (claim 批量)。

## 迁移模式映射

| 原生/自定义 | shadcn 替代 | 备注 |
|---|---|---|
| `<button class="...">` | `<Button variant/size>` | variant: default/outline/secondary/ghost/destructive/link |
| `<input>` + 自定义 label div | `<Field><FieldLabel><Input/></Field>` | FieldGroup 容器,非裸 div |
| `<select>` + options | `<Select>` (简单) / `<Combobox>` (搜索) | 短列表 Select,长/可搜 Combobox |
| `<textarea>` | `<Textarea>` (FieldGroup 内) | |
| 自定义 `*Modal.tsx` (Portal) | `<Dialog>` / `<AlertDialog>` / `<Sheet>` | Radix Portal 内置,删自研 Modal.tsx |
| 自定义 dropdown | `<DropdownMenu>` | |
| 自定义 confirm() | `<AlertDialog>` | 禁原生 confirm (CLAUDE.md) |
| 自定义 tab | `<Tabs><TabsList><TabsTrigger>` | trigger 必在 list 内 |
| `<hr>` / border-t div | `<Separator>` | |
| 自定义 toast | `sonner` `toast()` | |
| 自定义 badge span | `<Badge variant>` | |
| 自定义空状态 | `<Empty>` | |
| 自定义 loading pulse | `<Skeleton>` | |
| 图标 + 手动 size | lucide + `data-icon` | 无 sizing 类 |
| `space-x/y-*` | `gap-*` (flex) | |
| `w-X h-X` 等值 | `size-X` | |

## 不变量 (迁移不可破)

- Tauri invoke 调用 + 参数名零改 (Rust↔TS 边界,见 memory ts-protocol-rust-serde-sync)
- i18n key 零改 (8 locale 文件不动 key,check-i18n 过)
- 数值格式化走 `utils/formatters.ts` (CLAUDE.md)
- 导航走 App.tsx 本地 state + utils/navGuard.ts,禁 react-router (CLAUDE.md)
- modal 经 Radix Portal (memory modal-window-center-rule)
- 业务逻辑 / props 契约 / 数据流零改,仅换 UI 渲染层 (JSX + className)

## 关键取舍

1. **8 域并行** — 各域文件不重叠 (grep 确认 import 不交叉),可全并行 claim。共享文件 (utils/themes/App.tsx) 已在 infra/primitives 锁定,迁移只消费。
2. **迁移即清理** — 每域迁完删旧自定义 CSS 类 (globals.css/popover.css 对应段),不留死代码。
3. **icons 渐进换** — 域内 caller 迁移时把 `icons.tsx` 自研图标换 lucide,不预批量换 (减少 diff 爆炸)。
4. **SortableList 保留 dnd-kit** — 拖拽逻辑不动,只换内部 button/视觉。
5. **视觉巡检靠手动** — 自动测试覆盖组件交互 (PlatformCard/BalanceBar 等已有测试),视觉一致性靠 yarn tauri dev 逐域巡。

## 可能性分支

- **若某域 diff 过大 (>500 行)**: 域内再拆 (如 settings 拆 settings-main + settings-editors),当前按域先跑。
- **若发现缺组件**: 快速 `npx shadcn@latest add <x>` 补 (primitives 已建环境,add 秒级),不阻塞。
- **若 Cargo/i18n 发现新 key 需求**: 补 8 locale (check-i18n 守门),当前假设文案不变。
