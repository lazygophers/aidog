# 补全 minimax(+minimax_en) model_list+endpoints 全部官方信息

## Goal

MiniMax（国内 `minimax` + 国际 `minimax_en` 两块）官方自研模型平台。preset 现有 5 个 M 系列模型 + `MiniMax-M3` 主力，缺 3 个 highspeed 变体（M2.7/M2.5/M2.1-highspeed，research 确认是官方独立 model id）；`models.default` 仅单档 default，需按语义档位扩多档。同时改写 desc 去除已废弃的 "abab" 表述（research 判定 abab 系列已被 M 系列完全取代，两站均无具体 abab 模型 id）。endpoints/source_urls 保留。两块对称改。

## Research References

- [`research/minimax-models.md`](research/minimax-models.md) — 8 个 stable 模型 + 3 个 highspeed 变体（独立 model id）；abab 已废弃；国内/国际站端点对称验证（401 探测确认路径正确）；M3 是 2026-06-01 发布的最新旗舰

## Requirements

### 1. endpoints.default（2 端点 × 2 块，保留不动）

两块端点均已验证（401 探测 + 官方文档对称性确认）：

```json
// minimax（国内 api.minimaxi.com）
"endpoints": {
  "default": [
    { "protocol": "openai", "base_url": "https://api.minimaxi.com/v1", "client_type": "codex_tui", "coding_plan": false },
    { "protocol": "anthropic", "base_url": "https://api.minimaxi.com/anthropic", "client_type": "claude_code", "coding_plan": false }
  ]
}
// minimax_en（国际 api.minimax.io）
"endpoints": {
  "default": [
    { "protocol": "openai", "base_url": "https://api.minimax.io/v1", "client_type": "codex_tui", "coding_plan": false },
    { "protocol": "anthropic", "base_url": "https://api.minimax.io/anthropic", "client_type": "claude_code", "coding_plan": false }
  ]
}
```

### 2. model_list.default（8 模型，字符串数组，两块一致）

按 research release-notes 时间倒序 + highspeed 紧随原版：

```json
"model_list": {
  "default": [
    "MiniMax-M3",
    "MiniMax-M2.7",
    "MiniMax-M2.7-highspeed",
    "MiniMax-M2.5",
    "MiniMax-M2.5-highspeed",
    "MiniMax-M2.1",
    "MiniMax-M2.1-highspeed",
    "MiniMax-M2"
  ]
}
```

排除：`MiniMax-Text-01` / `MiniMax-VL-01`（遗留）、`abab6.5/6.5s/7`（已废弃，两站均无）、speech/hailuo/music/image（非文本对话主线）。

### 3. models.default（档位名 key → model id string，两块一致）

```json
"models": {
  "default": {
    "default": "MiniMax-M3",
    "sonnet": "MiniMax-M2.7",
    "coder": "MiniMax-M2.5",
    "fast": "MiniMax-M2.7-highspeed"
  }
}
```

档位依据：
- `default`：MiniMax-M3（2026-06-01 旗舰，1M 上下文，Frontier multimodal coding model）
- `sonnet`：MiniMax-M2.7（递归自我改进起点，高层次通用）
- `coder`：MiniMax-M2.5（research 明确"编程与重构优化，巅峰性能"）
- `fast`：MiniMax-M2.7-highspeed（research 明确"相同性能更快推理"，highspeed 语义 = fast）

### 4. desc（改写，去除 abab 表述）

research 判定 abab 系列已被 M 系列完全取代、两站均无具体 abab 模型 id。8 语言改写为"Hailuo M 系列"：

| 语言 | 新 desc |
|------|---------|
| en-US | MiniMax API for Hailuo M-series models |
| zh-Hans | MiniMax API, 海螺 M 系列模型 |
| ar-SA | واجهة MiniMax لنماذج سلسلة Hailuo M |
| fr-FR | API MiniMax pour les modèles série Hailuo M |
| de-DE | MiniMax-API für Hailuo M-Serien-Modelle |
| ru-RU | API MiniMax для моделей серии Hailuo M |
| ja-JP | MiniMax API、海螺 M シリーズモデル |
| es-ES | API de MiniMax para modelos serie Hailuo M |

### 5. source_urls（保留）

- minimax：docs https://platform.minimaxi.com/document/Announcement，pricing https://platform.minimaxi.com/document/Price
- minimax_en：docs https://platform.minimax.io/document/Announcement，pricing https://platform.minimax.io/document/Price

## Acceptance Criteria

- [ ] 两块 `model_list.default` 均为上述 8 模型数组（JSON 合法、无重复、与 highspeed 变体齐备）
- [ ] 两块 `models.default` 均为 4 档位 key→string（default/sonnet/coder/fast）
- [ ] 两块 desc 8 语言均改写为"Hailuo M 系列"，无残留 "abab"
- [ ] 两块 endpoints/source_urls/name/homepage/logo_url/client_type 不动
- [ ] `python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));[print(k,len(d['protocols'][k]['model_list']['default']),d['protocols'][k]['models']['default'],len(d['protocols'][k]['endpoints']['default'])) for k in ['minimax','minimax_en']]"` 输出两行 `8 {'default': 'MiniMax-M3', 'sonnet': 'MiniMax-M2.7', 'coder': 'MiniMax-M2.5', 'fast': 'MiniMax-M2.7-highspeed'} 2`
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] 不动其他协议块、不动顶层 version/last_updated

## Out of Scope

- 上下文窗口 / STATIC_MODEL_IDS / peak_hours
- `coding_plan` 分支（两块均无 cp 分支，仅 default；endpoint 级 `coding_plan: false` flag 保留不动）
- speech/hailuo/music/image 非文本主线模型
- 国内站模型列表的 API key 实测验证（无 key，依赖 research 文档结论）
- 其他协议块

## Technical Notes

- 真值源：`protocols.minimax` + `protocols.minimax_en`（两块对称改）
- 数据来源：research/minimax-models.md（国际站 release notes + 控制台 + 401 探测）
- id 格式：`MiniMax-M<x>` + `-highspeed` 后缀（PascalCase）
- 数据强度：**强**（国际站 release notes 明确、highspeed 为官方反引号标注的独立 id、两站端点 401 验证）
- 国内站模型清单未决项：research 标注国内站文档需登录/JS 渲染无法完全获取，按国际站对称处理（两站响应一致）
