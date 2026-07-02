# PRD — 平台分组优先级检查时机改确认后触发

## 背景
用户报：「优先级的设置，检查是否符合规范的时机应该是用户确认后而不是发生变更的时候」。指 aidog app 内**平台分组优先级**(`level_priority`)编辑控件。

现状(PlatformCard.tsx:590 `LevelPriorityControl`)：stepper(上下按钮 + number input)，input `onChange`(:646) 每次按键立即 `clamp(1-10)` + `onChange(next)` → `handleSetLevelPriority`(Groups.tsx:1274 乐观更新 + 立即 API + 失败回滚)。即**变更时**实时检查范围 + 保存。

问题：输入"15"过程中先存"1"再"15"中间态触发多次 API；范围外值实时闪烁；每次按键都打后端。

## 决策(用户 2026-07-02 锁)
| 维度 | 决策 |
|---|---|
| 确认交互 | **onBlur 提交** — input 编辑过程走本地 state(不检查不保存)，blur 或 Enter 时才 clamp + onChange 提交。上下按钮(+/-)仍立即提交(按钮=明确确认动作) |
| 范围外处理 | **blur 时 clamp 回边界静默纠正** — 99→10, 0→1, blur 时 clamp 后提交边界值(沿用现有 clamp 逻辑,仅时机移到 blur) |
| 检查时机 | clamp(规范检查)从 onChange(set 实时) 移到 onBlur(确认后) |

## 交付
1. **`LevelPriorityControl`(PlatformCard.tsx:590)** 改 input 行为：
   - 加 local state(编辑态文本，init = value)；input `value={local}` 而非 `value`
   - `onChange`：仅更新 local state（不 clamp、不调 onChange prop、不触发保存）
   - `onBlur` + `onKeyDown Enter`：`commit()` = clamp(local) → 若 ≠ value 则 onChange(next) → local 重置为 value
   - value prop 变化(外部)时同步 local（useEffect）
   - 上下按钮(+/-)保留立即提交：`onClick` → `set(value±1)` → clamp + onChange(不变,按钮是明确确认)
2. **clamp 时机** — 从 `set`(被 input onChange + 按钮共用)拆出：input 走 commit-time clamp(blur)，按钮走即时 clamp(保留)。或 set 保留即时 clamp 供按钮，input 不走 set 改走 commit。
3. **`handleSetLevelPriority`(Groups.tsx:1274)** — 不变(乐观更新+API+回滚)，仅上游触发时机从 onChange 移到 blur。

## 验收
- input 编辑过程(按键)不触发 onChange / 不调 API(无中间态保存)
- blur 或 Enter 时 clamp(1-10) + 提交(触发 handleSetLevelPriority)
- 输入 99 → blur → 静默 clamp 10 + 提交；输入 0 → clamp 1 + 提交
- 上下按钮(+/-)点击立即提交(不变)
- 外部 value 变化(如回滚)同步到 input 显示
- `yarn build` + `cargo clippy`(无 Rust 改) + `check-i18n` 全绿

## 非目标(YAGNI)
- 上下按钮延后(用户定按钮=明确确认,保留即时)
- 显式"保存"按钮(onBlur 即确认,免加按钮)
- debounce 方案(用户选 onBlur 非 debounce)
- 范围外 toast 提示(静默 clamp,不打扰)
- 改 `priority`(分组内平台拖拽排序,:1219 按序赋值,非用户编辑数字,不在范围)

## 调度
- write-files: `src/components/platforms/PlatformCard.tsx`(LevelPriorityControl :590-660 单控件)
- 与 deeplink(D2 Platforms.tsx / D3 Mcp.tsx / D4 Skills.tsx / ShareModal)不相交 → 文件级可并行
- **但 active 槽满 2**(deeplink parent + D2 in_progress)→ 排队,D2 finish 释放槽后 start
- 单文件单控件改,中等复杂度(local state + blur commit),≤1 文件

## 风险
- local state 与受控 value 同步(useEffect 慎防循环)
- Enter 提交后 input 保持聚焦或 blur(交互细节,实现时定)
- 上下按钮即时 vs input 延后的行为不一致需注释说明(按钮=确认动作,input=自由编辑)
