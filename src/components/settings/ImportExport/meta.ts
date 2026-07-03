// 导入导出子系统的 scope / 菜单元数据 + 派生 helper（纯数据 + 纯函数，零 JSX）。
// 抽自原 ImportExport.tsx L37-163，行为零变更。

import type { TFunction } from "i18next";
import type { ImportExportScope, ImportItem } from "../../../services/api";

// scope 元数据：id + i18n labelKey + 默认 label + 映射图标（PRD scope→icon）+ 一行描述。
export const ALL_SCOPES: {
  id: ImportExportScope;
  labelKey: string;
  defaultLabel: string;
  icon: string;
  descKey: string;
  defaultDesc: string;
}[] = [
  { id: "platform", labelKey: "importExport.scope.platform", defaultLabel: "平台", icon: "network", descKey: "importExport.scopeDesc.platform", defaultDesc: "平台连接与凭据" },
  { id: "group", labelKey: "importExport.scope.group", defaultLabel: "分组", icon: "team", descKey: "importExport.scopeDesc.group", defaultDesc: "分组与调度策略" },
  { id: "group_platform", labelKey: "importExport.scope.groupPlatform", defaultLabel: "分组↔平台关联", icon: "worktree", descKey: "importExport.scopeDesc.groupPlatform", defaultDesc: "分组与平台的关联关系" },
  { id: "setting", labelKey: "importExport.scope.setting", defaultLabel: "全局设置", icon: "bolt", descKey: "importExport.scopeDesc.setting", defaultDesc: "主题 / 语言 / 代理 / 通知等" },
  { id: "codex", labelKey: "importExport.scope.codex", defaultLabel: "Codex 设置", icon: "file", descKey: "importExport.scopeDesc.codex", defaultDesc: "Codex 配置" },
  { id: "claude_code", labelKey: "importExport.scope.claudeCode", defaultLabel: "Claude Code 设置", icon: "memory", descKey: "importExport.scopeDesc.claudeCode", defaultDesc: "Claude Code 配置" },
  { id: "model_price", labelKey: "importExport.scope.modelPrice", defaultLabel: "模型价格", icon: "pricing", descKey: "importExport.scopeDesc.modelPrice", defaultDesc: "自定义模型定价" },
  { id: "mcp", labelKey: "importExport.scope.mcp", defaultLabel: "MCP 服务器", icon: "mcp", descKey: "importExport.scopeDesc.mcp", defaultDesc: "MCP 服务器配置 + 启用状态" },
  { id: "middleware", labelKey: "importExport.scope.middleware", defaultLabel: "中间件规则", icon: "rules", descKey: "importExport.scopeDesc.middleware", defaultDesc: "请求/响应中间件规则" },
  { id: "skills", labelKey: "importExport.scope.skills", defaultLabel: "Skills", icon: "plugins", descKey: "importExport.scopeDesc.skills", defaultDesc: "npx 安装 + 启用状态" },
];

export const SCOPE_ICON: Record<string, string> = Object.fromEntries(ALL_SCOPES.map((s) => [s.id, s.icon]));

export function scopeLabel(t: TFunction, scope: string): string {
  const entry = ALL_SCOPES.find((s) => s.id === scope);
  if (!entry) return scope;
  return t(entry.labelKey, entry.defaultLabel);
}

// ── IA 菜单分组（按侧栏菜单组织）──
// 导出/导入条目不再按裸 scope 平铺，而是按菜单组聚合呈现：
//   - 平台模块三 scope 各自独立子分组（platform / group / group_platform 分开，不一组平铺）
//   - setting scope 的条目按其 key 前缀（settingScope）二次归类到对应菜单组
//     （key 形如 `<settingScope>:<settingKey>`，见后端 build_items setting 约定）
export type MenuGroupId =
  | "platform" | "group" | "group_platform"
  | "extension" | "rules" | "scheduling" | "uiPref" | "system";

export const MENU_GROUPS: { id: MenuGroupId; labelKey: string; defaultLabel: string; icon: string }[] = [
  { id: "platform", labelKey: "importExport.menuGroup.platform", defaultLabel: "平台", icon: "network" },
  { id: "group", labelKey: "importExport.menuGroup.group", defaultLabel: "分组", icon: "team" },
  { id: "group_platform", labelKey: "importExport.menuGroup.groupPlatform", defaultLabel: "分组↔平台关联", icon: "worktree" },
  { id: "extension", labelKey: "importExport.menuGroup.extension", defaultLabel: "扩展", icon: "plugins" },
  { id: "rules", labelKey: "importExport.menuGroup.rules", defaultLabel: "规则", icon: "rules" },
  { id: "scheduling", labelKey: "importExport.menuGroup.scheduling", defaultLabel: "调度", icon: "worktree" },
  { id: "uiPref", labelKey: "importExport.menuGroup.uiPref", defaultLabel: "界面偏好", icon: "bolt" },
  { id: "system", labelKey: "importExport.menuGroup.system", defaultLabel: "系统", icon: "settings" },
];

