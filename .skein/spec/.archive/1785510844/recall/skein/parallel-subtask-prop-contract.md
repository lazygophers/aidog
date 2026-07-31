---
title: 并行 subtask 中的 prop 契约锁定 — 避免组件树改动冲突
layer: recall
category: skein
keywords: [并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning]
source: time-models-timezone task design.md §3.5
authored-by: skein-spec
created: 1753805040
status: active
related: [form-level-tz-state-sharing]
updated: 1753805040
protected: true
---

## 触发场景

两个或多个 subtask 需要同时改造同一组件树中的多个文件（例如 S2 改 `PlatformEditForm.tsx` + `usePlatformForm.ts` 的状态传递，S3 改 `WindowsEditModal.tsx` + `ModelsMatrixSection.tsx` 的业务逻辑）。并行执行时需确保：
1. 文件改动范围不重叠（S2 改文件 A，S3 改文件 B）
2. 组件间通信接口（prop 签名）提前锁定，不因工序顺序变化而改变

## 陷阱：未锁定 prop 契约导致运行时 BAD_REQUEST / TS 类型错

> S2 和 S3 分别并行改造组件树的不同部分，但 S2 声明的 prop 接收端签名（如 `ModelsMatrixSection` 需要 `tzMode` 和 `setTzMode`）与 S3 实现的发送端签名（S3 可能实现成 `tz` 和 `onTzChange`）不匹配：
>
> - **TS 编译时**：签名不匹配，编译报错（TS2322 / TS2345）
> - **运行时**：即使编译过，undefined prop 导致渲染错误或状态丢失
> - **集成失败**：两条分支合并时发现接口不一致，需要返工

## 正解：planning 阶段锁定 prop 契约（硬约束，关键）

### MUST 在 design.md 明确标记文件分工

```markdown
## 3.5 并行契约（S2/S3 同时跑，锁死边界）

### 文件划分（禁止跨界改动）
- **S2 负责**：`PlatformEditForm.tsx`（给 ModelsMatrixSection 新增 tzProps）
           + `usePlatformForm.ts`（管理 windowsTz state）
- **S3 负责**：`WindowsEditModal.tsx`（接入 time_models 侧时区换算）
           + `ModelsMatrixSection.tsx`（describeWindow + describeWindows 函数）

### Prop 契约（TS 签名）
`PlatformEditForm` → `ModelsMatrixSection` 的接口锁定：

\`\`\`tsx
<ModelsMatrixSection 
  tzMode={windowsTz}           // ← 类型: TzMode = "local" | "utc"
  setTzMode={setWindowsTz}     // ← 类型: React.Dispatch<React.SetStateAction<TzMode>>
  // ... 其他 props 不变 ...
/>
\`\`\`

**不许改的字面**：
- prop 名：`tzMode` / `setTzMode`（禁改为 `tz` / `onTzChange` 等）
- 类型：`React.Dispatch<React.SetStateAction<TzMode>>` 精确
- 实际值：来自 `usePlatformForm` 的 `windowsTz` / `setWindowsTz`
```

### MUST planning 表格明确化

在 task 的 planning 文档（如 prd.md / design.md）加入表格：

| 项目             | S2 输出端                      | S3 输入端                          | 契约      |
| -----            | -----                          | -----                              | -----     |
| 文件             | `usePlatformForm.ts` L216 state | `ModelsMatrixSection.tsx` L106 props | S2 声明，S3 消费 |
| State 名         | `windowsTz`                    | —（无需知道来源）                    | S2 创建，S3 接收 |
| Prop 名 (发送)   | —                              | `tzMode` (接收)                      | **逐字对齐** |
| Prop 名 (setter) | —                              | `setTzMode` (接收)                   | **逐字对齐** |
| 类型约束         | `"local" \| "utc"`             | `TzMode` (从 peakHours.ts import)   | 共用 type 定义 |
| 默认值           | `"local"`                      | —（由父传入）                        | S2 设置   |

**关键原则**：S2 在 design 文档写明「ModelsMatrixSection 需要 `tzMode: TzMode` + `setTzMode: Dispatch<...>`」，S3 开发时必须按此签名实现接收端。

### MUST 共用类型定义

