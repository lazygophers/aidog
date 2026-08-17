# 06 — pi 设置页、导航入口、图标

**What to build:** 用户在 aidog 的设置导航里看到「pi」条目，点进去能编辑 pi 的全局设置，不必手改 JSON。Sidebar 与 Group 列表里 pi 有自己的图标，与 Claude Code、Codex 视觉等价。

设置页对照现有 Codex 设置页的做法：按字段分节、给推荐默认、未被 schema 覆盖的键在往返中原样保留不丢失。pi 的设置是 JSON（不像 Codex 要 TOML 往返），少一层转换。

覆盖的字段以 pi 官方 settings 文档为准，挑用户真正会调的（默认 provider 与模型、思考等级、主题、压缩、重试、会话目录、代理），不必穷举。

**Blocked by:** 05

**Status:** ready-for-agent

- [ ] 设置导航新增 pi 条目，位置与 Claude Code / Codex 条目并列
- [ ] pi 设置页能读出现有配置、编辑、写回
- [ ] schema 未覆盖的键经一次读-写往返后原样保留
- [ ] 配置文件不存在时页面显示推荐默认而非报错
- [ ] Sidebar 与 Group 列表出现 pi 图标
- [ ] 新增文案 8 语言齐，`check-i18n` 绿
- [ ] `cargo test` / `cargo clippy` / `yarn test` / `yarn build` 全绿
