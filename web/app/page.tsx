"use client";

// 浏览器版入口（票 I14）：吃掉 Vite 版 `src/main.tsx` 的三件事
// （挂载 <App/>、AppProvider、`import "./locales"`）。React.StrictMode 由
// React 19 的开发模式双渲染等价覆盖，这里不重复包。
import App from "@/App";
import { AppProvider } from "@/context/AppContext";
import "@/locales";

export default function Page() {
  return (
    <AppProvider>
      <App />
    </AppProvider>
  );
}
