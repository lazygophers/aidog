import { useState } from "react";
import { Sidebar, type NavItem } from "./components/Sidebar";
import { Platforms } from "./pages/Platforms";
import { Groups } from "./pages/Groups";
import { Proxy } from "./pages/Proxy";
import { Settings } from "./pages/Settings";

const NAV_ITEMS: NavItem[] = [
  { id: "proxy", icon: "proxy", labelKey: "nav.proxy" },
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms" },
  { id: "groups", icon: "groups", labelKey: "nav.groups" },
  { id: "settings", icon: "settings", labelKey: "nav.settings" },
];

function App() {
  const [activeNav, setActiveNav] = useState("proxy");

  return (
    <div style={{
      display: "flex",
      height: "100%",
      width: "100%",
      padding: 12,
      gap: 12,
    }}>
      <Sidebar
        navItems={NAV_ITEMS}
        activeId={activeNav}
        onNavigate={setActiveNav}
      />
      <main style={{
        flex: 1,
        overflow: "auto",
        padding: "24px 32px",
      }}>
        <div className="animate-fade-in" key={activeNav}>
          {activeNav === "proxy" && <Proxy />}
          {activeNav === "platforms" && <Platforms />}
          {activeNav === "groups" && <Groups />}
          {activeNav === "settings" && <Settings />}
        </div>
      </main>
    </div>
  );
}

export default App;
