import { useState, useEffect } from "react";
import { Sidebar, type NavItem } from "./components/Sidebar";
import { Platforms } from "./pages/Platforms";
import { Groups } from "./pages/Groups";
import { Proxy } from "./pages/Proxy";
import { Settings } from "./pages/Settings";
import { AppSettings } from "./pages/AppSettings";
import { Logs } from "./pages/Logs";
import { proxyLogApi } from "./services/api";

const BASE_NAV: NavItem[] = [
  { id: "proxy", icon: "proxy", labelKey: "nav.proxy" },
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms" },
  { id: "groups", icon: "groups", labelKey: "nav.groups" },
  { id: "settings", icon: "settings", labelKey: "nav.appSettings" },
  { id: "claudeConfig", icon: "settings", labelKey: "nav.claudeConfig" },
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
    setActiveNav(id);
    if (id === "logs") {
      proxyLogApi.getSettings()
        .then(s => setLogEnabled(s.enabled))
        .catch(() => {});
    }
  };

  // Insert logs nav item after proxy when enabled
  const navItems = logEnabled
    ? [BASE_NAV[0], LOG_NAV_ITEM, ...BASE_NAV.slice(1)]
    : BASE_NAV;

  // If logs was active but logging got disabled, fallback to proxy
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
          {effectiveNav === "claudeConfig" && <Settings />}
          {effectiveNav === "logs" && <Logs />}
        </div>
      </main>
    </div>
  );
}

export default App;
