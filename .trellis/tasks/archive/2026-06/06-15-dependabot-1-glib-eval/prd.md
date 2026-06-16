# PRD: dependabot-1-glib-eval

## 背景
GitHub Dependabot alert #1: https://github.com/lazygophers/aidog/security/dependabot/1

- 依赖: `glib` v0.18.5 (rust, runtime, transitive)
- 漏洞: RUSTSEC-2024-0429 / GHSA-wrw7-89jp-8q8g — `VariantStrIter` Iterator impl 的 `impl_get` 传 `&p`（应为 `&mut p`）给 C variadic 函数 `g_variant_get_child` 的 out-arg，UB → NULL deref 崩溃
- 严重度: **informational / unsound**（非 RCE/exploit，CVSS None）
- 受影响函数: `VariantStrIter::{next, nth, last, next_back, nth_back}`
- 漏洞范围: `>=0.15.0, <0.20.0`
- 修复版本: `>=0.20.0`（gtk-rs-core PR #1343）
- advisory 日期: 2024-03-30

## 依赖链 (Linux 目标, --target all)
```
glib v0.18.5
└── gtk v0.18.2 (UNMAINTAINED — crates.io 标注)
    └── tauri v2.11.2 ← aidog (我们的 app)
```
macOS/Windows 构建不链接 glib（gtk 是 Linux webkit2gtk 专属），但 Cargo.lock 仍记录该版本 → dependabot 扫描命中。

## 可达性分析
- 我们的 `src-tauri/src/` 不直接引用 glib / VariantStrIter / GVariant
- VariantStrIter 用于迭代 string-typed GVariant 数组（典型场景: GSettings schema 读取、D-Bus variant 子项）
- tauri 2.x Linux 路径内部是否触发该迭代器：不确定；即便触发，影响 = NULL deref 崩溃（非代码执行）

## 上游阻塞评估
| 路径 | 可行性 |
|---|---|
| `cargo update -p glib --precise 0.20.0` | ❌ gtk 0.18 要求 `glib ^0.18`（即 `<0.19`），semver 阻断 |
| `cargo update -p glib --precise 0.19.0` | ❌ 同上，0.19 也不满足 `^0.18` |
| 升级 gtk 到 0.19+ | ❌ gtk-rs 0.19 线不存在（gtk 0.18.2 = 最新，标注 UNMAINTAINED，建议迁 gtk4）|
| 升级 tauri 到使用 gtk4 的版本 | ❌ tauri 2.x 全系锁 gtk3/glib0.18；gtk4 迁移在 tauri 3.x 路线（未发布）|
| `[patch.crates-io]` glib 0.18.5 fork 回移植 PR #1343 | ⚠️ 技术可行（单行 `&p`→`&mut p`），但维护私有 fork 成本 > 收益，informational 级 advisory 不值得 |

**结论: tauri 2.x 内无可用修复路径**。此 advisory 影响**所有** tauri 2.x Linux 构建的依赖图，是 tauri 生态普遍现状。

## 决策
**Dismiss dependabot alert #1**，理由 `tolerable_risk`（GitHub dismiss reason），附说明：
- 上游阻塞（tauri 2.x / gtk-rs 0.18 线无修复）
- advisory 级别 informational/unsound，影响面 = UB 崩溃，非可利用漏洞
- 我们的代码路径不触及受影响函数

GitHub dismiss 是可逆操作（可 reopen），但属仓库安全姿态的外部动作 → **执行前需 main 明确确认**。

## 产出
1. 本 PRD 作为决策记录（已含完整分析）
2. 用户确认后，在 GitHub dismiss alert #1（reason: tolerable_risk，comment 摘录本 PRD 关键结论）

## 非目标
- 不维护 glib fork
- 不升级 tauri 到非 2.x
- 不改 Rust 业务代码

## 验证
- PRD 含：advisory 元数据 + 依赖链 + 可达性 + 上游阻塞矩阵 + 决策
- GitHub alert #1 状态变更为 dismissed（用户确认后执行）
