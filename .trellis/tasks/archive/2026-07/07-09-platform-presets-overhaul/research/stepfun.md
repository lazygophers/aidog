# Research: stepfun / stepfun_en（阶跃星辰 国内 / 国际）

- **Query**: 核对 stepfun 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | stepfun（国内） | stepfun_en（国际） |
|---|---|---|
| anthropic base_url | `https://api.stepfun.com/step_plan` | `https://api.stepfun.ai/step_plan` |
| openai base_url | `https://api.stepfun.com/v1` | `https://api.stepfun.ai/v1` |
| models.default.default | step-3.7-flash | step-3.7-flash |
| model_list | step-3.7-flash, step-3.5-flash, step-3.5-flash-2603, step-1o-turbo-vision | 同 |

## 官方文档列出值

### Source
- 国内：https://platform.stepfun.com/docs（pricing 子路径 `/docs/product/price` **抓取 404**）
- 国际：https://platform.stepfun.ai/docs（pricing 同样 404）

### 官方模型
`需要: stepfun 官方模型清单与定价页有效 URL`（404；文档 SPA + 部分路径失效）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| source_urls.pricing | `/docs/product/price` | **404** | **更新 pricing URL**（文档站路径已改） |
| anthropic base_url `/step_plan` | 套餐专用路径 | 需核实 | `需要: stepfun anthropic 兼容端点 official 路径` |
| model_list（step-3.7/3.5-flash, 1o-turbo-vision） | 简洁 | 需核实 | `需要: step 模型完整清单`（推测：step-3.7-flash 是最新主线，1o 是多模态/视觉线） |
| 国际站与国内站同模型 | 同 | ✅ 镜像 | 维持 |

## 补齐建议

1. **修 source_urls.pricing**（404）。重定向到 `https://platform.stepfun.com/docs/pricing` 或文档根。
2. 核对 step 模型清单（curl 抓不到，标 `需要`）。

## Caveats

- stepfun docs SPA + pricing 路径变更，全部数据需人工或浏览器二次核实。
- **优先级中**：用户未报 stepfun 缺失，JSON 自洽；但 pricing URL 失效需修。
