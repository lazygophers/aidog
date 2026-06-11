import { useState, useEffect } from "react";
import { Sidebar, type NavItem } from "./components/Sidebar";
import { Platforms } from "./pages/Platforms";
import { Groups } from "./pages/Groups";
import { Proxy } from "./pages/Proxy";
import { AppSettings } from "./pages/AppSettings";
import { Logs } from "./pages/Logs";
import { Stats } from "./pages/Stats";
import { proxyLogApi } from "./services/api";
import { requestNavigation } from "./utils/navGuard";

const BASE_NAV: NavItem[] = [
  { id: "proxy", icon: "proxy", labelKey: "nav.proxy" },
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms" },
  { id: "groups", icon: "groups", labelKey: "nav.groups" },
  { id: "stats", icon: "stats", labelKey: "nav.stats" },
  { id: "settings", icon: "settings", labelKey: "nav.settings" },
];

const LOG_NAV_ITEM: NavItem = { id: "logs", icon: "logs", labelKey: "nav.logs" };

function App() {
  const [activeNav, setActiveNav] = useState("proxy");
  const [logEnabled, setLogEnabled] = useState(false);

  useEffect(() => {
    proxyLogApi.getSettings()
      .then(s => setLogEnabled(s.enabled))
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

  const navItems = logEnabled
    ? [BASE_NAV[0], LOG_NAV_ITEM, ...BASE_NAV.slice(1)]
    : BASE_NAV;

  const effectiveNav = activeNav === "logs" && !logEnabled ? "proxy" : activeNav;

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
          {effectiveNav === "proxy" && <Proxy />}
          {effectiveNav === "platforms" && <Platforms />}
          {effectiveNav === "groups" && <Groups />}
          {effectiveNav === "settings" && <AppSettings onLogSettingsChanged={(enabled) => setLogEnabled(enabled)} />}
          {effectiveNav === "logs" && <Logs />}
          {effectiveNav === "stats" && <Stats />}
        </div>
      </main>
    </div>
  );
}

export default App;
