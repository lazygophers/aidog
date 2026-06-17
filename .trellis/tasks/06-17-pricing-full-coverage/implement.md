# Implement — Pricing 全平台覆盖 subtask 划分

> 优先级靠 REGISTRY 顺序 (first-party → litellm → openrouter) + `_merge._ensure_platform_pricing` 首次非空, **不改 Rust resolve_price**。

## G1 — 骨干 (并行, 无互相依赖)

### T1 litellm 骨干 scraper 新增

- 目标: 新增 `scrapers/litellm.py`, 抓 LiteLLM raw JSON, 按 prefix 映射 platform_type 作兜底骨干
- 产出: `scrapers/litellm.py` (fetch + PREFIX_MAP) + `scrapers/__init__.py` REGISTRY 登记 (位置: first-party 之后, openrouter 之前)
- 验证: `uv run python -c "import asyncio,scrapers.litellm as l; print(len(asyncio.run(l.fetch())))"` 非空 + `make prices-sync` litellm 出数
- 资源: prd.md prefix 映射表; LiteLLM URL `raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json`; `fetchutil.get_json`; `schema.{ModelEntry,PlatformPricing}`
- 依赖: 无 (REGISTRY 顺序在 T2 openrouter 之前即可)

### T2 openrouter.py 修 per-platform

- 目标: `openrouter.py:87` PREFIX_MAP 命中模型补 `pricing[<platform_type>]` (openrouter 价兜底, litellm 未覆盖的 siliconflow/novita/stepfun 路由模型受益)
- 产出: `scrapers/openrouter.py` 修 (pricing dict 加 platform key)
- 验证: `make prices-sync` openrouter 出数 + 模型 per-platform pricing 含 platform_type key (不只 openrouter)
- 资源: `openrouter.py:17-30 PREFIX_MAP`; prd.md
- 依赖: T1 (REGISTRY 顺序 litellm 在 openrouter 前, openrouter 价劣后不覆盖 litellm)

## G2 — first-party (并行, 依赖 G1 REGISTRY 顺序)

### T3 anthropic.py 写

- 目标: 抓 anthropic 一手价, 覆盖 litellm 兜底
- 产出: `scrapers/anthropic.py` (替换 return {})
- 验证: anthropic 出数 + `make prices-sync` 不报空跳过
- 资源: anthropic.com/pricing / 文档; WebFetch 试静态表; Claude/GPT 模型族
- 依赖: G1 (REGISTRY 在 litellm 前)

### T4 kimi.py 调研+写

- 目标: 调研 platform.moonshot.cn 抓取可行性, 能抓则写 first-party
- 产出: `scrapers/kimi.py` 实现 或 `需要:` 标记 (JS 渲染抓不到)
- 验证: kimi 出数 或 标 `需要:` 由 main 问用户
- 资源: platform.moonshot.cn/docs/pricing; WebFetch
- 依赖: G1

### T5 minimax.py 调研+写

- 目标: 同 T4, minimax 平台
- 产出: `scrapers/minimax.py` 或 `需要:`
- 验证: 同 T4
- 资源: platform.minimaxi.com/document/Price; WebFetch
- 依赖: G1

## G3 — 阻塞 (需用户供源)

### T6 siliconflow + novita + stepfun

- 目标: 三平台无 LiteLLM prefix 覆盖, 官方页 JS/鉴权, 需用户供源后写 first-party
- 产出: 待用户供源 → 各 scraper 实现; 或确认走 openrouter 兜底 (T2 已覆盖其路由模型)
- 验证: 三平台出数 或 用户确认 openrouter 兜底
- 资源: **用户供源** (prd.md `需要:` 段); openrouter.py PREFIX_MAP 无此三平台 (T2 补 openrouter 价兜底路由模型)
- 依赖: 用户回复

## 全局 check (G1+G2 完成后)

- `make prices-sync` 实跑: 11 平台出数统计 (除 G3 阻塞)
- `cargo test` (db.rs resolve_price/apply_context_tier 不破)
- `data/models.json` 各平台 per-platform pricing 条目 ≥ 1 (除 G3)
- 不提交 data/models.json (生成产物)
