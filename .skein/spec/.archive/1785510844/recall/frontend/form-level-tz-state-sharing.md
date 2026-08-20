---
title: 表单级时区状态共用 — 单一 state 透传多组件避免口径漂移
layer: recall
category: frontend
keywords: [表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性]
source: time-models-timezone task design.md §3.5 / prd.md 边界
authored-by: skein-spec
created: 1753805040
status: active
related: [time-zone-minute-arithmetic]
updated: 1753805040
---

## 触发场景

同一表单内多个组件展示同一类数据的不同维度（如 peak_hours 编辑器 + time_models 编辑器，都展示「时段」），需要在两个组件间同步切换时区显示模式。

## 陷阱：各组件独立 state 导致口径漂移

> `PlatformEditForm` 编辑单个平台配置。peak_hours 与 time_models 都含「时段」结构（start_hour/end_hour），都需时区显示切换。若各组件独立管理 `tzMode` state：
>
> - 用户在 peak_hours 切到 UTC+0
> - time_models 仍显示本地时区
> - 同一时段在两处显示基准不同 → 用户困惑 / 数据语义混乱

```tsx
// ❌ 各自独立 state（禁用）
<PeakHoursSection tzMode={peakHoursTz} setTzMode={setPeakHoursTz} />
<ModelsMatrixSection tzMode={modelsTz} setTzMode={setModelsTz} />
// 结果：切换一处另一处不变，两处显示基准矛盾
```

## 正解：表单级单一 state 透传（硬约束，关键）

### MUST 单一真值源（usePlatformForm hook）

```ts
// usePlatformForm.ts：表单级 hook，管理整个编辑态
export function usePlatformForm(...): PlatformFormState {
  // ... 其他 50+ 个 state ...
  
  // ✅ 时区展示模式：表单级单一 state（默认本地）
  const [windowsTz, setWindowsTz] = useState<"local" | "utc">("local");
  
  // 返回给 PlatformEditForm 的 context
  return {
    // ...
    peakHours, setPeakHours, windowsTz, setWindowsTz,  // 单一 state 对外透传
    // ...
  };
}
```

### MUST 无二次声明（PlatformEditForm 纯转发）

```tsx
// PlatformEditForm.tsx：纯转发，禁止重新声明或包装状态
export function PlatformEditForm({ s }: { s: PlatformsState }) {
  const {
    // ... 解构 50+ 个 state ...
    windowsTz, setWindowsTz,  // 直接来自 usePlatformForm
    // ...
  } = s.form;

  return (
    <>
      {/* peak_hours 编辑器 */}
      <PeakHoursSection
        windows={peakHours} setWindows={setPeakHours}
        tzMode={windowsTz}      // ← 透传
        setTzMode={setWindowsTz} // ← 透传
        // ...
      />

      {/* time_models 编辑器 */}
      <ModelsMatrixSection
        rules={timeModels} setRules={setTimeModels}
        tzMode={windowsTz}      // ← 同一 state
        setTzMode={setWindowsTz} // ← 同一 setter
        // ...
      />
    </>
  );
}
```

### MUST prop 签名对齐（设计锁死）

两个组件的 `tzMode` / `setTzMode` prop 签名**必须逐字一致**（避免形态分歧）：

```ts
// 类型定义（shared across all consuming components）
export type TzMode = "local" | "utc";

// PeakHoursSection
function PeakHoursSection({
  tzMode: TzMode,
  setTzMode: React.Dispatch<React.SetStateAction<TzMode>>,
  // ...
}: {...}) { ... }

// ModelsMatrixSection
function ModelsMatrixSection({
  tzMode: TzMode,  // ← 逐字相同
  setTzMode: React.Dispatch<React.SetStateAction<TzMode>>,  // ← 逐字相同
  // ...
}: {...}) { ... }
```

**理由**：React hooks 依赖项、类型检查、重构安全性都依赖精确对齐。字面不同（如 `onTzModeChange` vs `setTzMode`）等同于新建隐式状态。

## 反例 / 常见错误

| 错误                          | 为什么错                                        | 正确做法                                      |
| ----------------------------- | ----------------------------------------------- | ----------------------------------------- |
| 各组件独立 state `tzMode`    | 切一处另一处不变，同时段显示基准矛盾            | 表单级单一 state 透传                        |
| 中间组件再包装 state          | 形态分歧（A 传 local，B 改为 "local2"），一致性破裂 | 纯转发，零包装                                |
| prop 名不一致 `tzMode` vs `tz` | 含义混淆，重构时漏改，无法全局搜索/replace    | 逐字对齐 `tzMode` / `setTzMode`                  |
| prop 类型声明不一致         | TS 编译过但运行时行为不同                       | React.SetStateAction<TzMode> 精确声明         |
| 后续新增组件忘记接收 tzMode   | 第 3 个时段编辑器用默认本地，仍显示矛盾        | 在 hook/form 级文档明确：所有时段组件必须透传 |

## 落地 checklist

```bash
# 1. 验证单一真值源（usePlatformForm.ts 唯一声明）
grep -n "windowsTz\|peakHoursTz" src/pages/platforms/usePlatformForm.ts | wc -l  # 应为 3 处（声明+返回）

# 2. 验证无二次声明（PlatformEditForm 纯转发）
grep -n "useState.*[Tt]z\|windowsTz\|peakHoursTz" src/pages/platforms/PlatformEditForm.tsx | grep -v "=.*form\." | wc -l  # 应为 0

# 3. 验证 prop 签名对齐
grep -A10 "PeakHoursSection({" src/pages/platforms/formSections.tsx | grep "tzMode\|setTzMode"
grep -A10 "ModelsMatrixSection({" src/pages/platforms/ModelsMatrixSection.tsx | grep "tzMode\|setTzMode"
# 输出应逐字相同

# 4. 验证透传调用
grep -rn "tzMode=.*setTzMode=" src/pages/platforms/PlatformEditForm.tsx | wc -l  # 应为 2（PeakHours + ModelsMatrix）
```

## 验证场景

1. 用户勾选「本地时区」→ peak_hours 显示本地、time_models 也显示本地 ✅
2. 用户切到「UTC+0」→ 两个编辑器同时切到 UTC+0 ✅
3. 关闭表单重新打开 → 恢复默认「本地」（无持久化） ✅
4. 添加第 3 个时段组件 → 开发者必须在 design 文档明确声明「接收 tzMode/setTzMode」，避免漏掉

## 适用

- 同表单内多组件展示同一维度的数据（时区、主题、排序）
- 跨页面 UI state 需一致性同步

## 关联

[[time-zone-minute-arithmetic]] · [[rule-04]]

## 案例

- time-models-timezone task (commit 7f78c93e) — peakHoursTz 改名 windowsTz，表单级单一 state 透传 PeakHoursSection + ModelsMatrixSection
