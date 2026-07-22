import * as React from "react"

import { Button, type ButtonProps } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { cn } from "@/lib/utils"

export interface LoadingButtonProps extends ButtonProps {
  loading?: boolean
}

/**
 * LoadingButton — Button + Spinner + disabled 组合。
 * loading=true 时禁用按钮、在 children 前插入 Spinner。
 * 无 isLoading prop（遵循 shadcn 命名约定）。
 */
export const LoadingButton = React.forwardRef<
  HTMLButtonElement,
  LoadingButtonProps
>(({ loading = false, disabled, className, children, ...props }, ref) => {
  return (
    <Button
      ref={ref}
      disabled={disabled || loading}
      className={cn(loading && "gap-2", className)}
      {...props}
    >
      {loading ? <Spinner /> : null}
      {children}
    </Button>
  )
})
LoadingButton.displayName = "LoadingButton"
