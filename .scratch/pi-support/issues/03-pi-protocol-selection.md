# 03 — 分组级线路协议选择

**What to build:** 用户在 Group 编辑面板选这个 Group 的 pi provider 说哪种线路协议，四选一：Anthropic Messages、OpenAI Chat Completions、OpenAI Responses、Google Generative AI。选完，aidog 生成的 provider 的 `api` 字段与 `baseUrl` 一起正确变化，用户永远不用自己拼 URL。

**这里有个反直觉的硬约束**：版本后缀规则在两类协议下是相反的。Anthropic 线路要代理根地址、**不带**版本后缀（其 SDK 自己补路径）；OpenAI 线路要**带**版本前缀。pi 自己的文档在这处给了错误示例，以 pi 源码里的内置 provider 常量为准。这条必须写成显式测试，否则后人会照着 pi 文档「修」回错的。

协议值存在 Group 已有的 `extra` JSON blob 里，不加数据库列 —— 与 Platform 级 `peak_hours` 的既有存法一致。

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] Group 编辑面板有协议下拉，四个选项，默认值向后兼容（老 Group 无值时按 anthropic 处理）
- [ ] 选择持久化在 Group 的 `extra` 内，无 schema 迁移
- [ ] 生成的 provider 的 `api` 字段随选择变化
- [ ] 有测试直接断言：anthropic 选项下 `baseUrl` 无版本后缀，OpenAI 选项下有版本前缀
- [ ] 该测试的注释写明「pi 官方文档示例有误，以源码常量为准」，防止后人反向修改
- [ ] 用户在 UI 上无处可输入 URL
- [ ] 新增文案 8 语言齐，`check-i18n` 绿
- [ ] `cargo test` / `cargo clippy` / `yarn test` / `yarn build` 全绿
