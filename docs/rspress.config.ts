import * as path from 'node:path';
import { defineConfig } from 'rspress/config';

export default defineConfig({
  root: 'docs',
  base: '/aidog/',
  lang: 'zh',
  title: 'AiDog',
  description: 'AI API 网关代理 — 多平台聚合、智能路由、用量统计',
  icon: '/logo.svg',
  globalStyles: path.join(__dirname, 'styles/custom.css'),
  locales: [
    {
      lang: 'zh',
      label: '简体中文',
      title: 'AiDog',
      description: 'AI API 网关代理 — 多平台聚合、智能路由、用量统计',
    },
    {
      lang: 'en',
      label: 'English',
      title: 'AiDog',
      description: 'AI API Gateway Proxy — Multi-platform aggregation, smart routing, usage analytics',
    },
    {
      lang: 'ja',
      label: '日本語',
      title: 'AiDog',
      description: 'AI API ゲートウェイプロキシ — マルチプラットフォーム統合、スマートルーティング、使用量分析',
    },
    {
      lang: 'fr',
      label: 'Français',
      title: 'AiDog',
      description: 'Proxy passerelle API IA — Agrégation multi-plateforme, routage intelligent, analytique d\'utilisation',
    },
    {
      lang: 'de',
      label: 'Deutsch',
      title: 'AiDog',
      description: 'KI-API-Gateway-Proxy — Multi-Plattform-Aggregation, intelligentes Routing, Nutzungsanalysen',
    },
    {
      lang: 'ar',
      label: 'العربية',
      title: 'AiDog',
      description: 'بوابة وكيل API للذكاء الاصطناعي — تجميع متعدد المنصات، توجيه ذكي، تحليلات الاستخدام',
    },
    {
      lang: 'es',
      label: 'Español',
      title: 'AiDog',
      description: 'Proxy de puerta de enlace API de IA — Agregación multiplataforma, enrutamiento inteligente, análisis de uso',
    },
    {
      lang: 'ru',
      label: 'Русский',
      title: 'AiDog',
      description: 'Прокси-шлюз API ИИ — Мультиплатформенная агрегация, умная маршрутизация, аналитика использования',
    },
  ],
  themeConfig: {
    socialLinks: [
      { icon: 'github', mode: 'link', content: 'https://github.com/lazygophers/aidog' },
    ],
  },
  builderConfig: {
    html: {
      tags: [
        { tag: 'meta', attrs: { name: 'theme-color', content: '#0a0a0a' } },
      ],
    },
  },
});
