// ── ErrorBoundary：渲染错误兜底，白屏 → 错误面板 ──
// 两层：页级（App.tsx <main> 内，key=effectiveNav，切页重挂载即自动恢复，侧栏仍可用）
// + 根级（main.tsx，整壳崩时兜底）。错误经 console.error 由 frontendLog 转发进
// ~/.aidog/logs，同时全文渲染在页面上供用户复制提交。
import { Component, type ErrorInfo, type ReactNode } from "react";
import i18n from "../../locales";
import { CopyButton } from "./CopyButton";
import { Button } from "@/components/ui/button";

interface Props {
  children: ReactNode;
  /** 仅用于日志区分层级。 */
  scope: "page" | "root";
}

interface State {
  error: Error | null;
  info: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, info: null };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // frontendLog 已 patch console.error → 落 ~/.aidog/logs/aidog.<小时>.log
    console.error(`ErrorBoundary(${this.props.scope}):`, error, info.componentStack);
    this.setState({ info });
  }

  render() {
    if (!this.state.error) return this.props.children;
    const detail = `${String(this.state.error)}\n${this.state.info?.componentStack ?? ""}`;
    return (
      <div role="alert" style={{ maxWidth: 720, margin: "48px auto", padding: "24px", borderRadius: 10, border: "1px solid var(--color-danger, #D14B4B)", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: 16, fontWeight: 600, color: "var(--color-danger, #D14B4B)" }}>
          {i18n.t("errorBoundary.title")}
        </div>
        <div style={{ fontSize: 13, color: "var(--text-secondary, #888)" }}>
          {i18n.t("errorBoundary.hint")}
        </div>
        <pre data-testid="error-boundary-detail" style={{ margin: 0, padding: 12, borderRadius: 7, background: "var(--bg-subtle, #f4f4f5)", fontSize: 11.5, whiteSpace: "pre-wrap", wordBreak: "break-word", maxHeight: 320, overflow: "auto" }}>
          {detail}
        </pre>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <CopyButton text={detail} label={i18n.t("errorBoundary.copy")} />
          <Button variant="outline" size="sm" onClick={() => window.location.reload()}>
            {i18n.t("errorBoundary.reload")}
          </Button>
        </div>
      </div>
    );
  }
}
