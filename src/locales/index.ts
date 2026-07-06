import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zhHans from "./zh-Hans.json";
import enUS from "./en-US.json";

export type Locale =
  | "zh-Hans"
  | "en-US"
  | "ar-SA"
  | "fr-FR"
  | "de-DE"
  | "ru-RU"
  | "ja-JP"
  | "es-ES";

export const ALL_LOCALES: Locale[] = [
  "zh-Hans",
  "en-US",
  "ar-SA",
  "fr-FR",
  "de-DE",
  "ru-RU",
  "ja-JP",
  "es-ES",
];

/** RTL 语言列表 */
export const RTL_LOCALES: readonly Locale[] = ["ar-SA"];

export function isRTL(locale: Locale): boolean {
  return (RTL_LOCALES as readonly string[]).includes(locale);
}

// 默认语言 (zh-Hans) + fallback (en-US) 同步打包，保证首屏 t() 立即可用；
// 其余 5 语言按需 dynamic import，由 Vite 拆为独立 chunk，减小主包体积。
const lazyLoaders: Partial<Record<Locale, () => Promise<{ default: Record<string, unknown> }>>> = {
  "ar-SA": () => import("./ar-SA.json"),
  "fr-FR": () => import("./fr-FR.json"),
  "de-DE": () => import("./de-DE.json"),
  "ru-RU": () => import("./ru-RU.json"),
  "ja-JP": () => import("./ja-JP.json"),
  "es-ES": () => import("./es-ES.json"),
};

i18n.use(initReactI18next).init({
  resources: {
    "zh-Hans": { translation: zhHans },
    "en-US": { translation: enUS },
  },
  lng: "zh-Hans",
  fallbackLng: "en-US",
  interpolation: { escapeValue: false },
});

/**
 * 确保指定 locale 的翻译资源已加载，再返回。
 * 对已打包 (zh-Hans / en-US) 或已加载过的 locale 直接返回。
 * 按需 locale 首次切换时 dynamic import 并注入 i18n。
 */
export async function ensureLocaleLoaded(locale: Locale): Promise<void> {
  if (i18n.hasResourceBundle(locale, "translation")) return;
  const loader = lazyLoaders[locale];
  if (!loader) return;
  try {
    const mod = await loader();
    i18n.addResourceBundle(locale, "translation", mod.default, true, true);
  } catch {
    // 加载失败时静默降级到 fallback (en-US)，不阻塞切换
  }
}

export default i18n;
