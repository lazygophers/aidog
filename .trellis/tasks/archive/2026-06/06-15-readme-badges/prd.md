# PRD: README + docs 添加 LINUX DO 社区徽章及常见 GitHub 徽章

## 目标
在 README（8 语言）和 docs Rspress 首页（8 语言 index.mdx）添加：
1. **LINUX DO 社区认可徽章** — 声明开源项目已链接认可 LINUX DO 社区
2. **常见 GitHub 动态徽章** — Stars / Downloads / Last Commit / Issues / PRs welcome

## 背景
- LINUX DO 开源推广发帖模板要求「我的开源项目已链接认可 LINUX DO 社区：是/否」
- 社区徽章惯例（来源 https://linux.do/t/topic/2144405 + https://github.com/programming666/ld-badge）：
  `![认可linux.do](https://ld.xh.do/ld-badge.svg)` 链接 `https://linux.do`
- 项目当前 README 仅 4 徽章（docs/release/license/platforms），缺社区认可 + 活跃度信号

## 产出
### 1. README（8 文件：README.md + README.{en,fr,de,ru,ar,es,ja}.md）
在现有 L12（platforms 徽章）后追加一行徽章：
```
[![认可LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)
[![GitHub Stars](https://img.shields.io/github/stars/lazygophers/aidog?style=social)](https://github.com/lazygophers/aidog/stargazers)
[![Downloads](https://img.shields.io/github/downloads/lazygophers/aidog/total?logo=github)](https://github.com/lazygophers/aidog/releases)
[![Last Commit](https://img.shields.io/github/last-commit/lazygophers/aidog?logo=git&logoColor=white)](https://github.com/lazygophers/aidog/commits)
[![Issues](https://img.shields.io/github/issues/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?logo=github)](https://github.com/lazygophers/aidog/pulls)
```
- LINUX DO 徽章 alt 文本按语言本地化（认可/Endorsed/Approuvé/...），SVG URL 不变

### 2. docs Rspress 首页（8 语言 docs/docs/{lang}/index.mdx）
在 frontmatter `---` 后、`## 三步上手`（或对应语言标题）前插入徽章段：
```mdx
<div align="center">

[![认可LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)
[![GitHub Stars](https://img.shields.io/github/stars/lazygophers/aidog?style=social)](https://github.com/lazygophers/aidog/stargazers)
[![Downloads](https://img.shields.io/github/downloads/lazygophers/aidog/total?logo=github)](https://github.com/lazygophers/aidog/releases)
[![Last Commit](https://img.shields.io/github/last-commit/lazygophers/aidog?logo=git&logoColor=white)](https://github.com/lazygophers/aidog/commits)

</div>
```

## 验证
- `grep "ld.xh.do" README*.md docs/docs/*/index.mdx` → 16 文件各 1 命中
- `grep "shields.io/github/stars" README*.md` → 8 文件各 1 命中
- docs 构建：`cd docs && yarn build`（Rspress）无报错
- 视觉：徽章在 README 预览居中、换行合理

## 范围（不做）
- 不改 README/docs 正文内容（仅徽章行）
- 不加 Discord/Telegram 等社交徽章（项目暂无社群）
- 不改 docs 配置（rspress.config.ts）

## 资源
- LINUX DO 徽章 SVG: https://ld.xh.do/ld-badge.svg
- shields.io 动态徽章: https://img.shields.io/github/<metric>/lazygophers/aidog