```ts
// src/utils/peakHours.ts — 唯一真值源
export type TzMode = "local" | "utc";

// 任何 component props 都 import 这个，禁止重新定义
import type { TzMode } from "../../utils/peakHours";

interface ModelsMatrixSectionProps {
  tzMode: TzMode;  // ← 导入共用定义
  setTzMode: React.Dispatch<React.SetStateAction<TzMode>>;
}
```

### 检查清单（S2 与 S3 集成前）

```bash
# 1. 验证文件分工无重叠
S2_FILES=$(grep -l "PlatformEditForm\|usePlatformForm" <(git diff HEAD~4...HEAD --name-only | grep S2))
S3_FILES=$(grep -l "WindowsEditModal\|ModelsMatrixSection" <(git diff HEAD~4...HEAD --name-only | grep S3))
# S2_FILES 与 S3_FILES 应无交集

# 2. 验证 prop 签名一致
grep -A5 "ModelsMatrixSection({" src/pages/platforms/ModelsMatrixSection.tsx | grep "tzMode\|setTzMode"
grep -rn "tzMode=.*setTzMode=" src/pages/platforms/PlatformEditForm.tsx
# 输出应逐字相同

# 3. 验证类型导入共用
grep "import.*TzMode" src/pages/platforms/ModelsMatrixSection.tsx
grep "import.*TzMode" src/pages/platforms/WindowsEditModal.tsx
# 都应来自 peakHours.ts（同一源）
```

## 反例 / 常见错误

| 错误                            | 为什么错                                        | 正确做法                                      |
| --------------------------------- | ----------------------------------------------- | ----------------------------------------- |
| S2/S3 都改 PlatformEditForm.tsx   | 文件冲突，合并时字面冲突，需手工编辑            | planning 明确划分，S2 专改此文件，S3 禁碰       |
| Prop 名不对齐 (`tz` vs `tzMode`)| 编译错 TS2322，或运行时 undefined → 渲染破裂    | design 文档锁定字面，两侧逐字对齐          |
| 类型不共用 (各自定义 TzMode)    | type alias 字面相同但是不同 reference，TS 报错 | 导入共用 type 定义 (peakHours.ts)          |
| Prop 类型签名含混（字符串 vs union） | 宽松接收可能漏掉值检查，状态污染 | React.SetStateAction<TzMode> 精确约束           |
| 没有 planning 明确文档           | S2/S3 各自推测契约，结果不同，集成炸裂         | design 表格明确每项契约，可 grep 验证      |

## 落地 checklist

```bash
# 集成前逐项验证
# 1. 文件分工
git log --oneline time-models-timezone-s2...time-models-timezone-s3 --name-only | sort -u
# 确认无文件重叠

# 2. Prop 签名一致性
diff <(grep "tzMode.*setTzMode" PlatformEditForm.tsx) <(grep "tzMode.*setTzMode" ModelsMatrixSection.tsx)
# 无输出 = 完全一致

# 3. 类型共用
grep -h "import.*TzMode" src/pages/platforms/{WindowsEditModal,ModelsMatrixSection}.tsx | sort -u | wc -l
# 应为 1（同一行）

# 4. TS 编译全绿
npx tsc --noEmit
# 零错误
```

## 验证场景

1. S2 提交：`usePlatformForm.ts` 新增 `windowsTz` state，design 文档标明「ModelsMatrixSection 需 `tzMode`/`setTzMode`」
2. S3 提交：`ModelsMatrixSection.tsx` 声明 prop 接口 `{ tzMode: TzMode, setTzMode: Dispatch<...> }`
3. S2/S3 合并：`PlatformEditForm.tsx` 中 `<ModelsMatrixSection tzMode={windowsTz} setTzMode={setWindowsTz} />`，类型完美对齐，零改动
4. CI 编译：npx tsc --noEmit 过，yarn test 过

## 适用

- 并行多个 subtask 改造同一组件树的不同部分
- 跨团队开发中需要接口预协商的场景（prop 签名即"API 契约"）

## 关联

[[dirty-float-hour-normalization]] · [[form-level-tz-state-sharing]]

## 案例

- time-models-timezone task (design.md §3.5) — S2/S3 并行，prop 契约在 design 表格锁定；集成时 PlatformEditForm.tsx 给 ModelsMatrixSection 传 `tzMode={windowsTz} setTzMode={setWindowsTz}`，逐字对齐
