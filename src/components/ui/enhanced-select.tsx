"use client"

import * as React from "react"
import { Check, Search } from "lucide-react"

import { cn } from "@/lib/utils"
import { pinyinMatch } from "@/utils/pinyin"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
} from "@/components/ui/select"

export type EnhancedSelectOption = {
  value: string
  label: string
  /** 可选分组键; groups=true 时按此字段分段 */
  group?: string
}

export type EnhancedSelectProps = {
  options: EnhancedSelectOption[]
  /** 单选: string; 多选: string[] */
  value?: string | string[]
  onChange: (value: string | string[]) => void
  multiple?: boolean
  searchable?: boolean
  placeholder?: string
  /** 启用按 option.group 分段渲染 (仅当 options 携带 group 字段时生效) */
  groups?: boolean
  className?: string
  contentClassName?: string
  searchPlaceholder?: string
  searchAriaLabel?: string
  /** 渲染为禁用态的 value 列表 */
  disabledValues?: string[]
  id?: string
}

/**
 * 过滤选项: label 或 value 命中 query (pinyin 友好)
 * ponytail: O(n) 每次 keystroke; <500 options 不需虚拟化, 超出考虑 react-window
 */
export function filterOptions(
  options: EnhancedSelectOption[],
  query: string,
): EnhancedSelectOption[] {
  const q = query.trim()
  if (!q) return options
  return options.filter(
    (o) => pinyinMatch(q, o.label) || pinyinMatch(q, o.value),
  )
}

/** 按 option.group 聚合, 保持首次出现顺序; 无 group 归入 "" 桶 */
export function groupOptions(
  options: EnhancedSelectOption[],
): Array<[string, EnhancedSelectOption[]]> {
  const map = new Map<string, EnhancedSelectOption[]>()
  for (const o of options) {
    const k = o.group ?? ""
    let arr = map.get(k)
    if (!arr) {
      arr = []
      map.set(k, arr)
    }
    arr.push(o)
  }
  return Array.from(map.entries())
}

/** 多选 toggle: 存在移除, 不存在追加 */
export function toggleValue(arr: string[], v: string): string[] {
  return arr.includes(v) ? arr.filter((x) => x !== v) : [...arr, v]
}

/** 触发器显示文本: 单选显 label, 多选 join(", "), 空值显 placeholder */
export function formatDisplay(
  options: EnhancedSelectOption[],
  value: string | string[] | undefined,
  multiple: boolean,
  placeholder?: string,
): string {
  if (multiple) {
    const arr = Array.isArray(value) ? value : []
    if (arr.length === 0) return placeholder ?? ""
    return arr
      .map((v) => options.find((o) => o.value === v)?.label ?? v)
      .join(", ")
  }
  if (!value || Array.isArray(value)) return placeholder ?? ""
  return options.find((o) => o.value === value)?.label ?? value
}

/**
 * EnhancedSelect — Radix Select 内核 + 可选 搜索 / 分组 / 多选.
 *
 * 单选: 同 Radix Select 用法 (value:string, onChange 收 string), 点击 item 关闭菜单
 * 多选: value:string[], onChange 收 string[]; 通过 SelectItem.onSelect + preventDefault
 *       保持菜单开启, 手动 toggle 数组 (Radix 不原生支持多选)
 * 搜索: SelectContent 顶部 sticky Input, pinyinMatch 过滤 label+value
 * 分组: options 带 group 字段时按 group 渲染 SelectGroup + SelectLabel
 *
 * ponytail: 多选是 Radix Select 的已知缺口, 这里用 onSelect preventDefault 模拟;
 *           上限 ~500 options (无虚拟化), 超出需 react-window 之类
 */
