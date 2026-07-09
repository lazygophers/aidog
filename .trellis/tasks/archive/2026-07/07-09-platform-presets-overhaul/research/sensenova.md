# Research: sensenova（商汤日日新 SenseNova）

- **Query**: 核对 sensenova 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | openai: `https://token.sensenova.cn/v1`（codex_tui）<br>anthropic: `https://token.sensenova.cn`（claude_code） |
| models.default | default: sensenova-6.7-flash-lite |
| model_list | sensenova-6.7-flash-lite, sensenova-u1-fast, deepseek-v4-flash |

## 官方文档列出值

### Source
- 文档：https://platform.sensenova.cn/document
- 定价：https://platform.sensenova.cn/pricing（**404**）

### 官方模型
`需要: SenseNova 官方模型清单与定价有效 URL`（pricing 404；document 页未抓取，推测 SPA）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| source_urls.pricing | `/pricing` | **404** | **更新 pricing URL** |
| base_url `token.sensenova.cn` | 单域名双协议 | 需核实（推测：商汤 token 网关） | `需要: SenseNova OpenAI/Anthropic 兼容端点官方说明` |
| model_list 含跨厂商 deepseek-v4-flash | 有 | 推测：商汤网关也转发第三方 | 维持（与 doubao 同模式） |
| model_list 缺 sensenova 其他版本（6.6 / 6.5 / u1 以外） | 无 | `需要: SenseNova 全模型矩阵` | 待官方文档核实 |
| anthropic base_url 无路径后缀 | 仅根域名 | 需核实（多数厂商有 `/anthropic` 路径） | `需要: anthropic 端点路径` |

## 补齐建议

1. **修 source_urls.pricing**（404）。
2. 核对 sensenova 模型清单（标 `需要`）。
3. 核对 anthropic 端点路径（无 `/anthropic` 后缀异常）。

## Caveats

- SenseNova 文档全 SPA + pricing 失效，无法静态验证。
- **优先级低**：用户未报缺失，JSON 三字段非空。
- `需要: SenseNova 文档与定价页有效 URL + 模型矩阵`。
