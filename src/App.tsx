import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { Sidebar, type NavItem } from "./components/Sidebar";
import { Platforms } from "./pages/Platforms";
import { Groups } from "./pages/Groups";
import { AppSettings, type Tab } from "./pages/AppSettings";
import { Logs } from "./pages/Logs";
import { Stats } from "./pages/Stats";
import { Notifications } from "./pages/Notifications";
import { Skills } from "./pages/Skills";
import { Mcp } from "./pages/Mcp";
import { About } from "./pages/About";
import { UpdatePromptModal } from "./components/UpdatePromptModal";
import {
  proxyLogApi,
  notificationApi,
  NOTIF_SPEAK,
} from "./services/api";
import { checkForUpdateDailyThrottled } from "./services/updater";
import type { Update } from "@tauri-apps/plugin-updater";
import { requestNavigation } from "./utils/navGuard";

const BASE_NAV: NavItem[] = [
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms" },
  { id: "groups", icon: "groups", labelKey: "nav.groups" },
  { id: "stats", icon: "stats", labelKey: "nav.stats" },
  { id: "logs", icon: "logs", labelKey: "nav.logs" },
  { id: "notifications", icon: "notifications", labelKey: "nav.notifications" },
  { id: "skills", icon: "skills", labelKey: "nav.skills" },
  { id: "mcp", icon: "mcp", labelKey: "nav.mcp" },
  {
    id: "settings",
    icon: "settings",
    labelKey: "nav.settings",
    children: [
      { id: "settings/system", labelKey: "appSettings.systemTab", group: "nav.settingsGroup.general" },
      { id: "settings/claude", labelKey: "appSettings.claudeTab", group: "nav.settingsGroup.integration" },
      { id: "settings/codex", labelKey: "appSettings.codexTab", group: "nav.settingsGroup.integration" },
      { id: "settings/middleware", labelKey: "appSettings.middlewareTab", group: "nav.settingsGroup.rules" },
      { id: "settings/scheduling", labelKey: "appSettings.schedulingTab", group: "nav.settingsGroup.rules" },
      { id: "settings/notifications", labelKey: "appSettings.notificationsTab", group: "nav.settingsGroup.notification" },
      { id: "settings/pricing", labelKey: "appSettings.pricingTab", group: "nav.settingsGroup.config" },
      { id: "settings/tray", labelKey: "appSettings.trayTab", group: "nav.settingsGroup.config" },
      { id: "settings/popover", labelKey: "appSettings.popoverTab", group: "nav.settingsGroup.config" },
      { id: "settings/importexport", labelKey: "appSettings.importExportTab", group: "nav.settingsGroup.config" },
    ],
  },
  { id: "about", icon: "about", labelKey: "nav.about" },
];

function App() {
  const [activeNav, setActiveNav] = useState("platforms");
  const [logEnabled, setLogEnabled] = useState(false);
  const [notifEnabled, setNotifEnabled] = useState(true);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);

  useEffect(() => {
    proxyLogApi.getSettings()
      .then(s => setLogEnabled(s.enabled))
      .catch(() => {});
  }, []);

  // 通知总开关 off 时隐藏「通知中心」侧栏入口（仿 logs 隐藏模式）。
  useEffect(() => {
    notificationApi.getSettings()
      .then(s => setNotifEnabled(s.enabled))
      .catch(() => {});
  }, []);

  // WebSpeech 播报：tts_backend=web_speech 时后端 emit NOTIF_SPEAK（payload=文本），前端朗读。
  // 全局挂载（与当前页无关），保证任意页都能播报。
  useEffect(() => {
    const speak = (text: string) => {
      if (!text || typeof window === "undefined" || !window.speechSynthesis) return;
      try {
        const u = new SpeechSynthesisUtterance(text);
        window.speechSynthesis.speak(u);
      } catch (e) {
        console.error("web speech failed", e);
      }
    };
    const unlistenPromise = listen<string>(NOTIF_SPEAK, (e) => { speak(e.payload); });
    return () => { unlistenPromise.then((un) => un()).catch((e) => console.error(e)); };
  }, []);

  // 每日检测更新：启动调节流检查 (24h)，有更新弹自定义提醒 modal。
  // dev/未签名/无网络失败已在 service 内 catch 静默，不打扰。
  useEffect(() => {
    checkForUpdateDailyThrottled()
      .then((upd) => { if (upd) setPendingUpdate(upd); })
      .catch(() => {});
  }, []);

  const handleNavigate = (id: string) => {
    if (id === activeNav) return;
    // A dirty page (e.g. Claude Code Settings) may intercept the switch.
    requestNavigation(() => {
      setActiveNav(id);
      if (id === "logs") {
        proxyLogApi.getSettings()
          .then(s => setLogEnabled(s.enabled))
          .catch(() => {});
      }
    });
  };

  // 隐藏菜单：日志关闭去 logs；通知关闭去 notifications。
  const navItems = BASE_NAV.filter(n => {
    if (!logEnabled && n.id === "logs") return false;
    if (!notifEnabled && n.id === "notifications") return false;
    return true;
  });

  const effectiveNav =
    activeNav === "logs" && !logEnabled ? "platforms"
    : activeNav === "notifications" && !notifEnabled ? "platforms"
    : activeNav.split("/")[0];
  // settings 子页：activeNav 形如 "settings/system"；裸 "settings" 回退 system。
  const settingsTab: Tab = activeNav.startsWith("settings/") ? (activeNav.slice(9) as Tab) : "system";

  return (
    <div style={{
      display: "flex",
      height: "100%",
      width: "100%",
      padding: 12,
      gap: 12,
    }}>
      <Sidebar
        navItems={navItems}
        activeId={effectiveNav}
        onNavigate={handleNavigate}
      />
      <main style={{
        flex: 1,
        overflow: "auto",
        padding: "24px 32px",
      }}>
        <div className="animate-fade-in" key={effectiveNav}>
          {effectiveNav === "platforms" && <Platforms />}
          {effectiveNav === "groups" && <Groups />}
          {effectiveNav === "settings" && <AppSettings tab={settingsTab} onLogSettingsChanged={(enabled) => setLogEnabled(enabled)} onNotifSettingsChanged={(enabled) => setNotifEnabled(enabled)} />}
          {effectiveNav === "logs" && <Logs />}
          {effectiveNav === "stats" && <Stats />}
          {effectiveNav === "notifications" && <Notifications onNavigate={handleNavigate} />}
          {effectiveNav === "skills" && <Skills />}
        {effectiveNav === "mcp" && <Mcp />}
          {effectiveNav === "about" && <About />}
        </div>
      </main>
      {pendingUpdate && (
        <UpdatePromptModal
          update={pendingUpdate}
          onClose={() => setPendingUpdate(null)}
        />
      )}
    </div>
  );
}

export default App;
