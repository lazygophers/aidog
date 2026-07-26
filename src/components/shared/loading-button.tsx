import * as React from "react"

import { Button, type ButtonProps } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { cn } from "@/lib/utils"
import { makeRipple } from "../../utils/motion"

export interface LoadingButtonProps extends ButtonProps {
  loading?: boolean
}

/**
 * LoadingButton — Button + Spinner + disabled 组合。
 * loading=true 时禁用按钮、在 children 前插入 Spinner。
 * 无 isLoading prop（遵循 shadcn 命名约定）。
 * 萤火虫: ripple 涟漪 CTA 反馈 (与 CompactCard toggle 同模式); loading 视觉由 Spinner 承载, 不叠 流光
 */
export const LoadingButton = React.forwardRef<
  HTMLButtonElement,
  LoadingButtonProps
>(({ loading = false, disabled, className, children, onClick, ...props }, ref) => {
  return (
    <Button
      ref={ref}
      disabled={disabled || loading}
      className={cn("ripple", loading && "gap-2", className)}
      onClick={(e) => {
        makeRipple(e)
        onClick?.(e)
      }}
      {...props}
    >
      {loading ? <Spinner /> : null}
      {children}
    </Button>
  )
})
LoadingButton.displayName = "LoadingButton"
