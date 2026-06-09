import { useState } from "react";
import { Sidebar, type NavItem } from "./components/Sidebar";
import { Platforms } from "./pages/Platforms";
import { Groups } from "./pages/Groups";
import { Proxy } from "./pages/Proxy";

const NAV_ITEMS: NavItem[] = [
  { id: "proxy", icon: "🚀", labelKey: "nav.proxy" },
  { id: "platforms", icon: "🔌", labelKey: "nav.platforms" },
  { id: "groups", icon: "📂", labelKey: "nav.groups" },
];

function App() {
  const [activeNav, setActiveNav] = useState("proxy");

  return (
    <div style={{ display: "flex", height: "100%", width: "100%" }}>
      <Sidebar
        navItems={NAV_ITEMS}
        activeId={activeNav}
        onNavigate={setActiveNav}
      />
      <main
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          alignItems: "flex-start",
          padding: 32,
          gap: 24,
          overflow: "auto",
        }}
      >
        {activeNav === "proxy" && <Proxy />}
        {activeNav === "platforms" && <Platforms />}
        {activeNav === "groups" && <Groups />}
      </main>
    </div>
  );
}

export default App;
