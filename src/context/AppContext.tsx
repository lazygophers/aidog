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
  /** 从 DB 重读主题/语言偏好（导入 .aidogx 应用后刷新用）。 */
  reloadFromDB: () => Promise<void>;
  availableStyles: ReturnType<typeof getAvailableStyles>;
  availableColors: ReturnType<typeof getAvailableColors>;
}

const AppContext = createContext<AppContextValue | null>(null);

const STORAGE_KEY = "aidog-settings";
const SETTING_SCOPE = "app";
const THEME_KEY = "theme";
const LOCALE_KEY = "locale";

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
 * localStorage 同步读取 + 旧 themeName 迁移（首渲染 fallback，防白屏/闪烁）。
 */
function loadSettingsFromStorage(): Settings {
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

/**
 * DB 读取（权威源）。DB 缺字段不覆盖（partial），调用方与 fallback 合并。
 * 失败回退空对象（不阻断 UI）。
 */
async function loadSettingsFromDB(): Promise<Partial<Settings>> {
  try {
    const [themeRow, localeRow] = await Promise.all([
      settingsApi.get(SETTING_SCOPE, THEME_KEY),
      settingsApi.get(SETTING_SCOPE, LOCALE_KEY),
    ]);
    const partial: Partial<Settings> = {};
    if (
      themeRow &&
      typeof themeRow.style === "string" &&
      typeof themeRow.color === "string" &&
      typeof themeRow.mode === "string"
    ) {
      partial.themeStyle = themeRow.style as ThemeStyle;
      partial.themeColor = themeRow.color as ThemeColor;
      partial.themeMode = themeRow.mode as ThemeMode;
    }
    if (localeRow && typeof localeRow.locale === "string") {
      partial.locale = localeRow.locale as Locale;
    }
    return partial;
  } catch {
    return {};
  }
}

/**
 * 持久化（双写）：localStorage 过渡兜底 + DB 权威。
 * DB 写失败仅 log，不阻断 UI（localStorage 仍兜住本会话）。
 */
function persistSettings(s: Settings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  } catch {
    // ignore
  }
  settingsApi
    .set(SETTING_SCOPE, THEME_KEY, {
      style: s.themeStyle,
      color: s.themeColor,
      mode: s.themeMode,
    })
    .catch(() => {
      // DB 写失败不阻断 UI
    });
}

/** DB theme 行是否完整（三字段齐全）。 */
function dbThemeComplete(p: Partial<Settings>): p is Partial<Settings> & Pick<Settings, "themeStyle" | "themeColor" | "themeMode"> {
  return Boolean(p.themeStyle && p.themeColor && p.themeMode);
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings>(loadSettingsFromStorage);
  const { i18n } = useTranslation();
  const availableStyles = useMemo(() => getAvailableStyles(), []);
  const availableColors = useMemo(() => getAvailableColors(), []);

  // 启动：DB 权威覆盖 + 首启迁移（DB 无 theme → localStorage/默认物化到 DB，一次性）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const dbPartial = await loadSettingsFromDB();
      if (cancelled) return;
      setSettings((prev) => {
        const next: Settings = { ...prev, ...dbPartial } as Settings;
        // DB 无完整 theme → 迁移物化（旧 localStorage 用户首启写入 DB）
        if (!dbThemeComplete(dbPartial)) {
          settingsApi
            .set(SETTING_SCOPE, THEME_KEY, {
              style: next.themeStyle,
              color: next.themeColor,
              mode: next.themeMode,
            })
            .catch(() => {});
        }
        // 同步 localStorage（过渡双写）
        try {
          localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
        } catch {
          // ignore
        }
        return next;
      });
    })();
    return () => {
      cancelled = true;
    };
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
    settingsApi.set(SETTING_SCOPE, LOCALE_KEY, { locale: settings.locale }).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [settings.locale, i18n]);

  // 同步主题（3 轴）
  useEffect(() => {
    applyTheme(settings.themeStyle, settings.themeColor, settings.themeMode);
  }, [settings.themeStyle, settings.themeColor, settings.themeMode]);

  const update = useCallback(
    (patch: Partial<Settings>) => {
      setSettings((prev) => {
        const next = { ...prev, ...patch };
        persistSettings(next);
        return next;
      });
    },
    [],
  );

  const toggleMode = useCallback(() => {
    setSettings((prev) => {
      const nextMode: ThemeMode = prev.themeMode === "light" ? "dark" : "light";
      const next = { ...prev, themeMode: nextMode };
      persistSettings(next);
      return next;
    });
  }, []);

  // 从 DB 重读主题/语言偏好（导入 .aidogx 应用后刷新）
  const reloadFromDB = useCallback(async () => {
    const dbPartial = await loadSettingsFromDB();
    setSettings((prev) => {
      const next: Settings = { ...prev, ...dbPartial } as Settings;
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      } catch {
        // ignore
      }
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
      reloadFromDB,
      availableStyles,
      availableColors,
    }),
    [settings, setLocale, setThemeStyle, setThemeColor, setThemeMode, toggleMode, reloadFromDB, availableStyles, availableColors],
  );

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp must be used within AppProvider");
  return ctx;
}
