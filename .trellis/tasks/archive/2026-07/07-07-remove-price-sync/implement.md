# implement.md: 移除 Python 价格聚合管线

## 执行层

- 载体: main 同步执行（纯删除 + 文档编辑，无 subagent）
- worktree: 无（改动局部）
- 并行: 串行

## 改动清单（按顺序）

### 步骤 1 — 删 `scripts/pricing/` 整目录

```bash
git rm -r scripts/pricing/
```

含 25M（主要是 `.venv/`）：aggregate.py / schema.py / fetchutil.py / util.py / pyproject.toml / uv.lock / README.md / scrapers/(15 个 .py) / .venv/ / __pycache__/。

验证: `ls scripts/pricing/ 2>&1 | grep -i "no such"` 有命中。

### 步骤 2 — 删 Makefile `prices-sync` target

定位: `Makefile:62-66`（`##@ Pricing` 段头 + `.PHONY: prices-sync` + target body + 空行）。

删后 `##@ Help` 段紧接 `install` target。验证: `grep -n "prices-sync\|Pricing" Makefile` 无命中。

> 注意: 仓库有 `Makefile` + `makefile` 两份（mac 默认大小写不敏感，实际同一文件）。改一次即可。

### 步骤 3 — 清理 Rust docstring 悬空引用 + 更新 `.wiki/modules/pricing.md`

**3a.** `src-tauri/src/gateway/price_sync.rs:4` docstring `schema 见 scripts/pricing/schema.py（...）` → 改为 `schema 见 src-tauri/defaults/models.json（人工维护，顶层 models 对象，每项含 input/output/cache_read pricing + max_tokens/context_window）`。

**3b.** `.wiki/modules/pricing.md` 当前架构图把 `scripts/pricing/` 画成维护源。改为:

当前架构图把 `scripts/pricing/` 画成维护源。改为:

- 架构图顶层 `scripts/pricing/` 层整删
- `src-tauri/defaults/models.json` 标注「人工维护（开发者手编，push GitHub master）」
- 保留 `price_sync.rs` 远端同步层 + `resolve_price` 回退链段（不变）
- 「关键决策」表删「入口 Makefile prices-sync 唯一」「文件粒度每平台 scraper」「主干 OpenRouter」「精选源 deepseek/gemini」4 行；保留「信源 GitHub raw」「max_tokens 出站 cap」相关行
- 「反踩坑」段整删（http.py 遮蔽 / __pycache__ 误提交都是 scripts/pricing 专属坑）

### 步骤 4 — 清理 `.claude/skills/aidog-price-source-update/SKILL.md`

frontmatter:
- `description`: 删「first-party scraper / LiteLLM / OpenRouter 三层」相关；改为聚焦「resolve_price 回退链 / context_tiers 阶梯计费 / price_sync 远端拉取 / 出站 cap / models.json 人工维护」。触发词删「first-party scraper / LiteLLM / OpenRouter」。
- `paths`: 删 `scripts/pricing/**`（目录已不存在）；保留 `price_sync.rs` / `estimate.rs` / `db.rs`。

正文:
- 「架构铁律」第 2 条（三层信源 REGISTRY 顺序）整删
- 「架构铁律」第 4 条（first-party scraper 覆盖清单）整删
- 「架构铁律」第 3 条改为「信源 = models.json（人工维护，push GitHub）+ app 远端拉取 + 出站 cap」
- 「三层信源架构」表整段删
- 「任务 A: 刷新过期价」段: 删 `make prices-sync` 指引；改为「手编 models.json 后 push master，app 下次同步拉取」
- 「任务 B/C: 写 first-party scraper / 注册 REGISTRY」整段删
- 保留: resolve_price 回退链 / context_tiers 阶梯计费 / price_sync 远端拉取链路 / 出站 cap / max_tokens 段

### 步骤 5 — Rust 门禁回归（零改动，确认无连带破坏）

```bash
cd src-tauri && cargo build && cargo clippy && cargo test
```

预期: price_sync.rs / commands/price.rs / models/price.rs 全保留，无编译错误。

### 步骤 6 — 前端构建

```bash
yarn build
```

## 自检

`✅ lint=? type=? test=? TODO=? 验收物=scripts/pricing 删除 + Makefile/wiki/skill 清理 + cargo/yarn 门禁过`

## 失败处理

- 步骤 5 cargo 报错: 排查是否有遗漏的 `scripts/pricing` 引用（理论上 Rust 不依赖 Python 工程，步骤 3a 已清 price_sync.rs docstring 悬空引用）。
- 步骤 6 yarn build 报错: 前端不应有任何 `scripts/pricing` 引用（前端不碰 Python）；若报，排查 ts import。
