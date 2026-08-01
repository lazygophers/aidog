import { useState, useEffect, useTransition, lazy, Suspense } from "react";
import { listen } from "@tauri-apps/api/event";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { Sidebar, type NavItem, type NavContext } from "./components/Sidebar";
import type { Tab } from "./pages/AppSettings";
import { UpdatePromptModal } from "./components/UpdatePromptModal";
import i18n from "./locales";

// ponytail: 每个侧栏页单独 chunk（React.lazy + 具名导出适配）。
// 首屏只加载 effectiveNav 对应的一个 chunk；切页由 startTransition
// 包裹 setActiveNav 驱动 —— Suspense 边界（<main> 内）跨 key 切换保留,
// 挂起时 React 保留旧树在屏上直到新 chunk resolve, 不掉进 fallback, 无闪烁。
const Home = lazy(() => import("./pages/Home").then(m => ({ default: m.Home })));
const Platforms = lazy(() => import("./pages/Platforms").then(m => ({ default: m.Platforms })));
const AppSettings = lazy(() => import("./pages/AppSettings").then(m => ({ default: m.AppSettings })));
const Logs = lazy(() => import("./pages/Logs").then(m => ({ default: m.Logs })));
const Stats = lazy(() => import("./pages/Stats").then(m => ({ default: m.Stats })));
const Notifications = lazy(() => import("./pages/Notifications").then(m => ({ default: m.Notifications })));
const Skills = lazy(() => import("./pages/Skills").then(m => ({ default: m.Skills })));
const Mcp = lazy(() => import("./pages/Mcp").then(m => ({ default: m.Mcp })));
const CliProxy = lazy(() => import("./pages/CliProxy").then(m => ({ default: m.CliProxy })));
const RequestLog = lazy(() => import("./pages/RequestLog").then(m => ({ default: m.RequestLog })));
const About = lazy(() => import("./pages/About").then(m => ({ default: m.About })));
import {
  proxyLogApi,
  notificationApi,
  NOTIF_SPEAK,
  autoUpdateApi,
} from "./services/api";
import { checkForUpdateDailyThrottled } from "./services/updater";
import type { Update } from "@tauri-apps/plugin-updater";
import { requestNavigation } from "./utils/navGuard";

