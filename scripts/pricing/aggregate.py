"""聚合入口 — 唯一调用点 (make prices-sync → uv run python aggregate.py)。

流程: 并发跑 REGISTRY 全部 scraper → 合并去重 → schema 校验 → 原子写 data/models.json。
某 scraper 失败: 跳过该平台 + 打印, 不阻塞其他平台 (保留旧值由调用方决定, 本进程不删既有条目)。
"""

from __future__ import annotations

import asyncio
import json
import sys
from pathlib import Path

from schema import ModelEntry, ModelsFile

# 仓根 = scripts/pricing 的上两级
REPO_ROOT = Path(__file__).resolve().parent.parent.parent
OUTPUT = REPO_ROOT / "data" / "models.json"

# 导入 scraper 注册表
sys.path.insert(0, str(Path(__file__).resolve().parent))
from scrapers import REGISTRY  # noqa: E402


async def run_scraper(module, platform: str) -> tuple[str, dict[str, ModelEntry] | None, str | None]:
    try:
        entries = await module.fetch()
        return platform, entries, None
    except Exception as e:  # noqa: BLE001 — 顶层聚合容错
        return platform, None, f"{type(e).__name__}: {e}"


async def main() -> int:
    print(f"[aggregate] 跑 {len(REGISTRY)} 个 scraper ...")
    results = await asyncio.gather(*(run_scraper(mod, plat) for mod, plat in REGISTRY))

    models: dict[str, ModelEntry] = {}
    ok = 0
    for platform, entries, err in results:
        if err is not None:
            print(f"  ✗ {platform}: FAIL ({err})")
            continue
        if not entries:
            print(f"  · {platform}: 空 (跳过)")
            continue
        for name, entry in entries.items():
            _merge(models, name, entry, platform)
        print(f"  ✓ {platform}: {len(entries)} 模型")
        ok += 1

    out = ModelsFile(models=models)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    tmp = OUTPUT.with_suffix(".json.tmp")
    tmp.write_text(out.model_dump_json(indent=2, exclude_none=True), encoding="utf-8")
    tmp.replace(OUTPUT)
    print(f"[aggregate] 完成: {ok}/{len(REGISTRY)} 平台, {len(models)} 模型 → {OUTPUT.relative_to(REPO_ROOT)}")
    return 0


def _merge(models: dict[str, ModelEntry], name: str, entry: ModelEntry, platform: str) -> None:
    """同名模型合并: top-level 价取首次非 0; per-platform 价写入 pricing[platform]; max_* 取首次非空。"""
    existing = models.get(name)
    if existing is None:
        # 标注 default_platform (若 entry 未指定)
        if entry.default_platform is None:
            entry.default_platform = platform
        models[name] = entry
        # 同时登记 per-platform 价
        _ensure_platform_pricing(entry, platform)
        return
    # 合并 max_* (取首次非空)
    for f in ("max_input_tokens", "max_output_tokens", "context_window"):
        if getattr(existing, f) is None and getattr(entry, f) is not None:
            setattr(existing, f, getattr(entry, f))
    # 合并 top-level 价 (取首次非 0)
    if existing.input_cost_per_token == 0 and entry.input_cost_per_token:
        existing.input_cost_per_token = entry.input_cost_per_token
    if existing.output_cost_per_token == 0 and entry.output_cost_per_token:
        existing.output_cost_per_token = entry.output_cost_per_token
    if existing.cache_read_input_token_cost == 0 and entry.cache_read_input_token_cost:
        existing.cache_read_input_token_cost = entry.cache_read_input_token_cost
    _ensure_platform_pricing(existing, platform)


def _ensure_platform_pricing(entry: ModelEntry, platform: str) -> None:
    pp = entry.pricing.get(platform)
    if pp is None:
        from schema import PlatformPricing

        pp = PlatformPricing()
        entry.pricing[platform] = pp
    if pp.input_cost_per_token is None and entry.input_cost_per_token:
        pp.input_cost_per_token = entry.input_cost_per_token
    if pp.output_cost_per_token is None and entry.output_cost_per_token:
        pp.output_cost_per_token = entry.output_cost_per_token
    if pp.cache_read_input_token_cost is None and entry.cache_read_input_token_cost:
        pp.cache_read_input_token_cost = entry.cache_read_input_token_cost


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
