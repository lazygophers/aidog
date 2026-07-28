# SKEIN 规则库总索引

三层: **core** 常驻注入 (SessionStart) · **recall** 按需召回 (planning `recall <query>`) · **external** 外部参考 (纯手动 CLI 检索, 不入 hook)。

| layer | 条数 | 类目分布 | 索引 |
|---|---|---|---|
| core | 12 | build(5), i18n(7) | [core/index.md](core/index.md) |
| recall | 487 | arch(116), build(51), cross-layer(10), db(20), domain(77), encoding(4), frontend(46), git(6), i18n(9), ops(5), optimization(5), proxy(39), reuse(6), shadcn(49), skein(1), style(11), test(12), testing(5), theme(5), ts-rust-boundary(10) | [recall/index.md](recall/index.md) |
| external | 0 | - | [external/index.md](external/index.md) |