const BASE_NAV: NavItem[] = [
  { id: "home", icon: "home", labelKey: "nav.home", section: "nav.section.overview" },
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms", section: "nav.section.platform" },
  { id: "cli-proxy", icon: "proxy", labelKey: "nav.cliProxy", section: "nav.section.platform" },
  { id: "stats", icon: "stats", labelKey: "nav.stats", section: "nav.section.logStats" },
  { id: "logs", icon: "logs", labelKey: "nav.logs", section: "nav.section.logStats" },
  { id: "request-log", icon: "logs", labelKey: "nav.requestLog", section: "nav.section.logStats" },
  { id: "notifications", icon: "notifications", labelKey: "nav.notifications", section: "nav.section.logStats" },
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
      { id: "settings/mitm", labelKey: "appSettings.mitmTab", group: "nav.settingsGroup.config" },
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
  const [, startTransition] = useTransition();

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
  // 设置开关关闭 → 跳过启动自动检查（手动按钮仍可查）。
  useEffect(() => {
    autoUpdateApi.get()
      .then((enabled) => { if (!enabled) return null; return checkForUpdateDailyThrottled(); })
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
        if (entity === "mcp") setActiveNav("mcp");
        if (entity === "skill") setActiveNav("skills");
      },
    );
    return () => { unlistenPromise.then((un) => un()).catch((e) => console.error(e)); };
  }, []);

  // 无前端窗口路径启动失败（自启动 / 托盘点启动，proxy-port-no-drift s3）：
  // Rust 侧 app_setup.rs 两处 emit "proxy-start-failed"（kind/port，与手动启动错误条
  // 同 ProxyStartError 形状），这里转系统通知。文案走 i18n（同 ProxyStatusSection 用的
  // proxy.startFailedPortInUse / proxy.startFailedOther key），Rust 侧不硬编码文案。
  useEffect(() => {
    const unlistenPromise = listen<{ kind: "addr_in_use" | "other"; port: number }>(
      "proxy-start-failed",
      async (e) => {
        const { kind, port } = e.payload;
        const body = kind === "addr_in_use"
          ? i18n.t("proxy.startFailedPortInUse", { port })
          : i18n.t("proxy.startFailedOther", { port });
        try {
          let granted = await isPermissionGranted();
          if (!granted) granted = (await requestPermission()) === "granted";
          if (granted) sendNotification({ title: i18n.t("app.title"), body });
        } catch (err) {
          console.error("system notification failed", err);
        }
      },
    );
    return () => { unlistenPromise.then((un) => un()).catch((e) => console.error(e)); };
  }, []);

  const handleNavigate = (id: string, context?: NavContext) => {
    if (id === activeNav && !context) return;
    // A dirty page (e.g. Claude Code Settings) may intercept the switch.
    requestNavigation(() => {
      // startTransition: 目标页 chunk 未加载完时, React 保留当前已提交的树
      // 在屏上, 不掉入 Suspense fallback —— 避免懒加载切页闪烁（红线 3）。
      startTransition(() => {
        setActiveNav(id);
        setNavContext(context ?? {});
      });
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

  // 保留子页后缀（如 "settings/claude"），Sidebar 靠它高亮二级菜单项。
  const resolvedNav =
    activeNav === "logs" && !logEnabled ? "platforms"
    : activeNav === "notifications" && !notifEnabled ? "platforms"
    : activeNav;
  const effectiveNav = resolvedNav.split("/")[0];
  // settings 子页：activeNav 形如 "settings/system"；裸 "settings" 回退 system。
  const settingsTab: Tab = activeNav.startsWith("settings/") ? (activeNav.slice(9) as Tab) : "system";

  return (
    <div style={{
      display: "flex",
      height: "100%",
      width: "100%",
      padding: 12,
      gap: 12,
      // 萤火虫：主壳玻璃衔接 — 侧栏与 main 间一条极淡的 primary 分隔光晕
      backgroundImage: "linear-gradient(90deg, transparent 200px, color-mix(in srgb, var(--primary) 8%, transparent) 212px, transparent 224px)",
      backgroundRepeat: "no-repeat",
    }}>
      <Sidebar
        navItems={navItems}
        activeId={resolvedNav}
        onNavigate={handleNavigate}
      />
      <main style={{
        flex: 1,
        overflow: "auto",
        padding: "24px 32px",
        // 萤火虫：main 区域玻璃卡面感（轻微 bg-surface 叠层）
        borderRadius: "var(--radius-lg)",
      }}>
        {/* fallback=null：startTransition 已保证挂起时旧树留屏, fallback 正常路径不会被看到；
            仅冷启动首帧（无旧树可留）时短暂命中，此时页面本就空白，null 与骨架视觉等价。 */}
        <Suspense fallback={null}>
          <div className="animate-fade-in" key={effectiveNav}>
            {effectiveNav === "home" && <Home onNavigate={handleNavigate} />}
            {effectiveNav === "platforms" && <Platforms onNavigate={handleNavigate} initialFilter={navContext} />}
            {effectiveNav === "cli-proxy" && <CliProxy />}
            {effectiveNav === "request-log" && <RequestLog />}
            {effectiveNav === "settings" && <AppSettings tab={settingsTab} onLogSettingsChanged={(enabled) => setLogEnabled(enabled)} onNotifSettingsChanged={(enabled) => setNotifEnabled(enabled)} />}
            {effectiveNav === "logs" && <Logs initialFilter={navContext} />}
            {effectiveNav === "stats" && <Stats initialFilter={navContext} />}
            {effectiveNav === "notifications" && <Notifications onNavigate={handleNavigate} />}
            {effectiveNav === "skills" && <Skills />}
            {effectiveNav === "mcp" && <Mcp />}
            {effectiveNav === "about" && <About />}
          </div>
        </Suspense>
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
