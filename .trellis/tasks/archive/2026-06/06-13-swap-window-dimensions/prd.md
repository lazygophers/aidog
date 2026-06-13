# 对调默认窗口长宽

## 背景
`src-tauri/tauri.conf.json` `app.windows[0]` 当前 `width=959, height=926`（横向偏宽）。对调成竖向偏长。

## 目标
交换 width / height 值。

## 变更
- 文件：`src-tauri/tauri.conf.json`
- `width`: 959 → 926
- `height`: 926 → 959

## 验证
- JSON 合法（`python3 -c "import json; json.load(open(...))"`)
- 两值确认 926 / 959

## 非目标
- 不改 `minWidth/maxWidth/minHeight/maxHeight`（当前未配置）
- 不动 `macOSPrivateApi` / `csp` / 其他字段
