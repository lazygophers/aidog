# 冷启动阻塞消除与 bundle 拆分 — 详细设计

## 现状（含实测秒数）

| 问题 | 位置 | 数据 |
|---|---|---|
| **`$SHELL -ilc 'echo $PATH'`** | `src-tauri/src/app_setup.rs:21` → `gateway/skills/env.rs:23,36-41` | **实测 1.54s / 0.74s / 0.71s** |
| 启动期 4 处 `block_on` + `cleanup_old_logs` | `app_setup.rs:100-140` | 同步阻塞主线程 |
| 已有 spawn 先例 | `app_setup.rs:49,66,84` | 同文件内已有异步 idiom，直接抄 |
| 托盘 debounce | `app_setup.rs:394-416` | 200ms |
| settings 同步 | `sync_settings.rs:42-47` | 启动期 |
| `RollingFileAppender` **同步写** | `logging.rs:191-245` | 每条日志同步落盘 |
| sharded_slab 高水位不回落 | `logging.rs:222/235/241` | 常驻内存只涨不降 |
| 日志其他 | `logging.rs:160-179`、`:218`、`:120-132` | |
| **main bundle 1.6M** | `dist/assets/main-trwYgpvB.js` 实测 | + `window-DyisxYjm.js` 483.8K + CSS 55K |
| 14 页静态 import | `src/App.tsx:1-24` | 全部页面进 main chunk |
| 切页整树重建 | `src/App.tsx:158-205` `key={effectiveNav}` | 每次切页丢弃整棵子树重建 |
| 无 manualChunks | `vite.config.ts` | locale 已分包（ar-SA 136.2K … ru-RU 154.2K），页面未分 |

## 方案（当前方案 = 精简守现状）

### 1. PATH 探测异步化（最大单项，0.7-1.5s）

`app_setup.rs:21` 的调用挪出启动同步路径，抄同文件 `:49,66,84` 已有的 spawn idiom。

**硬约束**：`gateway/skills/env.rs` 的 `OnceLock` 幂等语义**保留不变**，本 task **只挪调用点**，不改 env.rs 的实现。首个真正需要 PATH 的消费者若早于探测完成，需 await 该 OnceLock —— 语义与现状一致（现状是启动时阻塞等它），只是等待点后移。

### 2. 启动期 block_on

`app_setup.rs:100-140` 的 4 处 `block_on` + `cleanup_old_logs` 逐个判：
- 启动**必须**完成的（影响首屏正确性）→ 保留
- 可后台的（`cleanup_old_logs` 明显属此类）→ spawn

逐个判，不一刀切。

### 3. 日志写入

`logging.rs:191-245` 的 `RollingFileAppender` 包 `tracing_appender::non_blocking`。**guard 必须保活**（否则进程退出时日志丢失）—— guard 存到 app state 或 `OnceLock`。

sharded_slab 高水位（`:222/235/241`）：先量实际常驻量再决定改不改；若量级 <1MB 则显式记「已查，无阻断项」。

### 4. bundle 拆分

- `App.tsx:1-24` 的 14 页静态 import → `React.lazy` + `Suspense`
- `vite.config.ts` 补 `manualChunks`（locale 已有先例）

目标：main chunk 显著下降（具体数字 grill 定），首屏只加载当前页。

### 5. `key={effectiveNav}`

`App.tsx:158-205` 用 `key` 强制整树重建 —— 每次切页丢弃全部子树状态。这是**故意的**（保证切页干净）还是**顺手的**？先读代码判意图；若为顺手，去掉 `key` 可省大量重渲染，但可能引入跨页状态残留 → **需实测切页表现（红线 3）**，且改动前后逐页人工比对。

## 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 启动时预热任何东西 | **红线 4 明令排除**（冷启动不得变慢） |
| 缓存 PATH 到磁盘 | 用户改 shell 配置后失效，需失效策略；异步化已解决延迟，不必引入缓存一致性问题 |
| 换掉 `$SHELL -ilc` 改读 `/etc/paths` | 拿不到用户 shell rc 里 export 的路径，功能降级 |
| SSR / 预渲染 | Tauri 本地应用无此场景 |
| 全站路由库（react-router） | 项目明确**无 react-router**（CLAUDE.md），引入是架构变更，超本 task 边界 |

## 数据流（验证链路）

```
release 构建 → 干净重启（每次独立，禁复用进程）
  → 计时：进程启动 → 首屏可交互（多次取中位）
  → 改前/改后对比，红线 4 判据：不得变慢
  → yarn build 产物体积对比（main chunk 字节）
  → 逐页切换人工比对（若动 key={effectiveNav}），红线 3 判据
```

**与 `frontend-compositing-purge` 的关系**：本 task `deps=[frontend-compositing-purge]` —— 二者共享量测环境与 bundle 体积基线，按 CLAUDE.md「共享环境先串行化」。

## 可能性分支（不进当前方案，仅留痕）

- **启动画面 / 骨架屏** — 触发条件：若异步化后仍有可感知的首屏空白。代价是多一套 UI + i18n。
- **PATH 探测结果持久化 + 后台刷新** — 触发条件：若异步化后首个 PATH 消费者仍需等待 >1s。
- **路由级 code splitting 之外的 vendor 分包** — 触发条件：若 `React.lazy` 后 main chunk 仍偏大，再拆 vendor。
