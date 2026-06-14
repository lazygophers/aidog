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
import {
  proxyLogApi,
  notificationApi,
  NOTIF_INBOX_UPDATED,
  NOTIF_SPEAK,
} from "./services/api";
import { requestNavigation } from "./utils/navGuard";

const BASE_NAV: NavItem[] = [
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms" },
  { id: "groups", icon: "groups", labelKey: "nav.groups" },
  { id: "stats", icon: "stats", labelKey: "nav.stats" },
  { id: "logs", icon: "logs", labelKey: "nav.logs" },
  { id: "notifications", icon: "notifications", labelKey: "nav.notifications" },
  { id: "skills", icon: "skills", labelKey: "nav.skills" },
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
    ],
  },
];

function App() {
  const [activeNav, setActiveNav] = useState("platforms");
  const [logEnabled, setLogEnabled] = useState(false);
  const [unread, setUnread] = useState(0);

  useEffect(() => {
    proxyLogApi.getSettings()
      .then(s => setLogEnabled(s.enabled))
      .catch(() => {});
  }, []);

  // 收件箱未读计数：初始拉取 + listen 实时刷新。
  useEffect(() => {
    const refreshUnread = () => {
      notificationApi.unreadCount()
        .then(setUnread)
        .catch(() => {});
    };
    refreshUnread();
    const unlistenPromise = listen(NOTIF_INBOX_UPDATED, () => { refreshUnread(); });
    return () => { unlistenPromise.then((un) => un()).catch((e) => console.error(e)); };
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

  // 隐藏 logs 菜单（日志关闭时）
  const navItems = (logEnabled
    ? BASE_NAV
    : BASE_NAV.filter(n => n.id !== "logs")
  ).map(n => n.id === "notifications" ? { ...n, badge: unread } : n);

  const effectiveNav = activeNav === "logs" && !logEnabled ? "platforms" : activeNav.split("/")[0];
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
          {effectiveNav === "settings" && <AppSettings tab={settingsTab} onLogSettingsChanged={(enabled) => setLogEnabled(enabled)} />}
          {effectiveNav === "logs" && <Logs />}
          {effectiveNav === "stats" && <Stats />}
          {effectiveNav === "notifications" && <Notifications />}
          {effectiveNav === "skills" && <Skills />}
        </div>
      </main>
    </div>
  );
}

export default App;
