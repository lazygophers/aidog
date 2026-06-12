# 开发环境搭建

## 前置依赖

| 工具 | 版本要求 | 安装方式 |
|------|---------|---------|
| Node.js | >= 18 | [nvm](https://github.com/nvm-sh/nvm) 或 [mise](https://mise.jdx.dev) |
| Yarn | 4.x | `corepack enable && corepack prepare yarn@stable --activate` |
| Rust | stable | [rustup](https://rustup.rs) |
| Tauri CLI | latest | `cargo install tauri-cli` |

## 克隆与安装

```bash
git clone https://github.com/lazygophers/aidog.git
cd aidog
yarn install
```

## 开发模式

```bash
# 启动开发服务器（Vite + Tauri）
yarn tauri dev
```

这会：
1. 启动 Vite 开发服务器（HMR）
2. 编译 Rust 后端
3. 打开桌面窗口
4. 前端代码修改自动热更新

## 构建生产版本

```bash
yarn tauri build
```

输出安装包在 `src-tauri/target/release/bundle/`。

## 项目配置

- `mise.toml` — 工具版本管理
- `.yarnrc.yml` — Yarn 配置
- `vite.config.ts` — Vite 配置
- `tsconfig.json` — TypeScript 配置
- `src-tauri/Cargo.toml` — Rust 依赖

## 关键约定

### URL 构造
- `base_url` 含版本前缀（如 `/v1`）
- `provider_api_path()` 返回 `/chat/completions`
- 最终 URL = `base_url + provider_api_path()`
- **禁止额外拼接**

### 数值格式化
- 统一使用 `utils/formatters.ts`
- 禁页面内重复定义 formatNumber

### 导航
- 无 react-router
- 导航通过 App.tsx / AppSettings.tsx 本地 state
- 离页拦截用 `utils/navGuard.ts`
- **禁原生 confirm / beforeunload**（破坏 Tauri）

### i18n
- 所有 UI 文案走 `t()` 函数
- 禁硬编码字符串
- 7 种语言文件在 `src/locales/`
