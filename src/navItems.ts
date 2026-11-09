import type { NavItem } from "./components/Sidebar";

export const BASE_NAV: NavItem[] = [
  // G3 侧栏：概览第一位 / 监控 = 平台（分组内嵌 GroupsEmbedded，同页展示分组段 + 平台行）/ 日志。
  { id: "home", icon: "home", labelKey: "nav.home", section: "nav.section.overview" },
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms", section: "nav.section.monitoring" },
  { id: "stats", icon: "stats", labelKey: "nav.stats", section: "nav.section.logStats" },
  { id: "logs", icon: "logs", labelKey: "nav.logs", section: "nav.section.logStats" },
  { id: "request-log", icon: "logs", labelKey: "nav.requestLog", section: "nav.section.logStats" },
  { id: "mitm-log", icon: "logs", labelKey: "nav.mitmLog", section: "nav.section.logStats" },
  { id: "tool-log", icon: "logs", labelKey: "nav.toolLog", section: "nav.section.logStats" },
  { id: "notifications", icon: "notifications", labelKey: "nav.notifications", section: "nav.section.logStats" },
  { id: "skills", icon: "skills", labelKey: "nav.skills", section: "nav.section.extension" },
  { id: "mcp", icon: "mcp", labelKey: "nav.mcp", section: "nav.section.extension" },
  { id: "tools", icon: "tools", labelKey: "nav.tools", section: "nav.section.extension" },
  { id: "middleware", icon: "middleware", labelKey: "appSettings.middlewareTab", section: "nav.section.extension" },
  {
    id: "settings",
    icon: "settings",
    labelKey: "nav.settings",
    section: "nav.section.system",
    children: [
      { id: "settings/system", labelKey: "appSettings.systemTab", group: "nav.settingsGroup.general" },
      { id: "settings/api", labelKey: "appSettings.apiTab", group: "nav.settingsGroup.general" },
      { id: "settings/coding_tools", labelKey: "appSettings.cliIntegrationTab", group: "nav.settingsGroup.integration" },
      { id: "settings/claude", labelKey: "appSettings.claudeTab", group: "nav.settingsGroup.integration" },
      { id: "settings/codex", labelKey: "appSettings.codexTab", group: "nav.settingsGroup.integration" },
      { id: "settings/pi", labelKey: "appSettings.piTab", group: "nav.settingsGroup.integration" },
      { id: "settings/scheduling", labelKey: "appSettings.schedulingTab", group: "nav.settingsGroup.rules" },
      { id: "settings/notifications", labelKey: "appSettings.notificationsTab", group: "nav.settingsGroup.notification" },
      { id: "settings/pricing", labelKey: "appSettings.modelInfoTab", group: "nav.settingsGroup.config" },
      { id: "settings/tray", labelKey: "appSettings.trayTab", group: "nav.settingsGroup.config" },
      { id: "settings/popover", labelKey: "appSettings.popoverTab", group: "nav.settingsGroup.config" },
      { id: "settings/importexport", labelKey: "appSettings.importExportTab", group: "nav.settingsGroup.config" },
      { id: "settings/mitm", labelKey: "appSettings.mitmTab", group: "nav.settingsGroup.config" },
    ],
  },
  { id: "about", icon: "about", labelKey: "nav.about", section: "nav.section.system" },
];

// 持久化的导航 id 须在清单内（含 settings 子页）。
export const NAV_IDS = BASE_NAV.flatMap((n) => [n.id, ...(n.children?.map((c) => c.id) ?? [])]);


// 旧持久化 id → 新 id。
export const NAV_ALIASES: Record<string, string> = { "settings/middleware": "middleware" };
