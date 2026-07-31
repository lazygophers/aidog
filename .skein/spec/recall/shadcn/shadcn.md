---
title: shadcn
category: shadcn
keywords: [radix,Select,空值,哨兵,__none__,PlatformPicker,planning,grep,预筛,范围,planning阶段,Radix,Dialog,DialogTitle,a11y,sr-only,无障碍,number,String,Number,双向映射,open,null,Promise,resolve,bool,shadcn,Button,cva,svg,16px,size-4,dnd-kit,SortableList,拖拽,迁移]
status: active
inclusion: auto
---

## radix Select 空值哨兵模式

使用 radix Select 组件时，value 属性需要处理空值/undefined 状态，使用哨兵值避免内部验证错误。

## 陷阱-正解

❌ **陷阱**：直接使用 `value=""` 会触发 radix Select 内部验证错误（SelectItem value="" 会抛错）。
✅ **正解**：使用 `__none__` 哨兵值 + onValueChange 映射回 undefined/""。

## 模式模板

```tsx
// 定义哨兵常量
const NONE = "__none__";

// 组件使用
<Select
  value={!value ? NONE : value}
  onValueChange={(v) => onChange(v === NONE ? undefined : v)}
>
  <SelectContent>
    <SelectItem value={NONE}>—</SelectItem>
    {opts.map((o) => <SelectItem key={o} value={o}>{o}</SelectItem>)}
  </SelectContent>
</Select>
```

## 适用

- radix Select 组件（@/components/ui/select）
- 需要空值占位符的下拉选择场景

## 案例

- `src/pages/platforms/PlatformPicker.tsx:105-109` 可选平台选择器

## 关联

[[radix-select-number-mapping]]

## planning 范围预筛纪律（grep）

planning 阶段需要预先用 grep 检查目标范围是否真的需要该改动，避免对不含相关代码的文件跑不必要的改造。

## 陷阱-正解

❌ **陷阱**：planning 时未预筛，按通用模板对所有目标域跑变更逻辑，对不含相关代码的区域产生误判。
✅ **正解**：planning 先 grep 预筛，检查目标域是否真的含需要改造的代码；命中 0 即跳过。

## 预筛命令模板

```bash
# 检查是否存在相关代码
grep -r "相关代码模式" 目标路径/ --include="*.ts*"

# 命中 0 → 跳过该域的改造
```

## 例子

- **shadcn 迁移**：检查是否有 `<button` / `<input` / `<select` 等表单控件
  ```bash
  grep -c "<button\|<input\|<select\|<textarea" src/pages/PopoverConfigTab/*.tsx
  # 命中 0 → 跳过
  ```
- **时区处理**：检查是否有 `new Date()` 或时间计算
  ```bash
  grep -r "new Date()\|getTime\|setHours" src/ --include="*.tsx"
  ```

## 适用

- planning 阶段大范围变更（框架升级、组件库迁移、业务重构）
- 确保 task 范围精准，避免 false positive

## 关联

[[radix-select-none-sentinel]]、[[platform-creation-entry-consolidation]]

## Radix Dialog 必须含 DialogTitle

Radix Dialog 组件必须包含 DialogTitle 以满足无障碍（a11y）要求。自定义 header 时使用 sr-only 隐藏 title。

## MUST 硬约束

Radix Dialog **必须包含 DialogTitle**，否则会触发 a11y 警告。

## 实现模式

❌ **陷阱**：自定义 header 时完全省略 DialogTitle，破坏 a11y。
✅ **正解**：用 `sr-only` className 隐藏 DialogTitle，保留语义但不破坏自定义 header 视觉。

## 模式模板

```tsx
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

<Dialog open={open} onOpenChange={onOpenChange}>
  <DialogContent>
    {/* sr-only title 满足 Radix Dialog a11y 要求 */}
    <DialogTitle className="sr-only">{title}</DialogTitle>
    
    {/* 自定义 header */}
    <div style={{ display: "flex", justifyContent: "space-between" }}>
      <div>{title}</div>
      <Button onClick={onClose}>×</Button>
    </div>
  </DialogContent>
</Dialog>
```

## 适用

