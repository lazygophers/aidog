# Check

## 验证
- yarn build ✓ (8 locale chunk 正常, tsc 通过)
- 字面量 t() key: 618 个, 缺失 0
- 模板动态 key: 21 前缀全覆盖 (statusline.seg 109 / hooks.event 24 / perm.* 26 / env 129 / notif.* 13 / stats 25 / theme 8)
- TrayConfigTab: computeItemText + makeMetricLabel + 标题/placeholder/separator 全走 t(), TFunction 类型正确

## 范围决策
- Platforms.tsx PLATFORM_PRESETS/LABELS 品牌专有名词 (智谱/月之暗面/百炼/豆包等) 保留 — 翻译会错误, keywords 含中文利于搜索
- editors.tsx statusline seg placeholder 默认值 (余额 /缓存 等用户可自定义前缀) 保留作示例
- JSX 注释 ({/* */}) + // 行注释 不处理
