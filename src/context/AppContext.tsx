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
import { isRTL, ensureLocaleLoaded, ALL_LOCALES } from "../locales";

// legacy 持久化(如 "zh-CN")或脏值归一化到有效 locale, 防 t(`lang.${locale}`) 落空显裸 key
function normalizeLocale(l: unknown): Locale {
  return typeof l === "string" && (ALL_LOCALES as string[]).includes(l)
    ? (l as Locale)
    : "zh-Hans";
}
import {
  type ThemeMode,
  applyTheme,
  DEFAULT_MODE,
} from "../themes";
import { settingsApi } from "../services/api";
// buildProtocolsFromPresets 由各 consumer 自行 await（hosts 内联派生，无需启动期注入）。
import { buildProtocolsFromPresets, buildClientTypesFromPresets } from "../domains/platforms";

interface Settings {
  locale: Locale;
  themeMode: ThemeMode;
}

interface AppContextValue extends Settings {
  setLocale: (locale: Locale) => void;
  setThemeMode: (mode: ThemeMode) => void;
  toggleMode: () => void;
  /** 从 DB 重读主题/语言偏好（导入 .aidogx 应用后刷新用）。 */
  reloadFromDB: () => Promise<void>;
}

const AppContext = createContext<AppContextValue | null>(null);

const STORAGE_KEY = "aidog-settings";
const SETTING_SCOPE = "app";
const THEME_KEY = "theme";
const LOCALE_KEY = "locale";

interface RawSettings {
  locale?: Locale;
  themeMode?: ThemeMode;
}

/**
 * localStorage 同步读取（首渲染 fallback，防白屏/闪烁）。
 * 主题收敛为单一 mono 主题 + mode，旧 themeStyle/themeColor 字段忽略。
 */
function loadSettingsFromStorage(): Settings {
  let raw: RawSettings = {};
  try {
    const s = localStorage.getItem(STORAGE_KEY);
    if (s) raw = JSON.parse(s) as RawSettings;
  } catch {
    // ignore
  }
  return {
    locale: normalizeLocale(raw.locale),
    themeMode: raw.themeMode ?? DEFAULT_MODE,
  };
}

/**
 * DB 读取（权威源）。DB 缺字段不覆盖（partial），调用方与 fallback 合并。
 * 失败回退空对象（不阻断 UI）。旧 theme 行的 style/color 字段忽略，只取 mode。
 */
async function loadSettingsFromDB(): Promise<Partial<Settings>> {
  try {
    const [themeRow, localeRow] = await Promise.all([
      settingsApi.get(SETTING_SCOPE, THEME_KEY),
      settingsApi.get(SETTING_SCOPE, LOCALE_KEY),
    ]);
    const partial: Partial<Settings> = {};
    if (themeRow && typeof themeRow.mode === "string") {
      partial.themeMode = themeRow.mode as ThemeMode;
    }
    if (localeRow && typeof localeRow.locale === "string") {
      partial.locale = normalizeLocale(localeRow.locale);
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
    .set(SETTING_SCOPE, THEME_KEY, { mode: s.themeMode })
    .catch(() => {
      // DB 写失败不阻断 UI
    });
}

/** DB theme 行是否完整（mode 存在）。 */
function dbThemeComplete(p: Partial<Settings>): p is Partial<Settings> & Pick<Settings, "themeMode"> {
  return Boolean(p.themeMode);
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings>(loadSettingsFromStorage);
  const { i18n } = useTranslation();

  // 启动：DB 权威覆盖 + 首启迁移（DB 无 theme → localStorage/默认物化到 DB，一次性）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      // PROTOCOLS 已删（派生层 buildProtocolsFromPresets 内联 hosts 派生，consumer 各自 await）；
      // 此处仅预热 docPromise 单次 RPC 缓存，让首个 consumer 拿到同步态快。
      buildProtocolsFromPresets().catch(() => { /* best-effort 预热 */ });
      // CLIENT_TYPES 删除（JSON 派生）：预热 clientTypesDocPromise 单次 RPC 缓存（同上范式）。
      buildClientTypesFromPresets().catch(() => { /* best-effort 预热 */ });
      const dbPartial = await loadSettingsFromDB();
      if (cancelled) return;
      setSettings((prev) => {
        const next: Settings = { ...prev, ...dbPartial } as Settings;
        // DB 无 theme → 迁移物化（旧 localStorage 用户首启写入 DB）
        if (!dbThemeComplete(dbPartial)) {
          settingsApi
            .set(SETTING_SCOPE, THEME_KEY, { mode: next.themeMode })
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

  // 同步主题（mono × mode）
  useEffect(() => {
    applyTheme(settings.themeMode);
  }, [settings.themeMode]);

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
  const setThemeMode = useCallback(
    (themeMode: ThemeMode) => update({ themeMode }),
    [update],
  );

  const value = useMemo<AppContextValue>(
    () => ({
      ...settings,
      setLocale,
      setThemeMode,
      toggleMode,
      reloadFromDB,
    }),
    [settings, setLocale, setThemeMode, toggleMode, reloadFromDB],
  );

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp must be used within AppProvider");
  return ctx;
}
