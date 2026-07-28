---
title: 搬迁类重构的 i18n 核对法（comm -23 key 集合）
layer: recall
category: arch
keywords: [i18n,migration,locale,key,coverage,comm]
source: -
authored-by: skein-spec
created: 1785226217
status: active
related: []
updated: 1785226217
---

## 触发场景
command/组件迁 crate 或改名时，若涉及 i18n key（如 UI 文案）。

## 陷阱
不动 locale 文件时 `yarn check-i18n` 查不出搬迁丢 key（新位置 key 可能取名不同）。

## 正解
搬迁前后比对 locale key 集合（grep 源代码找 namespace 模式），用 comm -23 差集查漏：
```bash
# 搬迁前：src/ 所有 i18n key 调用
grep -r "ns(\\\"<ns>" src/ --include="*.tsx" | sed 's/.*ns("//' | sed 's/").*//' | sort -u > keys_before.txt

# 搬迁后：新位置 key 调用
grep -r "ns(\\\"<ns>" src-tauri/ --include="*.rs" | ... > keys_after.txt  # 如果迁入 Rust

# 差集 = 搬迁丢的 key
comm -23 keys_before.txt keys_after.txt
```

## 案例
- arch-deepen-2 c3-commands batch 3：搬迁时检查 system/ai_tools/cli_env/cli_proxy key 覆盖

## 适用
- 跨 crate 搬迁涉及 i18n
- rename command 时
- 删减功能前验证

## 关联
[[新增_i18n_key_必须同步_8语言]] [[locale_扁平_key_约定]]