export function EnhancedSelect({
  options,
  value,
  onChange,
  multiple = false,
  searchable = false,
  placeholder,
  groups = false,
  className,
  contentClassName,
  searchPlaceholder,
  searchAriaLabel,
  disabledValues,
  id,
}: EnhancedSelectProps) {
  const [query, setQuery] = React.useState("")
  const [open, setOpen] = React.useState(false)
  const inputRef = React.useRef<HTMLInputElement>(null)

  // 关闭时清空搜索; 打开且 searchable 时把焦点抢到 Input
  // ponytail: SelectContent wrapper 没 forward onOpenAutoFocus, 用 effect 兜底
  React.useEffect(() => {
    if (!open) {
      setQuery("")
      return
    }
    if (searchable) {
      requestAnimationFrame(() => inputRef.current?.focus())
    }
  }, [open, searchable])

  const filtered = React.useMemo(
    () => filterOptions(options, query),
    [options, query],
  )

  const grouped = React.useMemo<Array<[string, EnhancedSelectOption[]]>>(
    () => (groups ? groupOptions(filtered) : [["", filtered]]),
    [filtered, groups],
  )

  const display = formatDisplay(options, value, multiple, placeholder)

  // Radix Root value: 单选透传; 多选传 undefined (我们手动渲染 checkmark,
  // 不依赖 Radix ItemIndicator —— 它由 Root.value 单值匹配驱动, 多选下不工作)
  const radixValue = multiple ? undefined : (value as string) || undefined

  return (
    <Select
      open={open}
      onOpenChange={setOpen}
      value={radixValue}
      onValueChange={(v) => {
        if (!multiple) {
          onChange(v)
          setOpen(false)
        }
      }}
    >
      <SelectTrigger id={id} className={className}>
        <span
          className={cn("truncate", !display && "text-muted-foreground")}
        >
          {display || placeholder}
        </span>
      </SelectTrigger>
      <SelectContent className={contentClassName}>
        {searchable && (
          <div className="sticky top-0 z-10 bg-popover p-2 pb-1">
            <div className="relative">
              <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                ref={inputRef}
                tabIndex={0}
                value={query}
                onChange={(e) => {
                  e.stopPropagation()
                  setQuery(e.target.value)
                }}
                // 阻止 Radix typeahead 截获键入 (否则字母键被消费做选项跳转)
                onKeyDown={(e) => e.stopPropagation()}
                placeholder={searchPlaceholder ?? "搜索..."}
                aria-label={searchAriaLabel ?? "搜索选项"}
                className="h-8 pl-7 text-sm"
              />
            </div>
          </div>
        )}
        {filtered.length === 0 && (
          <div className="px-2 py-4 text-center text-sm text-muted-foreground">
            无匹配项
          </div>
        )}
        {grouped.map(([groupName, items]) => (
          <SelectGroup key={groupName || "_default"}>
            {groups && groupName && <SelectLabel>{groupName}</SelectLabel>}
            {items.map((opt) => {
              const selected = multiple
                ? Array.isArray(value) && value.includes(opt.value)
                : value === opt.value
              const disabled = disabledValues?.includes(opt.value)
              return (
                <SelectItem
                  key={opt.value}
                  value={opt.value}
                  disabled={disabled}
                  onSelect={
                    multiple
                      ? (e) => {
                          e.preventDefault()
                          const arr = Array.isArray(value) ? value : []
                          onChange(toggleValue(arr, opt.value))
                        }
                      : undefined
                  }
                  data-state={
                    multiple ? (selected ? "checked" : "unchecked") : undefined
                  }
                >
                  {multiple && (
                    // 覆盖在 default indicator 槽位 (right-2) 上方; default 槽位
                    // 在多选下永远空 (Root.value=undefined), 不冲突
                    <span className="absolute right-2 flex h-3.5 w-3.5 items-center justify-center">
                      {selected && <Check className="h-4 w-4" />}
                    </span>
                  )}
                  {opt.label}
                </SelectItem>
              )
            })}
          </SelectGroup>
        ))}
      </SelectContent>
    </Select>
  )
}
