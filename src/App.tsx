import { useState, useEffect, useTransition, lazy, Suspense } from "react";
import { listen } from "./services/transport";
import { notify, isTauri } from "./services/platform";
import { PathPickerHost } from "./components/shared/PathPickerHost";
import { ErrorBoundary } from "./components/shared/ErrorBoundary";
import { Sidebar, type NavItem, type NavContext } from "./components/Sidebar";
import { CommandPalette } from "./components/CommandPalette";
import { Topbar } from "./components/Topbar";
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
const RequestLog = lazy(() => import("./pages/RequestLog").then(m => ({ default: m.RequestLog })));
const MitmLog = lazy(() => import("./pages/MitmLog").then(m => ({ default: m.MitmLog })));
const About = lazy(() => import("./pages/About").then(m => ({ default: m.About })));
import {
  notificationApi,
  NOTIF_SPEAK,
  autoUpdateApi,
  onRegistryUpdated,
} from "./services/api";
import { invalidateDefaultsDoc } from "./domains/platforms/defaults";
import { checkForUpdateDailyThrottled } from "./services/updater";
import type { Update } from "@tauri-apps/plugin-updater";
import { requestNavigation } from "./utils/navGuard";

const BASE_NAV: NavItem[] = [
  // G3 侧栏：概览 ⌘1 第一位 / 监控 = 平台 ⌘2（分组内嵌 GroupsEmbedded，同页展示分组段 + 平台行）/ 日志 ⌘3。
  { id: "home", icon: "home", labelKey: "nav.home", section: "nav.section.overview", kbd: "⌘1" },
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms", section: "nav.section.monitoring", kbd: "⌘2" },
  { id: "stats", icon: "stats", labelKey: "nav.stats", section: "nav.section.logStats" },
  { id: "logs", icon: "logs", labelKey: "nav.logs", section: "nav.section.logStats", kbd: "⌘3" },
  { id: "request-log", icon: "logs", labelKey: "nav.requestLog", section: "nav.section.logStats" },
  { id: "mitm-log", icon: "logs", labelKey: "nav.mitmLog", section: "nav.section.logStats" },
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
      { id: "settings/pi", labelKey: "appSettings.piTab", group: "nav.settingsGroup.integration" },
      { id: "settings/middleware", labelKey: "appSettings.middlewareTab", group: "nav.settingsGroup.rules" },
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

function App() {
  const [activeNav, setActiveNav] = useState("home");
  const [navContext, setNavContext] = useState<NavContext>({});
  // G3 全局骨架：⌘K 命令面板 + ⌘1/2/3 页面快捷键（设计稿顶栏 kbd + 侧栏 kbd 标识）。
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [notifEnabled, setNotifEnabled] = useState(true);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [, startTransition] = useTransition();

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
  // 票 10：自动更新是桌面壳独有的（更新的是本机安装包）。浏览器形态下 `check()` 必然抛错，
  // 与其让它每次启动都失败一次，不如直接不查 —— 内核的升级由部署方管。
  useEffect(() => {
    if (!isTauri()) return;
    autoUpdateApi.get()
      .then((enabled) => { if (!enabled) return null; return checkForUpdateDailyThrottled(); })
      .then((upd) => { if (upd) setPendingUpdate(upd); })
      .catch(() => {});
  }, []);

  // 注册表远程同步落库有变更（Rust emit "registry-updated"）→ 失效 defaults docPromise，
  // 后续 getDefault* 调用重新走 RPC（消费点分散在表单默认值 / 模型候选列，不在此逐个刷新）。
  // 模型信息页自身的 snapshot 重拉由 ModelInfoTab 自己订阅同一事件。
  useEffect(() => onRegistryUpdated(() => { invalidateDefaultsDoc(); }), []);

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
        // 票 10：桌面走 Tauri 通知插件，浏览器走 `window.Notification`，都在 platform.notify 里。
        await notify(i18n.t("app.title"), body);
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
    });
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (e.key === "k" || e.key === "K") {
        e.preventDefault();
        setPaletteOpen(v => !v);
      } else if (e.key === "1") { e.preventDefault(); handleNavigate("home"); }
      else if (e.key === "2") { e.preventDefault(); handleNavigate("platforms"); }
      else if (e.key === "3") { e.preventDefault(); handleNavigate("logs"); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // handleNavigate 每渲染重建（闭包捕获 activeNav），effect 跟随重挂监听。
  }, [handleNavigate]);

  // 隐藏菜单：日志关闭去 logs；通知关闭去 notifications。
  const navItems = BASE_NAV.filter(n => {
    if (!notifEnabled && n.id === "notifications") return false;
    return true;
  });

  // 保留子页后缀（如 "settings/claude"），Sidebar 靠它高亮二级菜单项。
  const resolvedNav =
    activeNav === "notifications" && !notifEnabled ? "platforms"
    : activeNav;
  const effectiveNav = resolvedNav.split("/")[0];
  // settings 子页：activeNav 形如 "settings/system"；裸 "settings" 回退 system。
  const settingsTab: Tab = activeNav.startsWith("settings/") ? (activeNav.slice(9) as Tab) : "system";

  // 面包屑：section 标签 / 页标签（从 BASE_NAV 解析；settings 子页用子页标签）。
  const t0 = (k: string) => i18n.t(k);
  const crumb = (() => {
    const item = BASE_NAV.find(n => n.id === resolvedNav.split("/")[0]);
    if (!item) return ["", ""] as [string, string];
    const child = item.children?.find(c => c.id === resolvedNav);
    return [t0(item.section ?? ""), t0(child ? child.labelKey : item.labelKey)] as [string, string];
  })();
  // G3 画布：大画布页（统计/日志/通知/首页）带点阵网格，表单页（settings 等）不带。
  const canvasPages = new Set(["home", "stats", "logs", "notifications"]);

  return (
    <div style={{
      display: "flex",
      height: "100%",
      width: "100%",
    }}>
      <Sidebar
        navItems={navItems}
        activeId={resolvedNav}
        onNavigate={handleNavigate}
      />
      <div id="g3-content" style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <CommandPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        navItems={navItems}
        activeId={resolvedNav}
        onNavigate={handleNavigate}
      />
      <Topbar crumb={crumb} onOpenNotifications={() => handleNavigate("notifications")} notifHidden={!notifEnabled} onOpenSearch={() => setPaletteOpen(true)} />
        <main
          id="g3-main"
          className={canvasPages.has(effectiveNav) ? "g3-canvas-grid" : undefined}
          style={{
            flex: 1,
            overflow: "auto",
            padding: "20px 26px",
          }}>
        {/* fallback=null：startTransition 已保证挂起时旧树留屏, fallback 正常路径不会被看到；
            仅冷启动首帧（无旧树可留）时短暂命中，此时页面本就空白，null 与骨架视觉等价。 */}
        <ErrorBoundary scope="page" key={effectiveNav}>
        <Suspense fallback={null}>
          <div className="animate-fade-in" key={effectiveNav}>
            {effectiveNav === "home" && <Home onNavigate={handleNavigate} />}
                {effectiveNav === "platforms" && <Platforms onNavigate={handleNavigate} initialFilter={navContext} />}
            {effectiveNav === "request-log" && <RequestLog />}
            {effectiveNav === "mitm-log" && <MitmLog />}
            {effectiveNav === "settings" && <AppSettings tab={settingsTab} onNotifSettingsChanged={(enabled) => setNotifEnabled(enabled)} />}
            {effectiveNav === "logs" && <Logs initialFilter={navContext} />}
            {effectiveNav === "stats" && <Stats initialFilter={navContext} />}
            {effectiveNav === "notifications" && <Notifications onNavigate={handleNavigate} />}
            {effectiveNav === "skills" && <Skills />}
            {effectiveNav === "mcp" && <Mcp />}
            {effectiveNav === "about" && <About />}
          </div>
        </Suspense>
        </ErrorBoundary>
      </main>
      </div>
      {pendingUpdate && (
        <UpdatePromptModal
          update={pendingUpdate}
          onClose={() => setPendingUpdate(null)}
        />
      )}
      {/* 票 10：浏览器形态的「选路径」弹窗宿主。桌面版走原生对话框，这里永远渲染 null。 */}
      <PathPickerHost />
    </div>
  );
}

export default App;
