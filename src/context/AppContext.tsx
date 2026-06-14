import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  useMemo,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import type { Locale } from "../locales";
import { isRTL, ensureLocaleLoaded } from "../locales";
import {
  type ThemeMode,
  type ThemeStyle,
  type ThemeColor,
  applyTheme,
  getAvailableStyles,
  getAvailableColors,
  DEFAULT_STYLE,
  DEFAULT_COLOR,
  DEFAULT_MODE,
} from "../themes";
import { settingsApi } from "../services/api";

interface Settings {
  locale: Locale;
  themeStyle: ThemeStyle;
  themeColor: ThemeColor;
  themeMode: ThemeMode;
}

interface AppContextValue extends Settings {
  setLocale: (locale: Locale) => void;
  setThemeStyle: (style: ThemeStyle) => void;
  setThemeColor: (color: ThemeColor) => void;
  setThemeMode: (mode: ThemeMode) => void;
  toggleMode: () => void;
  availableStyles: ReturnType<typeof getAvailableStyles>;
  availableColors: ReturnType<typeof getAvailableColors>;
}

const AppContext = createContext<AppContextValue | null>(null);

const STORAGE_KEY = "aidog-settings";

/** 旧 themeName → 新 {style,color} 迁移映射。 */
const LEGACY_THEME_MAP: Record<string, { style: ThemeStyle; color: ThemeColor }> = {
  liquidGlass: { style: "liquidGlass", color: "appleBlue" },
  nord: { style: "flat", color: "nord" },
  dracula: { style: "flat", color: "dracula" },
  catppuccin: { style: "flat", color: "catppuccin" },
  solarized: { style: "flat", color: "solarized" },
};

interface RawSettings {
  locale?: Locale;
  themeStyle?: ThemeStyle;
  themeColor?: ThemeColor;
  themeMode?: ThemeMode;
  /** 旧字段，迁移用。 */
  themeName?: string;
}

/**
 * 读取并迁移设置。
 * 优先用新字段；否则按旧 themeName 迁移；未知旧值回退默认，不白屏。
 */
function loadSettings(): Settings {
  let raw: RawSettings = {};
  try {
    const s = localStorage.getItem(STORAGE_KEY);
    if (s) raw = JSON.parse(s) as RawSettings;
  } catch {
    // ignore
  }

  const locale: Locale = raw.locale ?? "zh-CN";
  const themeMode: ThemeMode = raw.themeMode ?? DEFAULT_MODE;

  // 已是新结构
  if (raw.themeStyle && raw.themeColor) {
    return { locale, themeStyle: raw.themeStyle, themeColor: raw.themeColor, themeMode };
  }

  // 旧结构迁移
  const migrated = raw.themeName ? LEGACY_THEME_MAP[raw.themeName] : undefined;
  return {
    locale,
    themeStyle: migrated?.style ?? DEFAULT_STYLE,
    themeColor: migrated?.color ?? DEFAULT_COLOR,
    themeMode,
  };
}

function saveSettings(s: Settings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings>(loadSettings);
  const { i18n } = useTranslation();
  const availableStyles = useMemo(() => getAvailableStyles(), []);
  const availableColors = useMemo(() => getAvailableColors(), []);

  // 启动即把迁移后的新结构写回 localStorage（旧 themeName 用户升级一次性物化）
  useEffect(() => {
    saveSettings(settings);
    // 仅启动跑一次
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 同步 i18n + RTL + 持久化 locale 到 DB（供后端 proxy 错误消息使用）
  useEffect(() => {
    let cancelled = false;
    ensureLocaleLoaded(settings.locale).then(() => {
      if (!cancelled) i18n.changeLanguage(settings.locale);
    });
    document.documentElement.dir = isRTL(settings.locale) ? "rtl" : "ltr";
    document.documentElement.lang = settings.locale;
    settingsApi.set("app", "locale", { locale: settings.locale }).catch(() => {});
    return () => { cancelled = true; };
  }, [settings.locale, i18n]);

  // 同步主题（3 轴）
  useEffect(() => {
    applyTheme(settings.themeStyle, settings.themeColor, settings.themeMode);
  }, [settings.themeStyle, settings.themeColor, settings.themeMode]);

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

  const setLocale = useCallback((locale: Locale) => update({ locale }), [update]);
  const setThemeStyle = useCallback(
    (themeStyle: ThemeStyle) => update({ themeStyle }),
    [update],
  );
  const setThemeColor = useCallback(
    (themeColor: ThemeColor) => update({ themeColor }),
    [update],
  );
  const setThemeMode = useCallback(
    (themeMode: ThemeMode) => update({ themeMode }),
    [update],
  );

  const value = useMemo<AppContextValue>(
    () => ({
      ...settings,
      setLocale,
      setThemeStyle,
      setThemeColor,
      setThemeMode,
      toggleMode,
      availableStyles,
      availableColors,
    }),
    [settings, setLocale, setThemeStyle, setThemeColor, setThemeMode, toggleMode, availableStyles, availableColors],
  );

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp must be used within AppProvider");
  return ctx;
}
