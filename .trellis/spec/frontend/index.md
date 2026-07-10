---
updated: 2026-07-10
rewrite-version: 2
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

- [Conventions](./conventions.md) — 目录结构、组件、状态、API、类型、Hook 强制规则（唯一前端 spec 入口）；含 **CRUD 刷新链契约 (MUST)** —— `platformApi.delete` 等真删/真改入口全扫齐 + 受影响 state 必刷（乐观 filter 或全量 refreshPlatforms + `++epoch` 触发派生层）+ 独立信号优先（禁复用宽语义 callback）+ hook 级 renderHook 回归
- [Locale Tag Cross-Layer](./locale-tag-cross-layer.md) — locale 标签跨层一致性契约（**MUST `zh-Hans`** BCP47 script 禁 `zh-CN` 作前端 locale, **三层一致** i18next/presets JSON/DefaultsLocale 同集合, **AppContext:98 单向迁移** 旧 zh-CN→zh-Hans, **4 命名空间共存禁统一** 前端 zh-Hans / presets zh-Hans / 后端 Lang::ZhCn from_locale 归一 / Claude CLI zh-CN）
- [Derived Constants](./derived-constants.md) — 前端常量 → 后端 JSON 单真值源派生契约（**MUST** module-level `docPromise` 单次 RPC 缓存 + `buildXFromPresets(locale?)` / `getXMap()` async 派生; **MUST** 调用点 useState 空初始首帧 fallback + useEffect + `cancelled` flag 防竞态 + locale key `[i18n.language]`; 小常量 ≤10 稳定枚举例外保留硬编码; AppContext 预热 best-effort; 测试 mock 须对齐 JSON 真值）

⚠️ 改前端代码前 MUST 读 conventions.md 全部 MUST 节；❌ 禁跳过直接动手（每条违反代价见各节，漏读 = 引入已被禁止的模式）
