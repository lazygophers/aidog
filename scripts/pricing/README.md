# aidog model pricing aggregate

唯一入口（Makefile）：

```bash
make prices-sync
```

流程：并发跑 `scrapers/` 全部平台 scraper → 合并去重（同名模型 top-level 价取首次非 0、max_* 取首次非空、per-platform 价写入 `pricing[<platform>]`）→ pydantic schema 校验 → 原子写 `data/models.json`。

`data/models.json` 是 app 端 price/max_tokens 的**唯一真实信源**（app 从 GitHub raw 定时拉取，见 Rust `price_sync.rs`）。

## 数据源

| scraper | 类型 | 说明 |
| --- | --- | --- |
| `openrouter.py` | 骨干 live | OpenRouter `/api/v1/models`（单次抓取覆盖 openai/anthropic/gemini/deepseek/glm/kimi/minimax/stepfun 等多平台模型） |
| `deepseek.py` | 一手策定 | DeepSeek 官方价（CNY/M→$/token），已核实 |
| `gemini.py` | 一手策定 | Google Gemini 官方价（USD/M），已核实 |
| `openai.py` / `anthropic.py` / `glm.py` / `kimi.py` / `minimax.py` / `siliconflow.py` / `novita.py` / `stepfun.py` | 占位 | 官方页 JS 渲染暂无可解析源，返空由 openrouter 兜底；接入后填充一手价 |

> 价格单位统一 $/token（est_cost 直接乘 token 数）。CNY 计价平台用 `util.CNY_PER_USD` 转 USD。

## 添加/维护一手价

每个平台一个独立文件 `scrapers/<platform>.py`，暴露：

```python
async def fetch() -> dict[str, ModelEntry]:
    ...
```

返回值在 `scrapers/__init__.py` REGISTRY 中登记即可被 `make prices-sync` 自动聚合。
