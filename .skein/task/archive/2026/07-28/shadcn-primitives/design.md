# shadcn 核心 primitives 落地 — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json):

## 前置

依赖 shadcn-infra 完成 (components.json + Tailwind v4 + cn() + token 体系就绪)。

## 组件清单 (按现有原生元素用法反推)

现有用法 (grep src/): 445 `<button>` + 169 `<input>` + 68 `<select>` + 19 `<textarea>`,外加自定义 modal/dropdown/tab/toast。

| 类别 | 组件 | 现有消费点 |
|---|---|---|
| 表单基础 | `button` `input` `textarea` `label` `select` | 全局, 各 page + components/ |
| 表单高级 | `checkbox` `switch` `slider` `radio-group` `toggle-group` `combobox` | Settings 编辑器, platform form |
| 表单容器 | `field` `field-group` (shadcn Form 原语) | 替代自定义 div+label 布局 |
| 反馈 | `sonner` (toast) `alert` `alert-dialog` `progress` `skeleton` `spinner` | 替代自定义 toast / confirm / loading |
| 覆盖层 | `dialog` `sheet` `drawer` `popover` `tooltip` | 替代 components/shared/Modal.tsx + 各 *Modal.tsx |
| 导航 | `tabs` `breadcrumb` `pagination` | Settings tab, 列表分页 |
| 数据展示 | `card` `badge` `avatar` `separator` `table` `scroll-area` `collapsible` `accordion` | shared/CompactCard, StatChip, 分割线, 折叠区 |
| 菜单 | `dropdown-menu` `context-menu` `menubar` | 右键菜单, 操作下拉 |
| 命令面板 | `command` (内嵌 Dialog) | Cmd+K 搜索 (若现有无则按需) |

清单按 YAGNI:只 add 现有明确需要的。新增需求走后续 task。

## add 命令

```bash
npx shadcn@latest add button input textarea label select checkbox switch slider radio-group toggle-group field combobox
npx shadcn@latest add dialog sheet drawer popover tooltip alert-dialog
npx shadcn@latest add tabs breadcrumb pagination card badge avatar separator table scroll-area collapsapsible accordion
npx shadcn@latest add dropdown-menu context-menu command sonner alert progress skeleton spinner
```

源码落 `resolvedPaths.ui` (= `src/components/ui/`,components.json 配置)。

## wrapper 策略

shadcn 组件源码进 `src/components/ui/` (CLI 生成,不手改 — 便于后续 `add --diff` 升级)。项目级 wrapper 进 `src/components/shared/`:

- **i18n 注入点**: Dialog/AlertDialog 默认 close 按钮 aria-label 等固定文案,wrapper 接 i18n key 或 props 注入 (非必须,shadcn 组件本身不强求文案)。
- **Liquid Glass 变体**: 需玻璃效果的组件 (Card/Dialog/Sheet) wrapper 叠 `backdrop-blur` + `--glass-*` 变量 className,不污染 ui/ 源码。
- **modal Portal**: shadcn Dialog/Sheet 内置 Radix Portal (天然 createPortal 到 body),满足 memory `modal-window-center-rule` (祖先 transform/backdrop-filter 不影响)。**删旧 components/shared/Modal.tsx 自研 Portal 逻辑**,统一走 shadcn。
- **Button loading**: 无 `isLoading` prop (shadcn 规则),用 `<Button disabled><Spinner data-icon/>保存</Button>` 组合。若高频重复,wrapper 封 `<LoadingButton>` 糖。

## 关键取舍

1. **base/radix 按 nova preset** — init 时定,不强行切。radix 生态稳,asChild 模式熟。
2. **iconLibrary = lucide-react** (nova 默认) — 现有 `src/components/icons.tsx` 自研图标不一次性全换,shadcn-pages 按需替换 (新组件用 lucide,旧图标保留到迁移其 caller 时换)。
3. **不 add 未用组件** — YAGNI。command/menubar/drawer 等若现有无消费点,不预装,pages 迁移发现需要再补 (走快速 add)。
4. **保留非 shadcn 依赖** — `@dnd-kit/*` (SortableList)、`qrcode`、`react-markdown`、`pinyin-pro` 不动,它们不冲突。

## 可能性分支

- **若需 Chart**: shadcn `chart` (包 Recharts),Stats/PricingTab 图表若迁时再加。
- **若需 DataTable**: 现有 Logs/Stats 表格若复杂度涨,加 `data-table` (基于 table + tanstack),当前手写够用。
- **若需 shadcn Sidebar 组件**: 现有自研 Sidebar.tsx (20.9K) 功能足够,不换 shadcn sidebar (锁定了自定义导航逻辑),只把内部按钮/菜单换 shadcn primitives。
