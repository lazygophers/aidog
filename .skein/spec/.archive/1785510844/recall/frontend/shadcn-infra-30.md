---
title: CSS var live resolution 别名层
layer: recall
category: frontend
keywords: [css,var,alias,live-resolution,migration]
source: shadcn-infra
authored-by: skein-spec
created: 1784706722
status: active
related: []
updated: 1784706722
---

# CSS var live resolution 别名层

## 技巧
CSS 变量改名时，用 :root 定义别名层实现 live resolution，替代批量 sed 替换（零误伤、可回滚）。

## 正解
1. 在 :root 定义别名：`--legacy: var(--shadcn);`
2. 所有引用用旧名 `--legacy`，实际指向新名 `--shadcn`
3. 迁移完成后删别名行（自动失效）

## 对比
| 方式 | 改动量 | 误伤风险 | 回滚 |
|------|--------|---------|------|
| sed 批量替换 | 700+ 行 | 高（误伤类似变量名） | 难 |
| 别名层 | 10 行 | 无（CSS 引用透明） | 易（删别名） |

## 案例
- shadcn-infra task: 主题变量改名用别名层，globals.css 加 10 行 vs sed 700+ 行

## 适用
CSS 变量迁移、主题重构、大型 CSS 重构中间状态

## 关联
[[shadcn-infra-02]] (同任务 Tailwind 约束)
