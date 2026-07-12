---
title: 写代码前查复用 (grep 已有实现)
layer: core
category: reuse
keywords: [grep,reuse,复用,组件,utility,抽象,dry,新函数]
source: trellis
authored-by: skein-memory
created: 1783832113
---

# Code Reuse Rules

何时被读: 写新函数 / 新组件 / 新 utility 前
谁读: trellis-implement sub-agent / main
不遵守的代价: 重复逻辑散布 → bug fix 不传播 → 同一 bug 反复出现

---

## MUST

- 写新函数前必须 `grep -rE '<关键词>' src/` 查已有实现；命中则复用，禁重写（跳过 grep → 造重复实现，后续 fix 只改一处另一处仍带 bug）
- 新增平台协议必须扩展 `Protocol` union type + `PROTOCOLS` 数组，禁独立定义
- 新增主题必须遵循 `ThemeDefinition` 接口并在 `themeMap` 注册
- 新增 locale 必须加入 `ALL_LOCALES` 数组 + `resources` 对象 + `RTL_LOCALES`（若 RTL）
- 同一逻辑 ≥ 2 调用点必须提取到共享函数，禁各文件各自实现（各自实现 → 逻辑分叉，一处改另一处漏，同一 bug 反复出现）
- 提取共享函数必须放在语义正确的目录:
  - UI 相关 → `src/components/`
  - 数据/业务 → `src/services/`
  - 主题 → `src/themes/`
  - i18n → `src/locales/`

## MUST NOT

- 禁止为新页面复制已有页面的 CRUD 模板代码而不提取公共组件
- 禁止定义与 `api.ts` 中已有 namespace 功能重叠的新 API 函数
- 禁止在 >1 个文件中硬编码相同的字符串常量（如协议名、URL）
- 禁止绕过已有 utility 函数直接实现相同逻辑

## Abstract Threshold

- ≥ 3 处相同逻辑 → 必须 abstract
- 2 处相同逻辑 → 必须 grep 确认，并在 commit message 说明是否 abstract 及理由
- 1 处 → 仅当 <20 行允许 inline；≥20 行 MUST 抽象，禁以 `// TODO` 拖延（拖延 → TODO 永不还，巨函数累积）

## After Batch Modifications (MUST)

- 改完一批文件后必须 `grep -rE '<改动的值>' src/` 确认无遗漏
- 若发现 ≥ 2 处遗漏 → 停下提取公共函数，禁逐文件补丁

## Verification

```bash
# 所有 invoke 集中在 api.ts
grep -rn 'invoke(' src/ | grep -v 'services/api.ts' | grep -v 'vite-env'  # 必须 0 行

# 无 any
grep -rn 'any' src/ --include='*.ts' --include='*.tsx'  # 必须 0 行

# 无重复 protocol 定义
grep -rn 'anthropic.*openai.*glm.*kimi' src/ --include='*.ts' --include='*.tsx' | wc -l  # 必须 = 2 (PROTOCOLS 数组 + union type)
```
