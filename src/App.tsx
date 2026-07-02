import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { Sidebar, type NavItem, type NavContext } from "./components/Sidebar";
import { Home } from "./pages/Home";
import { Platforms } from "./pages/Platforms";
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
  { id: "home", icon: "home", labelKey: "nav.home", section: "nav.section.overview" },
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms", section: "nav.section.proxy" },
  { id: "stats", icon: "stats", labelKey: "nav.stats", section: "nav.section.observe" },
  { id: "logs", icon: "logs", labelKey: "nav.logs", section: "nav.section.observe" },
  { id: "notifications", icon: "notifications", labelKey: "nav.notifications", section: "nav.section.observe" },
  { id: "skills", icon: "skills", labelKey: "nav.skills", section: "nav.section.extension" },
  { id: "mcp", icon: "mcp", labelKey: "nav.mcp", section: "nav.section.extension" },
  {
    id: "settings",
    icon: "settings",
    labelKey: "nav.settings",
    section: "nav.section.system",
    children: [
      { id: "settings/system", labelKey: "appSettings.systemTab", group: "nav.settingsGroup.general" },
      { id: "settings/coding_tools", labelKey: "appSettings.cliIntegrationTab", group: "nav.settingsGroup.integration" },
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
  { id: "about", icon: "about", labelKey: "nav.about", section: "nav.section.system" },
];

function App() {
  const [activeNav, setActiveNav] = useState("home");
  const [navContext, setNavContext] = useState<NavContext>({});
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

  // aidog:// deep link 协议层事件分发：后端 emit `aidog-deep-link` {entity, action, data}，
  // 这里按 entity 二次分发到 `aidog:<entity>` window CustomEvent，children（D2/D3/D4 的
  // Platforms/Mcp/Skills 页）各自 addEventListener 接入。D1 只做协议层 + 分发，不处理 import。
  //
  // D2 补：children 页面是条件挂载（effectiveNav === "platforms" 才挂 <Platforms>），
  // window CustomEvent 在目标页未 mount 时会丢失（冷启动 + 热唤起当前在他页两路都中招）。
  // 这里在 dispatch 同时把 payload 缓存到 window.__aidogDeepLink[entity]，目标页 mount 时
  // 取一次消费（删 key 防重复）；并对 platform entity 主动 setActiveNav("platforms") 触发挂载。
  // ponytail: per-entity 缓存对象（非队列），单条 last-write-wins，deep-link 不要求保序。
  useEffect(() => {
    const unlistenPromise = listen<{ entity: string; action: string; data: string }>(
      "aidog-deep-link",
      (e) => {
        const { entity, action, data } = e.payload;
        const w = window as unknown as { __aidogDeepLink?: Record<string, { action: string; data: string }> };
        if (!w.__aidogDeepLink) w.__aidogDeepLink = {};
        w.__aidogDeepLink[entity] = { action, data };
        window.dispatchEvent(new CustomEvent(`aidog:${entity}`, { detail: { action, data } }));
        if (entity === "platform") setActiveNav("platforms");
      },
    );
    return () => { unlistenPromise.then((un) => un()).catch((e) => console.error(e)); };
  }, []);

  const handleNavigate = (id: string, context?: NavContext) => {
    if (id === activeNav && !context) return;
    // A dirty page (e.g. Claude Code Settings) may intercept the switch.
    requestNavigation(() => {
      setActiveNav(id);
      setNavContext(context ?? {});
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
          {effectiveNav === "home" && <Home onNavigate={handleNavigate} />}
          {effectiveNav === "platforms" && <Platforms onNavigate={handleNavigate} initialFilter={navContext} />}
          {effectiveNav === "settings" && <AppSettings tab={settingsTab} onLogSettingsChanged={(enabled) => setLogEnabled(enabled)} onNotifSettingsChanged={(enabled) => setNotifEnabled(enabled)} />}
          {effectiveNav === "logs" && <Logs initialFilter={navContext} />}
          {effectiveNav === "stats" && <Stats initialFilter={navContext} />}
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
