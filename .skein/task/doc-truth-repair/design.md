# 设计

## 策略

中文文档从“产品介绍”改成“源码事实索引”。每页只保留可由当前仓库证明的内容：页面入口、Tauri command/API 数据源、关键边界、验证命令。

## 改动面

- `docs/docs/zh/**/*.mdx`：删除推测性 UI 细节和营销描述，改为源码可核验事实。
- `docs/theme/fixtures/productDemo.ts`：保留固定脱敏 fixture，但文档明确 fixture 不是实时截图。
- 不改产品源码，除非发现默认端口或接入契约再次不一致。

## 测试接缝

- `yarn check:docs`
- `yarn workspace aidog-docs build`
- `node scripts/gen-command-docs.mjs --check`

## 风险

文档覆盖面会变窄，但事实准确性优先。后续如要扩写，每段必须补源码出处。
