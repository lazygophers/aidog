# SKEIN 规则库总索引

三层: **core** 常驻注入 (SessionStart) · **recall** 按需召回 (planning `recall <query>`) · **external** 外部参考 (纯手动 CLI 检索, 不入 hook)。

| layer | 条数 | 类目分布 | 索引 |
|---|---|---|---|
| core | 85 | arch(13), cross-layer(12), db(22), domain(5), frontend(3), i18n(8), perf(4), proxy(18) | [core/index.md](core/index.md) |
| external | 0 | - | [external/index.md](external/index.md) |
| recall | 555 | arch(97), build(58), db(5), domain(74), frontend(80), git(7), i18n(15), ops(23), optimization(43), proxy(26), reuse(5), shadcn(48), skein(24), style(9), test(14), testing(15), ts-rust-boundary(12) | [recall/index.md](recall/index.md) |
| rules | 0 | - | [rules/index.md](rules/index.md) |
