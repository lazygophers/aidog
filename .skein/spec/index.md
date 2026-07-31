# SKEIN 规则库总索引

三层: **core** 常驻注入 (SessionStart) · **recall** 按需召回 (planning `recall <query>`) · **external** 外部参考 (纯手动 CLI 检索, 不入 hook)。

| layer | 条数 | 类目分布 | 索引 |
|---|---|---|---|
| core | 67 | arch(14), cross-layer(7), db(21), domain(5), frontend(3), i18n(4), perf(4), proxy(9) | [core/index.md](core/index.md) |
| external | 0 | - | [external/index.md](external/index.md) |
| recall | 244 | arch(86), build(28), cross-layer(10), db(8), domain(45), ops(8), optimization(35), skein(24) | [recall/index.md](recall/index.md) |
| rules | 0 | - | [rules/index.md](rules/index.md) |
