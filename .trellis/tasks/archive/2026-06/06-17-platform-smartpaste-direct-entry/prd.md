# 平台智能识别入口前置（添加平台直达弹窗）

## 背景

当前从主列表到「智能识别」弹窗需 2 步：
1. 主列表「添加平台」按钮（`Platforms.tsx:2807`）→ `resetForm(); setShowForm(true)` → 进**空表单页**
2. 表单头「智能识别」按钮（`Platforms.tsx:2050`，仅 `!editing`）→ `setShowPaste(true)` → 开 SmartPasteModal

用户预期：点「添加平台」**直接**出智能识别弹窗，不要再多点一次「智能识别」。

## 决策（AskUserQuestion 已裁定 = 方案 A）

**「添加平台」按钮直达 SmartPasteModal**，智能识别成为添加平台的主入口；弹窗内新增「手动填写」次入口跳空表单，保留纯手动路径。

- apply（填入表单）后仍落到表单页确认字段（既有行为不变，仅补 `setShowForm(true)`）。
- 取消 → 回主列表（不开表单）。
- 手动填写 → 关弹窗 + 开空表单。

## 变更面

| # | 文件 | 改动 |
|---|---|---|
| 1 | `src/pages/Platforms.tsx:2807` | 「添加平台」onClick：`{ resetForm(); setShowPaste(true); }`（原 `setShowForm(true)`）→ 直达弹窗 |
| 2 | `src/pages/Platforms.tsx:1671`（`applyPaste` 末尾） | `setShowPaste(false)` 后补 `setShowForm(true)`（弹窗从列表开时表单未挂载，apply 后须显式拉起表单展示已填字段） |
| 3 | `src/pages/Platforms.tsx` 列表分支（`if (showForm)` 早返回之后的 list return） | 补 `{showPaste && <SmartPasteModal presets={PROTOCOLS} onApply={applyPaste} onManualEntry={...} onClose={() => setShowPaste(false)} />}`（镜像表单分支 line 2062 的渲染；modal 全屏 fixed overlay，两分支各渲染一处） |
| 4 | `src/components/platforms/SmartPasteModal.tsx` | ① `SmartPasteModalProps` 加 `onManualEntry?: () => void`；② 操作行（line 242）在「取消」前插「手动填写」按钮，点击调 `onManualEntry`（仅在传入时渲染，保持向后兼容） |
| 5 | i18n 8 locale | 新 key `platform.paste.manualEntry`：zh-CN「手动填写」/ en-US「Manual」/ ar-SA / fr-FR / de-DE / ru-RU / ja-JP（翻译参照既有 `action.*` 风格）；过 `scripts/check-i18n.mjs` |

## 不改

- 表单头「智能识别」按钮（`Platforms.tsx:2050`）保留 —— 已在空表单内时可再次粘贴。
- SmartPasteModal 打开即自动读剪贴板（`useEffect` line 51-60）、解析逻辑（`utils/platformPaste.ts`）、apply 数据结构 —— 全不动。
- 编辑已有平台（`editing`）流程不动。

## 验收

- [ ] 主列表点「添加平台」→ 直接弹 SmartPasteModal（不先进表单）。
- [ ] 弹窗 apply → 表单页出现且字段（name/protocol/endpoints/apiKey）已按解析结果填好。
- [ ] 弹窗「手动填写」→ 关弹窗 + 开空表单。
- [ ] 弹窗「取消」/ 点遮罩 → 回主列表，无残留表单。
- [ ] 表单内「智能识别」仍可用（回归）。
- [ ] `yarn build`（tsc + vite）0 error。
- [ ] `check-i18n.mjs` 0 裸 key。
- [ ] 8 locale `platform.paste.manualEntry` 齐全。

## 风险

- SmartPasteModal 在表单/列表两分支重复渲染 —— modal 为受控组件（`showPaste` 开关 + fixed overlay），重复渲染无副作用，但须确保 props 一致（同一 onApply/onClose/onManualEntry）。
- applyPaste 补 `setShowForm(true)` 后，若用户从**表单内**点「智能识别」再 apply，setShowForm(true) 是幂等无害（表单已显示）。
