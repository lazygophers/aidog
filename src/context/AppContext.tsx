import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import type { Locale } from "../locales";
import { isRTL } from "../locales";
import {
  type ThemeMode,
  type ThemeName,
  applyTheme,
  getAvailableThemes,
} from "../themes";

interface Settings {
  locale: Locale;
  themeName: ThemeName;
  themeMode: ThemeMode;
}

interface AppContextValue extends Settings {
  setLocale: (locale: Locale) => void;
  setThemeName: (name: ThemeName) => void;
  setThemeMode: (mode: ThemeMode) => void;
  toggleMode: () => void;
  availableThemes: ReturnType<typeof getAvailableThemes>;
}

const AppContext = createContext<AppContextValue | null>(null);

const STORAGE_KEY = "aidog-settings";

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw) as Settings;
  } catch {
    // ignore
  }
  return { locale: "zh-CN", themeName: "liquidGlass", themeMode: "light" };
}

function saveSettings(s: Settings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings>(loadSettings);
  const { i18n } = useTranslation();
  const availableThemes = getAvailableThemes();

  // 同步 i18n + RTL
  useEffect(() => {
    i18n.changeLanguage(settings.locale);
    document.documentElement.dir = isRTL(settings.locale) ? "rtl" : "ltr";
    document.documentElement.lang = settings.locale;
  }, [settings.locale, i18n]);

  // 同步主题
  useEffect(() => {
    applyTheme(settings.themeName, settings.themeMode);
  }, [settings.themeName, settings.themeMode]);

  const update = useCallback(
    (patch: Partial<Settings>) => {
      setSettings((prev) => {
        const next = { ...prev, ...patch };
        saveSettings(next);
        return next;
      });
    },
    [],
  );

  const toggleMode = useCallback(() => {
    setSettings((prev) => {
      const nextMode: ThemeMode = prev.themeMode === "light" ? "dark" : "light";
      const next = { ...prev, themeMode: nextMode };
      saveSettings(next);
      return next;
    });
  }, []);

  return (
    <AppContext.Provider
      value={{
        ...settings,
        setLocale: (locale) => update({ locale }),
        setThemeName: (themeName) => update({ themeName }),
        setThemeMode: (themeMode) => update({ themeMode }),
        toggleMode,
        availableThemes,
      }}
    >
      {children}
    </AppContext.Provider>
  );
}

export function useApp() {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp must be used within AppProvider");
  return ctx;
}
