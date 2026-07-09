# 补全 qianfan model_list+endpoints 全部官方信息

## Goal

百度千帆（Baidu Qianfan）。preset 现 `model_list.default=[]` + `models.default={}` + 单 anthropic `/anthropic/coding` 端点。**research 强度弱**：官方文档需登录/JS 渲染无法完全获取，模型 id 多为基于命名规范的推测，实际 API 调用名称未验证；`/v1` openai 端点实测 404（无公开 OpenAI 兼容端点）。本 task 保守补全：① 仅填 research "控制台确认"的 6 个 ERNIE 主线模型 id（小写化，标注未验证）；② `models.default` 仅设 `default` 档（主力兜底），不强扩多档（id 未验证，避免误导）；③ endpoints/desc/source_urls 全保留不动。

## Research References

- [`research/qianfan-models.md`](research/qianfan-models.md) — ERNIE 系列控制台确认的 6 主线模型（5.1/5.0/4.5 Turbo VL/4.5 Turbo/X1 Turbo/X1.1）；其余 id（ernie-4.5-128k-preview 等）均标"需验证"；/v1 openai 端点 404；无国际端点；认证方式推测 Bearer

## Requirements

### 1. endpoints.default（1 端点，保留不动）

research 实证 `/v1` openai 兼容端点返回 404（千帆无公开 OpenAI 兼容端点），仅保留现有 anthropic `/anthropic/coding` 端点：

```json
"endpoints": {
  "default": [
    { "protocol": "anthropic", "base_url": "https://qianfan.baidubce.com/anthropic/coding", "client_type": "claude_code" }
  ]
}
```

### 2. model_list.default（6 模型，字符串数组，保守小列）

仅取 research "控制台确认"段落的 6 个主线 ERNIE 模型（排除所有"推测/需验证"子型号如 ernie-4.5-128k-preview / ernie-speed-pro-128k / ernie-lite-128k 等）。id 小写化（对齐 research 推测的 API 调用格式），**标注未通过 API 实测验证**：

```json
"model_list": {
  "default": [
    "ernie-5.1",
    "ernie-5.0",
    "ernie-4.5-turbo-vl",
    "ernie-4.5-turbo",
    "ernie-x1-turbo",
    "ernie-x1.1-preview"
  ]
}
```

排除（research 标推测/需验证）：`ernie-4.5-128k-preview` / `ernie-4.5-turbo-128k` / `ernie-4.5-x1` / `ernie-speed-pro-128k` / `ernie-speed-128k` / `ernie-lite-128k` / `ernie-tiny-128k`；第三方聚合（deepseek/glm/kimi/qwen 在千帆上的 id，均未验证）；ERNIE 3.x/4.0 早期版（已 deprecated）；TTS/embedding/vision/OCR 非主线。

### 3. models.default（仅 default 档，主力兜底）

research id 未经验证，**不强扩多档**（避免将未验证 id 写入多个档位误导路由）。仅设 default：

```json
"models": { "default": { "default": "ernie-4.5-turbo" } }
```

选 `ernie-4.5-turbo` 而非 `ernie-5.1` 的理由：research 描述 4.5 Turbo 为"当前主推的高性能模型，支持长上下文和多模态"，5.1 虽更新但"搜索能力登顶"偏 specialty 定位，4.5 Turbo 更通用稳态。

### 4. desc（保留）

8 语言现状准确（"ERNIE 系列模型"），不改写。

### 5. source_urls（保留）

- docs: https://cloud.baidu.com/doc/qianfan-api/
- pricing: https://cloud.baidu.com/doc/qianfan-api/s/7m0g3yofv

## Acceptance Criteria

- [ ] `model_list.default` = 上述 6 模型（JSON 合法、无重复、仅控制台确认项）
- [ ] `models.default = {"default":"ernie-4.5-turbo"}`（仅 default 档，不强扩）
- [ ] endpoints/desc/source_urls/name/homepage/logo_url/client_type 全部不动
- [ ] `python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['qianfan'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"` 输出 `6 {'default': 'ernie-4.5-turbo'} 1`
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] 不动其他协议块、不动顶层 version/last_updated

## Out of Scope

- 推测性子型号 id（ernie-4.5-128k-preview 等，research 标"需验证"，等 API key 实测后再补）
- 第三方聚合模型在千帆的 id（deepseek/glm/kimi/qwen，均未验证）
- openai 兼容端点（research 实证 /v1 404）
- 国际端点（research 未发现）
- 认证方式实测（API key 直连 vs AK/SK 旧模式，未确认）
- STATIC_MODEL_IDS / peak_hours / coding_plan
- 其他协议块

## Technical Notes

- 真值源：`protocols.qianfan`
- 数据来源：research/qianfan-models.md（控制台模型广场 + /v1 端点 404 实测）
- id 格式：`ernie-<version>` 小写连字符（research 推测的 API 调用格式，**未通过 API 实测验证**）
- 数据强度：**弱**（官方文档需登录/JS 渲染无法完全获取、模型 id 多为推测、认证方式未确认）
- 后续验证：需有效 API key 调用 `/anthropic/coding/v1/models` 获取实际可用模型与准确 id，再修正 model_list + 扩 models.default 多档