// scope → 菜单组（platform/group/group_platform 三 scope 各自独立子分组，避免一菜单组平铺；
// setting 例外：按 settingScope 子归类，见 SETTING_SCOPE_GROUP）。
export const SCOPE_MENU_GROUP: Record<string, MenuGroupId> = {
  platform: "platform",
  group: "group",
  group_platform: "group_platform",
  skills: "extension",
  mcp: "extension",
  middleware: "rules",
  codex: "system",
  claude_code: "system",
  model_price: "system",
  setting: "system", // 兜底；细分见 SETTING_SCOPE_GROUP
};

// setting item 的 settingScope（key 前缀）→ 菜单组。未列出走 system（全局设置）。
export const SETTING_SCOPE_GROUP: Record<string, MenuGroupId> = {
  scheduling: "scheduling",
  tray: "uiPref",
  popover: "uiPref",
  notification: "uiPref",
};

// setting item 的 `<scope>:<key>` → i18n label key。
// 后端 build_items 对 setting scope 存裸 key（`app:theme` 等稳定标识），前端展示层做本地化映射；
// 未命中（新增/未知 setting key）→ 回退裸 key（不崩不空）。
// 全集来源：grep src-tauri set_setting/get_setting 各 (scope,key) 调用点。
export const SETTING_KEY_LABEL: Record<string, string> = {
  "app:theme": "importExport.settingLabel.app_theme",
  "app:locale": "importExport.settingLabel.app_locale",
  "app:logging": "importExport.settingLabel.app_logging",
  "app:script_executor": "importExport.settingLabel.app_script_executor",
  "proxy:settings": "importExport.settingLabel.proxy_settings",
  "proxy:proxy_client": "importExport.settingLabel.proxy_client",
  "proxy:timeout": "importExport.settingLabel.proxy_timeout",
  "proxy:logging": "importExport.settingLabel.proxy_logging",
  "notification:settings": "importExport.settingLabel.notification_settings",
  "middleware:settings": "importExport.settingLabel.middleware_settings",
  "scheduling:settings": "importExport.settingLabel.scheduling_settings",
  "stats:settings": "importExport.settingLabel.stats_settings",
  "stats:agg_rebuild_v1": "importExport.settingLabel.stats_agg_rebuild_v1",
  "stats:agg_count_tokens_excluded_v1": "importExport.settingLabel.stats_agg_count_tokens_excluded_v1",
  "pricing:sync": "importExport.settingLabel.pricing_sync",
  "tray:config": "importExport.settingLabel.tray_config",
  "popover:config": "importExport.settingLabel.popover_config",
  "global:claude_code": "importExport.settingLabel.global_claude_code",
  "global:coding_tools_settings": "importExport.settingLabel.global_coding_tools_settings",
  // ponytail: 旧 key，schema_late.rs migration 030 迁到 coding_tools_settings；
  // 老 DB 残留未迁时导出仍出此 key，补兜底 label 避免裸 key 展示（迁移另一回事，此处只兜底展示）。
  "global:cc_codex_settings": "importExport.settingLabel.global_cc_codex_settings",
  "backup:settings": "importExport.settingLabel.backup_settings",
  // ponytail: 内部迁移标记，迁移完成后必落库（db/maintenance.rs），导出高频可见，故补 label 而非裸 key 兜底。
  "db:compact_migrated_v1": "importExport.settingLabel.db_compact_migrated_v1",
};

// 取某 setting item 的本地化 label；非 setting scope 或未命中 → 返回 null（调用方回退原 label）。
export function settingLabelKey(item: ImportItem): string | null {
  if (item.scope !== "setting") return null;
  return SETTING_KEY_LABEL[item.key] ?? null;
}

// 取某 item 的菜单组 id。setting scope 按 key 前缀细分。
export function menuGroupOf(item: ImportItem): MenuGroupId {
  if (item.scope === "setting") {
    const settingScope = item.key.split(":")[0];
    return SETTING_SCOPE_GROUP[settingScope] ?? "system";
  }
  return SCOPE_MENU_GROUP[item.scope] ?? "system";
}

export function menuGroupLabel(t: TFunction, id: string): string {
  const g = MENU_GROUPS.find((x) => x.id === id);
  if (!g) return id;
  return t(g.labelKey, g.defaultLabel);
}

export const MENU_GROUP_ICON: Record<string, string> = Object.fromEntries(MENU_GROUPS.map((g) => [g.id, g.icon]));
