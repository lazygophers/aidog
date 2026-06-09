import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { useApp } from "./context/AppContext";
import { ALL_LOCALES } from "./locales";

function App() {
  const { t } = useTranslation();
  const {
    locale,
    setLocale,
    themeName,
    setThemeName,
    themeMode,
    toggleMode,
    availableThemes,
  } = useApp();
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const [langOpen, setLangOpen] = useState(false);
  const [themeOpen, setThemeOpen] = useState(false);

  async function greet() {
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <div
      className="flex flex-col items-center gap-md"
      style={{ padding: 32, height: "100%" }}
    >
      {/* Header bar */}
      <div className="flex items-center gap-md" style={{ width: "100%" }}>
        <h1 style={{ fontSize: 20, fontWeight: 600 }}>{t("app.title")}</h1>
        <div style={{ flex: 1 }} />

        {/* Theme dropdown */}
        <div style={{ position: "relative" }}>
          <button className="btn" onClick={() => setThemeOpen((v) => !v)}>
            🎨 {t(`theme.${themeName}`)} ▾
          </button>
          {themeOpen && (
            <div
              className="glass-elevated"
              style={{
                position: "absolute",
                top: "100%",
                right: 0,
                marginTop: 4,
                minWidth: 180,
                padding: 4,
                zIndex: 100,
              }}
            >
              {availableThemes.map((th) => (
                <button
                  key={th.name}
                  className="btn"
                  style={{
                    width: "100%",
                    justifyContent: "flex-start",
                    background:
                      th.name === themeName
                        ? "var(--accent-subtle)"
                        : undefined,
                    fontWeight: th.name === themeName ? 600 : 400,
                    border: "none",
                    borderRadius: "var(--radius-sm)",
                  }}
                  onClick={() => {
                    setThemeName(th.name);
                    setThemeOpen(false);
                  }}
                >
                  {t(th.label)}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Language dropdown */}
        <div style={{ position: "relative" }}>
          <button className="btn" onClick={() => setLangOpen((v) => !v)}>
            🌐 {t(`lang.${locale}`)} ▾
          </button>
          {langOpen && (
            <div
              className="glass-elevated"
              style={{
                position: "absolute",
                top: "100%",
                right: 0,
                marginTop: 4,
                minWidth: 160,
                padding: 4,
                zIndex: 100,
              }}
            >
              {ALL_LOCALES.map((loc) => (
                <button
                  key={loc}
                  className="btn"
                  style={{
                    width: "100%",
                    justifyContent: "flex-start",
                    background:
                      loc === locale ? "var(--accent-subtle)" : undefined,
                    fontWeight: loc === locale ? 600 : 400,
                    border: "none",
                    borderRadius: "var(--radius-sm)",
                  }}
                  onClick={() => {
                    setLocale(loc);
                    setLangOpen(false);
                  }}
                >
                  {t(`lang.${loc}`)}
                </button>
              ))}
            </div>
          )}
        </div>

        <button className="btn" onClick={toggleMode}>
          {themeMode === "light" ? "🌙" : "☀️"}
        </button>
      </div>

      {/* Glass demo card */}
      <div
        className="glass"
        style={{ padding: 32, width: "100%", maxWidth: 480 }}
      >
        <form
          className="flex items-center gap-sm"
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
    </div>
  );
}

export default App;
