import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zhCN from "./zh-CN.json";
import enUS from "./en-US.json";
import arSA from "./ar-SA.json";
import frFR from "./fr-FR.json";
import deDE from "./de-DE.json";
import ruRU from "./ru-RU.json";
import jaJP from "./ja-JP.json";

export type Locale =
  | "zh-CN"
  | "en-US"
  | "ar-SA"
  | "fr-FR"
  | "de-DE"
  | "ru-RU"
  | "ja-JP";

export const ALL_LOCALES: Locale[] = [
  "zh-CN",
  "en-US",
  "ar-SA",
  "fr-FR",
  "de-DE",
  "ru-RU",
  "ja-JP",
];

/** RTL 语言列表 */
export const RTL_LOCALES: readonly Locale[] = ["ar-SA"];

export function isRTL(locale: Locale): boolean {
  return (RTL_LOCALES as readonly string[]).includes(locale);
}

const resources = {
  "zh-CN": { translation: zhCN },
  "en-US": { translation: enUS },
  "ar-SA": { translation: arSA },
  "fr-FR": { translation: frFR },
  "de-DE": { translation: deDE },
  "ru-RU": { translation: ruRU },
  "ja-JP": { translation: jaJP },
};

i18n.use(initReactI18next).init({
  resources,
  lng: "zh-CN",
  fallbackLng: "en-US",
  interpolation: { escapeValue: false },
});

export default i18n;
