# Design — 模型信息中心化

## 架构总览

```
Python scrapers (Makefile 单入口)          app (Tauri)
┌──────────────────────────────┐      ┌─────────────────────────────┐
│ scripts/pricing/             │      │ 启动 + 定时(auto_sync)       │
│  scrapers/<provider>.py ×11  │      │   ↓ fetch raw URL            │
│  aggregate.py ──merge──→     │      │ raw.githubusercontent.com/   │
│  pyproject.toml (uv)         │      │   lazygophers/aidog/master/  │
└──────────────┬───────────────┘      │   data/models.json           │
               │ make prices-sync     │   ↓ parse → upsert           │
               ↓ git commit/push      │ model_price 表(+max_*列)     │
       data/models.json               │   ↓ resolve_price(不变)       │
       (master 分支唯一信源)           │   ↓ get_model_max_output     │
                                      │ est_cost / router 裁剪 / 展示 │
                                      └─────────────────────────────┘
```

## 决策汇总 (Q&A)

| # | 决策 |
| --- | --- |
| Q1 | **A 纯只读镜像**: GitHub JSON 整表替换本地 model_price; 移除手动 upsert/delete + import_export model_price scope |
| Q2 | **A 单文件** `data/models.json` |
| 入口 | **Makefile 单 target** `make prices-sync` 跑全部 11 scraper + 合并 (唯一入口, 无 per-platform target) |
| Q3 | **A+B 保守**: 展示 max_tokens/context + 出站仅当客户端值**超过**模型 max_output_tokens 时裁剪; 客户端未传不注入; 模型无上限不裁剪 |
| Q4 | **A 加列**: model_price 加 max_input_tokens/max_output_tokens/context_window (migration 008) |
| Q5 | **scripts/pricing/** + uv; 11 平台每平台独立 scraper 文件 |
| Q6 | **A 复用定时器+按钮**; URL=raw.githubusercontent.com/lazygophers/aidog/master/data/models.json; source litellm→github |

## data/models.json Schema

```jsonc
{
  "version": 1,                          // schema 版本, app 校验
  "generated_at": "2026-06-16T12:00:00Z",
  "models": {
    "deepseek-chat": {
      "default_platform": "deepseek",
      "input_cost_per_token": 0.00000027,      // $/token
      "output_cost_per_token": 0.00000110,
      "cache_read_input_token_cost": 0.00000007,
      "max_input_tokens": 64000,
      "max_output_tokens": 8192,
      "context_window": 64000,
      "pricing": {                            // per-platform override (保留 resolve_price 回退链)
        "deepseek": { "input_cost_per_token": 0.00000027, "output_cost_per_token": 0.00000110 },
        "openrouter": { "input_cost_per_token": 0.00000028, "output_cost_per_token": 0.00000112 }
      }
    }
  }
}
```

字段与现有 price_data JSON 同构 + 顶层加 max_* 三字段。resolve_price 回退链（pricing[platform_type] > top_level > default_platform > fallback）**不变**。

## DB 变更 (migration 008)

```sql
ALTER TABLE model_price ADD COLUMN max_input_tokens INTEGER;
ALTER TABLE model_price ADD COLUMN max_output_tokens INTEGER;
ALTER TABLE model_price ADD COLUMN context_window INTEGER;
```
- source 值: `litellm` → `github` (同步写 source="github")
- price_data JSON 仍存全量原始 (向后兼容 resolve_price 的 price_data_to_summary parse)
- 新增 max_* 列由 sync 时从 JSON parse 填充

## Rust 改动

### 移除
- `price_sync.rs` 全文件 + `mod.rs:12` 挂载
- commands: `model_price_upsert` / `model_price_delete` / `model_price_sync`(改为 github 同步) 重命名
- PriceSyncSettings.fallback_* **保留** (未覆盖模型兜底)
- import_export: SCOPE_MODEL_PRICE + collect/apply/container 的 model_price 分支

### 新增/改
- `price_sync.rs` → 重写为 `model_info_sync.rs`(或保留文件名改实现): fetch GitHub raw JSON → parse → upsert model_price(source="github", 含 max_* 列)
- URL 常量: `raw.githubusercontent.com/lazygophers/aidog/master/data/models.json`
- db.rs: `upsert_model_price` 签名加 max_* 参数; 新增 `get_model_max_output_tokens(db, model_name) -> Option<u32>` (热路径, 配 DbCache)
- router.rs (持 db): 选定平台后、convert 前, 查 model max_output_tokens; 若 `req.max_tokens > max_out`, 裁剪 = max_out; req 无 max_tokens / 模型无上限 → 不动
- resolve_price: **不变**

### 入站 parse (修 CONVERSION_TODO gap, 仅做 max_tokens)
- anthropic.rs / gemini.rs parse: 提取 max_tokens 填入 ChatRequest.max_tokens (现 anthropic serialize 硬编 4096)

## 前端改动

### PricingTab.tsx
- **保留**: 列表/搜索/筛选/分页/同步按钮/auto_sync/interval/fallback price 设置
- **移除**: 手动 upsert 表单 / delete 按钮
- **新增列**: max_input_tokens / max_output_tokens / context_window
- 文案: "从 LiteLLM 同步" → "从 GitHub 同步模型信息"
- ModelPriceSummary 加 max_* 字段

### api.ts
- 移除 modelPriceApi.upsert / delete
- PriceSyncSettings 保留; source 类型 "litellm"→"github"

## Python 工程 (scripts/pricing/)

```
scripts/pricing/
  pyproject.toml          # uv, deps: httpx/bs4/selectolax/pydantic
  Makefile 里: make prices-sync → uv run python aggregate.py
  aggregate.py            # 唯一聚合入口: 调各 scraper → 合并 → 校验 → 写 data/models.json
  scrapers/
    deepseek.py
    openai.py
    anthropic.py
    gemini.py
    glm.py
    kimi.py
    minimax.py
    siliconflow.py
    openrouter.py
    novita.py
    stepfun.py
  schema.py               # pydantic Model (与 data/models.json schema 对齐, 单一事实源)
```
每 scraper 独立文件, 各自抓官方定价页/API → 返回 `dict[model_name, ModelEntry]`。aggregate 合并去重 + version/generated_at → 原子写 data/models.json。

## 失败/边界
- **app 离线/拉取失败**: 保留本地旧 model_price 不清空 (upsert 不 DELETE); fallback_* 兜底 est_cost
- **JSON schema 版本不匹配**: app 校验 version, 不匹配跳过同步 + warn
- **Python scraper 某平台失败**: aggregate 跳过该平台 + log, 不阻塞其他平台; 失败平台该次不更新(保留旧值)
- **模型无 max_output_tokens**: 不裁剪 (按 Q3 保守)
- **per-platform 同名模型冲突**: pricing override 优先于 top-level
