# Research: qianfan（百度千帆）

- **Query**: 核对 qianfan 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | anthropic: `https://qianfan.baidubce.com/anthropic/coding`（claude_code） |
| models.default | default: ernie-4.5-turbo |
| model_list | ernie-5.1, ernie-5.0, ernie-4.5-turbo-vl, ernie-4.5-turbo, ernie-x1-turbo, ernie-x1.1-preview |

## 官方文档列出值

### Source
- 千帆文档：https://cloud.baidu.com/doc/qianfan-api/
- 定价：https://cloud.baidu.com/doc/qianfan-api/s/7m0g3yofv（**抓取 404**，URL 失效）

### 官方模型
`需要: qianfan-api 官方模型清单与定价页有效 URL`（curl 抓取的定价页 404，文档站 SPA 无法静态提取）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| source_urls.pricing | `cloud.baidu.com/doc/qianfan-api/s/7m0g3yofv` | **404 失效** | **必须更新 pricing URL** |
| base_url | `qianfan.baidubce.com/anthropic/coding` | 需核实 | `需要: 千帆 anthropic 兼容端点官方说明` |
| model_list（ernie 5.1 / 5.0 / 4.5-turbo / x1 系列） | 齐全 | 需核实 | `需要: ERNIE 模型最新清单官方链接`（推测：JSON 较全，ernie-5.x 是最新主线） |
| 仅 anthropic 单协议 | 无 openai endpoint | 需核实千帆是否开 openai 兼容 | `需要: 千帆 OpenAI 兼容端点说明`（推测：仅 anthropic-compat 路径开放给 coding 工具） |

## 补齐建议

1. **修 source_urls.pricing**（当前 404）。建议替换为千帆定价主页根：`https://cloud.baidu.com/product/wenxinworkshop/` 或在文档站内重新检索。
2. 核对 ernie 模型清单（curl 抓不到，标 `需要`）。

## Caveats

- 整个 qianfan 协议数据无法用 curl 静态验证（百度文档站全 SPA + 登录墙）。所有结论标 `需要: 官方文档链接`。
- **优先级低**：用户未报 qianfan 缺失问题，JSON 内部完整性自洽（endpoints/models/model_list 三字段非空）。
