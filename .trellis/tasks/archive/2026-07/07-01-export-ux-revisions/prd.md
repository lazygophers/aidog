# PRD — 导出 UX 修订: setting label 补全 + 菜单组拆分

> 用户请求（/trellisx-flow 两条合并）：
> ① `global:cc_codex_settings` 还是存在未适配的多语言
> ② 平台的导出应该区分分组、平台、平台分组关系三个，而不是一溜子平铺
> 排队：撞 07-01-export-default-check（同 ImportExport.tsx），等其 finish 后 start。

## 目标
两处导出/导入 preview 的 UX 修订：setting 裸 key 补 i18n label + 平台类三 scope 拆子类分开呈现。

## 现状（main 调研）
- `SETTING_KEY_LABEL`（ImportExport.tsx:110-129）已有 22 条，含 `global:coding_tools_settings`
- **缺口**：`global:cc_codex_settings`（旧名，schema_late.rs:246 migration 迁到 coding_tools_settings）— 老用户 DB 残留未迁时导出仍出该裸 key，表无兜底 → 裸 key 展示
- 菜单组（L67-86）：当前 platform + group + group_platform **合并到一个"平台"菜单组**平铺（SCOPE_MENU_GROUP L84-86 三者都 map "platform"）

## 交付项

### D1 — setting label 补全（R1）
- audit `SETTING_KEY_LABEL` 全表 vs 后端实际 (scope,key)（grep `src-tauri set_setting/get_setting` 全调用点 + migration 残留旧名）
- 补缺：至少 `global:cc_codex_settings`（旧名兜底）+ audit 发现的其他裸 key
- 8 locale 补对应 `importExport.settingLabel.*` key（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）
- 验收：导出 preview 无裸 key（`xxx:yyy` 形式不出现给用户），全部本地化

### D2 — 菜单组拆分（R2）
- 当前 platform/group/group_platform 合并一菜单组 → 拆为**三子类分开呈现**
- 改 SCOPE_MENU_GROUP 或菜单组渲染逻辑：三 scope 各自子分组（"平台"/"分组"/"分组↔平台关联"）而非一"平台"组内平铺
- 保持条目级勾选语义不变（D2 只改视觉分组，不改勾选/导入逻辑）
- 验收：导出/导入 preview 三类分开显示，不再一溜子平铺

## 验收
1. 导出 preview 无裸 setting key（cc_codex_settings 等全本地化）
2. 8 locale 全覆盖（check-i18n 绿）
3. platform/group/group_platform 三子类分开呈现
4. 条目勾选/导入逻辑零回归
5. `yarn build` + tsc 0 error

## 非目标
- 不改后端 build_items（前端展示层映射）
- 不改 migration（DB 层迁移另一回事，本 task 只兜底展示）
- 不改勾选/导入逻辑（D2 仅视觉）

## 风险
- audit 可能发现多个裸 key（补面大）→ 按 export 高频可见性排序补
- 菜单组拆分改渲染逻辑 → 注意 collapsible 状态（scopeCardGroups L446）

## 排队
撞 07-01-export-default-check（同 ImportExport.tsx）。等其 finish 后 start。
