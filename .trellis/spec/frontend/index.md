---
updated: 2026-07-07
rewrite-version: 1
supersedes:
  - frontend/index.md (v0 placeholder)
authored-by: trellisx-spec
mode: optimize
---

# Frontend Development

何时被读: 任何涉及 `src/` 的任务规划 / 代码改动
谁读: main / sub-agent
不遵守的代价: 模式漂移 → 维护成本翻倍

---

## Index

- [Conventions](./conventions.md) — 目录结构、组件、状态、API、类型、Hook 强制规则（唯一前端 spec 入口）
- [Locale Tag Cross-Layer](./locale-tag-cross-layer.md) — locale 标签跨层一致性契约（**MUST `zh-Hans`** BCP47 script 禁 `zh-CN` 作前端 locale, **三层一致** i18next/presets JSON/DefaultsLocale 同集合, **AppContext:98 单向迁移** 旧 zh-CN→zh-Hans, **4 命名空间共存禁统一** 前端 zh-Hans / presets zh-Hans / 后端 Lang::ZhCn from_locale 归一 / Claude CLI zh-CN）

⚠️ 改前端代码前 MUST 读 conventions.md 全部 MUST 节；❌ 禁跳过直接动手（每条违反代价见各节，漏读 = 引入已被禁止的模式）