- 所有 Radix Dialog 用法（@/components/ui/dialog）
- 需要完全自定义 header 视觉的场景

## 案例

- `src/components/settings/editors/StatusLineSection/SegmentEditModal.tsx:49-50` sr-only title + 自定义 header

## 关联

[[dialog-open-explicit-null]]

## radix Select number 双向映射

radix Select 的 value 属性只接受 string 类型，需要处理 number 类型数据时使用双向映射。

## 陷阱-正解

❌ **陷阱**：直接传 number 会触发类型错误或运行时异常。
✅ **正解**：存储/显示时 String() 转字符串，回调时 Number() 转回数字。

## 模式模板

```tsx
<Select
  value={String(numberValue)}  // 存储/显示：number → string
  onValueChange={(v) => onChange(Number(v))}  // 回调：string → number
>
  <SelectContent>
    {options.map((n) => <SelectItem key={n} value={String(n)}>{n}</SelectItem>)}
  </SelectContent>
</Select>
```

## 适用

- radix Select value 仅收 string（类型约束）
- 需要处理 number 选项的分页器/数值选择器

## 案例

- `src/pages/Logs/primitives.tsx:374` Pagination pageSize: `value={String(pageSize)}` + `onValueChange={v => onPageSizeChange(Number(v))}`

## 关联

[[radix-select-none-sentinel]]

## Dialog.open 需显式 null 判断

Dialog.open 属性需要 bool 类型，当使用 Promise resolve 型 state 时需显式 null 判断。

## 陷阱-正解

❌ **陷阱**：直接用 `open={modalState}` 会将 null/对象转为 bool，无法正确反映语义。
✅ **正解**：`open={modalState !== null}` 显式判断，确保 null 关闭、非空打开。

## 模式模板

```tsx
const [modalState, setModalState] = useState<{resolve: (v: any) => void} | null>(null);

<Dialog open={modalState !== null} onOpenChange={(o) => { if (!o) setModalState(null); }}>
  <DialogContent>
    {/* ... modal 内容 ... */}
  </DialogContent>
</Dialog>
```

## 适用

- 任何 Promise resolve 型 state 控制弹窗开关的场景（如 async confirm/自定义 Modal）
- Radix Dialog open 属性需要 bool 的场景

## 关联

[[radix-dialog-requires-title]]

## shadcn Button cva 基类压 svg 16px

shadcn Button 组件 cva 基类含 `[&_svg]:size-4` 规则，统一压内部 svg 至 16px。

## MUST 硬约束

shadcn Button 内的 svg 图标会被强制压至 16px（`size-4` = 1rem = 16px），自定义尺寸需显式覆盖。

## Button cva 基类

```tsx
variants: {
  // ...
  base: "inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 [&_svg]:size-4"
}
```

## 适用

- 所有 shadcn Button 用法（@/components/ui/button）
- nav icon 等小图标场景（接受 16px 默认）

## 关联

[[dialog-open-explicit-null]]

## dnd-kit SortableList 迁移保留拖拽逻辑

dnd-kit SortableList 组件迁移时，只需替换内部 button/视觉组件，拖拽逻辑保持不变。

## 陷阱-正解

❌ **陷阱**：重写整个拖拽逻辑，破坏已有行为。
✅ **正解**：保留 dnd-kit 的 useSortable/Sensors 逻辑，仅替换 `<button>` → `<Button>`、样式 → shadcn 风格。

## 模式模板

```tsx
// 保留：拖拽逻辑
const { attributes, listeners, setNodeRef, transform } = useSortable({ id });
const style = transform ? { transform: CSS.Transform.toString(transform) } : undefined;

// 替换：button → Button + 样式
<React.Fragment ref={setNodeRef} style={style} {...attributes}>
  <div {...listeners}>
    <Button variant="ghost" size="icon">
      <svg>...</svg> {/* drag handle */}
    </Button>
  </div>
</React.Fragment>
```

## 适用

- dnd-kit SortableList 迁移至 shadcn
- 保留拖拽逻辑仅换视觉的场景

## 案例

- shadcn-pages task：Groups/GroupListItem SortableList 迁移，保留拖拽仅换 Button

## 关联

[[radix-select-none-sentinel]]
