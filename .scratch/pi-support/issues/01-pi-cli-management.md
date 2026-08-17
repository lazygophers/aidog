# 01 — pi 装得上：CLI 注册、版本检测、安装升级

**What to build:** 用户在 About 页看到 pi 与 Claude Code / Codex 并列的一行，显示已装版本、可执行路径与状态（installed / broken / conflict）。未装时能点一下装上，有新版时能点一下升级。冲突诊断（同名二进制多处安装）也认得 pi。

pi 的 npm 包是 `@earendil-works/pi-coding-agent`，二进制名 `pi`（`packages/coding-agent/docs/quickstart.md:9-11`）。走 npm 安装路径，与 Codex 同形；pi 也提供独立二进制，但本票只做 npm 路径。

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] 工具注册表含 pi，且注册表断言测试同步更新
- [ ] 版本检测能在已装/未装/损坏三种情况下返回正确状态
- [ ] 安装与升级路径对 pi 可用，失败时错误信息指明是哪一步失败
- [ ] 冲突诊断把多处安装的 pi 列出来
- [ ] 前端工具类型联合不再写死两个客户端，About 页三行并列
- [ ] 新增文案 8 语言齐，`check-i18n` 绿
- [ ] `cargo test` / `cargo clippy` / `yarn test` / `yarn build` 全绿
