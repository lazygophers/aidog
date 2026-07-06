// 组件测试自定义 render：包一层独立 I18nextProvider。
// 用空 resources 的隔离 i18n 实例 → 翻译函数直接回传 key 本身（react-i18next 缺译文回退 key），
// 断言用 i18n key 而非译文，避免文案变动致测试脆。
//
// 用法：import { render } from "../../test/render"; render(<X />); 之后用 screen 断言。

import { type ReactElement } from "react";
import { render as rtlRender, type RenderOptions } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { createInstance } from "i18next";
import { initReactI18next } from "react-i18next";

const testI18n = createInstance();
testI18n.use(initReactI18next).init({
  // 空 resources：任意 dot-notation key 找不到译文 → 回退返回 key 本身。
  resources: { "zh-Hans": { translation: {} } },
  lng: "zh-Hans",
  fallbackLng: "zh-Hans",
  interpolation: { escapeValue: false },
  // 缺 key 时返回 key（react-i18next 默认行为），不输出告警噪声。
  parseMissingKeyHandler: (key) => key,
});

export function render(ui: ReactElement, options?: Omit<RenderOptions, "wrapper">) {
  return rtlRender(ui, {
    wrapper: ({ children }) => (
      <I18nextProvider i18n={testI18n}>{children}</I18nextProvider>
    ),
    ...options,
  });
}

export { testI18n };
export * from "@testing-library/react";
