// Tailwind v4 在 Next.js 下走 PostCSS（桌面 Vite 走仓库根的 @tailwindcss/vite 插件）。
// 本文件在 web/ 子目录内，Vite（在仓库根）不会加载它，两条构建链互不干扰。
const config = { plugins: { "@tailwindcss/postcss": {} } };
export default config;
