import * as path from 'node:path';
import { defineConfig } from '@rspress/core';

export default defineConfig({
  root: path.join(__dirname, 'docs'),
  title: 'aidog',
  description: 'aidog AI API 聚合、路由与用量管理文档。',
  lang: 'zh',
  icon: '/aidog-logo.svg',
  logo: '/aidog-logo.svg',
  logoText: 'aidog',
  search: {
    mode: 'local',
    codeBlocks: true,
  },
  llms: true,
  locales: [
    {
      lang: 'zh',
      label: '简体中文',
      title: 'aidog',
      description: 'aidog AI API 聚合、路由与用量管理文档。',
    },
    {
      lang: 'en',
      label: 'English',
      title: 'aidog',
      description:
        'Documentation for aidog AI API aggregation, routing, and usage management.',
    },
  ],
  themeConfig: {
    search: true,
    llmsUI: true,
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/lazygophers/aidog',
      },
    ],
  },
});
