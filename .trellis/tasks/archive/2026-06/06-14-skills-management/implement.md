# Implement — skills 管理模块

## 五要素

- **目标**: aidog 顶层「Skills」页 + 后端 skills 子系统，实现浏览/搜索/安装/列已装/卸载/更新（混合：读原生 + 写 shell out `npx skills`）。
- **产出**: `src-tauri/src/gateway/skills.rs` + lib.rs 7 command；`src/pages/Skills.tsx` + `services/api.ts` 封装 + `App.tsx` navItems + 7 语言 i18n。
- **验证**: `cargo clippy` 无 warning + `cargo test` 绿；`yarn build` 绿；7 语言 key 齐；手验环境探测/装/列/卸/更新逻辑路径。
- **资源**: PRD + spec(frontend/conventions, backend/index, guides/cross-layer+code-reuse) + 参考 notification.rs:264 shell out。
- **依赖**: 本机 node/npx（写操作）；无 DB schema 变更。

## 构建顺序（单 deliverable，worktree 内串行）

1. **后端契约先行** `gateway/skills.rs`
   - 数据模型：`SkillInfo { name, source, agent, scope, installed_path, version? }`、`CatalogEntry`、`SkillsEnv { npx_available, node_version? }`、`SkillScope { Global | Project(path) }`。
   - 读：`scan_installed(scope, agent)` 扫 `<base>/.claude|.codex|...skills/`；`browse_catalog()` HTTP 抓 skills.sh（失败回退 npx find）；`search(kw)`。
   - 写：`run_npx(args)` 封装 `Command::new("npx").args(["skills", ...])`，捕获 stdout/stderr/exit；`install/update/remove`。
   - `check_env()` 探测 npx/node。
2. **lib.rs 注册 7 command**：`skills_check_env` / `skills_browse_catalog` / `skills_search` / `skills_list_installed` / `skills_install` / `skills_remove` / `skills_update`。invoke_handler 登记。
3. **api.ts 封装** + TS 类型（与 Rust 模型字段名一一对齐，见 cross-layer-rules）。
4. **Skills.tsx 页面**：scope 选择（默认 Global / 选项目目录）+ agent 选择；catalog 浏览/搜索区；已装列表（卸载/更新）；环境缺失提示条。复用 shared 组件 + formatters，禁页内重复。
5. **App.tsx navItems** 加 `{ id: "skills", icon, labelKey: "nav.skills" }` + 路由分支渲染 `<Skills/>`。
6. **i18n 7 语言**：nav.skills + 页面所有字面量 key，7 个 locale 全补（含 ar-SA RTL）。

## 失败处理

- npx/node 缺失 → check_env 返回 false，页面顶部提示条引导装 node，禁阻塞整页。
- npx skills 子命令非 0 退出 → 捕获 stderr 回前端 toast，不 panic。
- catalog HTTP 失败 → 回退 `npx skills find`；再失败 → 空列表 + 提示。
- scope=Project 但路径无效 → 前端校验 + 后端兜底报错。

## 注意

- URL/路径不额外拼接；遵 spec。
- 项目无前端 lint/test，仅 `yarn build`(tsc) 把关类型。
- 单 deliverable，main 在 worktree 内一次性实现（前后端契约一致性要求不拆并行）。
