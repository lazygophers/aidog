# PRD: 移除 Python 价格聚合管线 (make prices-sync)

## 背景

aidog 模型定价现有三层架构:

1. **Python 聚合层** (`scripts/pricing/` + Makefile `prices-sync`): 15 个平台 scraper 抓官方定价页 → `aggregate.py` 合并 → 写 `src-tauri/defaults/models.json`。开发者本地手动跑。
2. **Rust 远端同步层** (`price_sync.rs`): app 运行期从 GitHub jsDelivr/raw 拉 `models.json` → upsert 进 SQLite `model_price` 表。含手动「立即同步」按钮 + 自动同步开关。
3. **价格消费层** (`resolve_price` / `calc_est_cost` / PricingTab 列表): 读 DB 估算花费。

用户决定: **移除第 1 层 Python 聚合管线**，`models.json` 改为人工维护（开发者手编或一次性脚本生成后 push GitHub），app 远端同步链路保留。

## 目标 (axis A)

- 删除 `scripts/pricing/` 整个 Python 工程 + Makefile `prices-sync` target
- 清理引用该管线的文档 / skill 描述
- **不动**: `price_sync.rs` / `model_price_sync` command / PricingTab sync UI / `PriceSyncSettings` / `models.json` 数据文件 / `model_price` 价格表 / 估算链路

## 非目标 (out of scope)

- 不删 `price_sync.rs`（app 远端同步保留）
- 不删 `src-tauri/defaults/models.json`（数据保留，仍为 GitHub 唯一事实源）
- 不删 PricingTab sync UI / fallback 价 UI
- 不改 `PriceSyncSettings` / 兜底价逻辑
- 不新增手动增删改 model_price 的 command / UI（用户确认「同步按钮即更新机制，已存在」）
- 不动 `.gitignore` `__pycache__/` + `*.pyc` 两条（通用 Python 规则，留作未来 Python 工程用）

## 交付 (axis B)

| # | 交付物 | 验收 |
|---|--------|------|
| D1 | `scripts/pricing/` 整目录删除（含 aggregate.py / schema.py / fetchutil.py / util.py / pyproject.toml / uv.lock / README.md / scrapers/*.py / .venv/ / __pycache__/） | `ls scripts/pricing/ 2>&1` 报「No such file or directory」 |
| D2 | Makefile 删 `##@ Pricing` 段 + `prices-sync` target（行 62-66） | `grep -n "prices-sync\|Pricing" Makefile` 无命中；`make help` 不显示 prices-sync |
| D3 | `price_sync.rs:4` docstring 悬空引用清理 + `.wiki/modules/pricing.md` 更新：架构图删 scripts/pricing 层，models.json 标注「人工维护」，保留 price_sync.rs 远端同步层描述 | price_sync.rs 无 `scripts/pricing` 引用；wiki 无 `scripts/pricing/` / `make prices-sync` / `aggregate.py` 引用 |
| D4 | `.claude/skills/aidog-price-source-update/SKILL.md` 清理：删 scraper 编写 / REGISTRY / first-party 覆盖清单 / `_ensure_platform_pricing` 段；保留 resolve_price 回退链 / context_tiers / price_sync 远端拉取 / 出站 cap 段 | skill 不再指引写 scraper；触发词删「first-party scraper / LiteLLM / OpenRouter」相关 |
| D5 | `cargo build` + `cargo clippy` + `cargo test` 通过（Rust 侧零改动，门禁回归） | 三命令 exit 0 |
| D6 | `yarn build` 通过（前端零改动） | exit 0 |

## 调度

单 task，串行（纯删除 + 文档清理，无并行组）。无 worktree（改动局部、无生产代码变更）。

```mermaid
flowchart LR
  D1[删 scripts/pricing/] --> D2[删 Makefile target]
  D2 --> D3[更新 wiki]
  D3 --> D4[清理 skill]
  D4 --> D5[Rust 门禁]
  D5 --> D6[前端构建]
```

## 风险

- **低**: skill `aidog-price-source-update` 删 scraper 段后剩余内容是否仍连贯 → 处理: 保留 resolve_price / context_tiers / price_sync 远端拉取段，重组段落顺序保证连贯。
- **低**: wiki 引用 `[[pricing-resolve-single-source]]` 等 memory 链接 → 保留（指向仍有效的 resolve_price 知识）。
