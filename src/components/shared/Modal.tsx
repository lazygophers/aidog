import { useEffect } from "react";
import { createPortal } from "react-dom";

export interface ModalProps {
  /** modal 是否显示 */
  open: boolean;
  /** 关闭回调 */
  onClose: () => void;
  /** modal 内容 */
  children: React.ReactNode;
  /** 容器额外 className（如 "glass-elevated"） */
  className?: string;
  /** 容器额外 style */
  style?: React.CSSProperties;
  /** backdrop 层 zIndex（默认 1100） */
  zIndex?: number;
  /** 点击 backdrop 是否关闭（默认 true） */
  closeOnBackdrop?: boolean;
  /** 按 ESC 是否关闭（默认 true） */
  closeOnEscape?: boolean;
  /** 容器 maxWidth（默认 500px） */
  maxWidth?: number | string;
  /** 容器 maxHeight（默认 85vh） */
  maxHeight?: number | string;
}

/**
 * Modal 基元组件
 *
 * 必须使用 createPortal(document.body) 解决 liquid glass 主题下
 * 祖先 transform/backdrop-filter 导致 position:fixed 退化问题。
 *
 * @see memory modal-window-center-rule
 */
export function Modal({
  open,
  onClose,
  children,
  className,
  style,
  zIndex = 1100,
  closeOnBackdrop = true,
  closeOnEscape = true,
  maxWidth = 500,
  maxHeight = "85vh",
}: ModalProps) {
  // ESC 键关闭
  useEffect(() => {
    if (!open || !closeOnEscape) return;

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };

    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [open, closeOnEscape, onClose]);

  if (!open) return null;

  const backdropStyle: React.CSSProperties = {
    position: "fixed",
    inset: 0,
    zIndex,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    background: "rgba(0,0,0,0.5)",
    animation: "fadeIn 150ms ease both",
  };

  const containerStyle: React.CSSProperties = {
    ...style,
    background: "var(--bg-surface)",
    borderRadius: "var(--radius-lg)",
    padding: 20,
    maxWidth: typeof maxWidth === "number" ? `${maxWidth}px` : maxWidth,
    maxHeight: typeof maxHeight === "number" ? `${maxHeight}px` : maxHeight,
    overflowY: "auto",
    border: "1px solid var(--border)",
    animation: "fadeIn 200ms ease both",
  };

  return createPortal(
    <div
      style={backdropStyle}
      onClick={closeOnBackdrop ? onClose : undefined}
    >
      <div
        className={className}
        style={containerStyle}
        onClick={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </div>,
    document.body
  );
}
