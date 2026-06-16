# Implementation Plan — 模型信息中心化

## PR 序列 (小步可验收)

### PR1: Python 定价工程 + data/models.json 首版
- `scripts/pricing/` 工程骨架: pyproject.toml(uv) + schema.py(pydantic) + aggregate.py + 11 scrapers/<provider>.py
- 根 Makefile 加 `prices-sync` target (唯一入口, `uv run python aggregate.py`)
- 各 scraper 抓官方定价 → aggregate 合并 → 写 `data/models.json` (version=1, generated_at)
- 验证: `make prices-sync` 跑通, data/models.json 含 ≥11 平台模型, schema 自校验
- 风险: 各平台定价页结构差异大 → scraper 可能需 httpx+selectolax/html 解析; 部分平台无公开价走占位

### PR2: Rust — migration 008 + 同步源换 GitHub
- migration `008_model_info_columns.sql`: model_price 加 max_input_tokens/max_output_tokens/context_window
- db.rs: `upsert_model_price` 加 max_* 参数; `price_data_to_summary` parse max_*; 新增 `get_model_max_output_tokens(db, model_name)` (配 DbCache 写时失效, memory: perf-hotpath-optimization)
- 重写 `price_sync.rs`: 删 LiteLLM fetch, 改 GitHub raw URL (`raw.githubusercontent.com/lazygophers/aidog/master/data/models.json`) + parse + upsert(source="github", 含 max_*)
- lib.rs: 重命名 `model_price_sync` → `model_info_sync` (或保留名改实现); 移除 `model_price_upsert` / `model_price_delete` commands
- models.rs: ModelPrice/ModelPriceSummary 加 max_* 字段; PriceSyncSettings.fallback_* 保留
- 验证: cargo build + cargo test (db/estimate) 绿; 同步命令拉取 master data/models.json 入库

### PR3: 移除 import_export model_price scope + 旧引用清理
- import_export/{collect,apply,mod,container}.rs: 删 SCOPE_MODEL_PRICE + model_price 分支 + scope 计数 + 冲突扫描
- mod.rs:12 仍保留 mod price_sync (已重写) / 或重命名 mod model_info_sync
- 验证: 导入导出其他 6 scope 不受影响; 旧 .aidogx 含 model_price 优雅忽略

### PR4: 前端 PricingTab 改造
- PricingTab.tsx: 移除手动 upsert 表单 + delete 按钮; 加 max_input/output_tokens/context_window 列; 文案 "LiteLLM"→"GitHub"
- api.ts: 移除 modelPriceApi.upsert/delete; ModelPriceSummary/PriceSyncSettings 加 max_* + source 类型
- i18n: 7 语言补 pricing 相关新 key (check-i18n.mjs 通过)
- 验证: yarn build 绿; 列表展示 max_tokens; 同步按钮拉 GitHub

### PR5: max_tokens 入站 parse + router 裁剪
- adapter/anthropic.rs parse: 提取 max_tokens 填 ChatRequest (现 serialize 硬编 4096)
- adapter/gemini.rs parse: 同上 (CONVERSION_TODO C1/C3)
- router.rs: 选定平台后 convert 前, `get_model_max_output_tokens(db, model)`; 若 `req.max_tokens.is_some() && req.max_tokens > max_out` → 裁剪 = max_out; 否则不动 (Q3 保守: 不注入默认)
- 验证: cargo test adapter + 新增 router max_tokens 裁剪单测; 超限请求被裁剪, 未传不被注入

## 依赖
PR1 ─┐
     ├→ PR2 (schema 对齐) → PR3 → PR4
     └→ PR5 (依赖 PR2 max_* 列)

PR1 与 PR2 schema 必须先对齐 (data/models.json schema = Rust ModelPrice 字段), 建议同会话内 PR1+PR2 串行。
PR3/PR4/PR5 可在 PR2 后并行 (改不同文件集)。

## 验收 (Definition of Done)
- [ ] `make prices-sync` 单入口跑通, 生成 data/models.json (≥11 平台)
- [ ] app 同步从 GitHub master 拉取, model_price 入库含 max_*
- [ ] PricingTab 展示 max_tokens/context, 无手动编辑入口
- [ ] est_cost 计算不变 (resolve_price 回退链保留, memory: pricing-resolve-single-source)
- [ ] 出站 max_tokens 超模型上限时裁剪, 未传不注入
- [ ] cargo clippy/test + yarn build + check-i18n.mjs 全绿
- [ ] import_export 移除 model_price scope, 其他 scope 不受影响
- [ ] cortex 落档 (架构决策: GitHub JSON 唯一信源 + max_tokens 消费)
