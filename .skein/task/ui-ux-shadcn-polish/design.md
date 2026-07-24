# UI/UX shadcn 全量适配 — 详细设计

## 分片策略：按页族切，各 subtask 独占文件集，零跨族冲突

审计 93 处按 5 页族切分。每族独占其文件，无共享写入点 → 5 subtask 全并行无依赖。

**关键前提（已核验）**：
- 色 token 全部已存在（`--color-success/warning/danger` / `--accent-foreground` / `--border`，globals.css）→ 无独立基建 subtask。
- 共享 `Toggle`（`editors/_shared.tsx:16`）仅 Settings 族内引用 → 归 subtask C 独占，改底层自动迁 ~11 caller。
- `globals.css:396 .toggle` CSS 只读保留（全迁完变死代码，本轮不删，YAGNI）。

## 5 subtask（各含全 7 批中该族命中项）

- **ui-A** Home/Stats/About/shared — 批1(Home danger)/批5(hex兜底)/批7(Stats table)/批3(FilterDropdown,CopyButton)
- **ui-B** platforms/* — 批1(formSectionsEndpoints,PlatformEditForm)/批2(formSections toggle)/批3(ModelsMatrix)/批4(checkbox)/批5/批6(MultiKeyPreview)
- **ui-C** Groups/* + components/settings/* — 批2(共享Toggle+raw toggle)/批4(SettingsHeader,GroupCreateModal)/批5
- **ui-D** Logs/CliProxy/AppSettings/ModelTestPanel/CodexSettings — 批1(padding)/批2(AppSettings toggle a11y)/批3(ModelTestPanel modal)/批4(CodexSettings)/批5/批6(CliProxy grid)
- **ui-E** Skills/Tray/Popover/Pricing — 批2(toggle)/批3(TrayConfigTab popup)/批5(color:#fff)/批6(SkillsView wrap)/批7(PricingTab table)

## 统一约束（写进每个 executor prompt）
- 色只用现有 token：`var(--color-danger/success/warning)` / `var(--accent-foreground)` / `var(--border)`；禁新增 token、禁 `var(--x,#hex)` 兜底。
- 非法 alpha 拼接（`var(--x)15`/`var(--x)20`）→ `color-mix(in srgb, var(--x) N%, transparent)`。
- modal/popup 必 shadcn Dialog/Popover + createPortal(document.body)（memory modal-window-center-rule）。
- TrayConfigTab macOS 菜单栏模拟色刻意保留，仅加注释禁改。
- 门禁：`yarn build` + `yarn test` + `scripts/check-i18n.mjs` 各族改完自验。
