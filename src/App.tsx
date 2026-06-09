import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Sidebar, type NavItem } from "./components/Sidebar";

const NAV_ITEMS: NavItem[] = [
  { id: "home", icon: "🏠", labelKey: "nav.home" },
  { id: "settings", icon: "⚙️", labelKey: "nav.settings" },
];

function App() {
  const { t } = useTranslation();
  const [activeNav, setActiveNav] = useState("home");
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <div style={{ display: "flex", height: "100%", width: "100%" }}>
      {/* Sidebar */}
      <Sidebar
        navItems={NAV_ITEMS}
        activeId={activeNav}
        onNavigate={setActiveNav}
      />

      {/* Main content */}
      <main
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          padding: 32,
          gap: 24,
          overflow: "auto",
        }}
      >
        <h1 style={{ fontSize: 28, fontWeight: 700 }}>
          {activeNav === "home" ? t("nav.home") : t("nav.settings")}
        </h1>

        <div className="glass" style={{ padding: 32, width: "100%", maxWidth: 480 }}>
          <form
            style={{ display: "flex", alignItems: "center", gap: 8 }}
            onSubmit={(e) => {
              e.preventDefault();
              greet();
            }}
          >
            <input
              className="input"
              style={{ flex: 1 }}
              placeholder={t("greet.placeholder")}
              onChange={(e) => setName(e.currentTarget.value)}
            />
            <button className="btn btn-primary" type="submit">
              {t("greet.button")}
            </button>
          </form>
          {greetMsg && (
            <p style={{ marginTop: 16, color: "var(--text-secondary)" }}>
              {greetMsg}
            </p>
          )}
        </div>
      </main>
    </div>
  );
}

export default App;
