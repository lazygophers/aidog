// 会话页客户端几何记号（设计稿方向 3）：Claude Code 四分之一圆 / Codex 方 / Gemini CLI 圆 / OpenCode 半圆；
// 实心 = 已启用读取，虚线描边 = 未启用。

export const SESSION_CLIENTS = [
  { code: "claude_code", name: "Claude Code" },
  { code: "codex", name: "Codex" },
  { code: "gemini_cli", name: "Gemini CLI" },
  { code: "opencode", name: "OpenCode" },
] as const;

const COLOR: Record<string, string> = {
  claude_code: "var(--primary)",
  codex: "var(--color-info)",
  gemini_cli: "var(--color-warning)",
  opencode: "var(--muted-foreground)",
};

export function clientName(code: string): string {
  return SESSION_CLIENTS.find((c) => c.code === code)?.name ?? code;
}

export function ClientGlyph({ code, size, on }: { code: string; size: number; on: boolean }) {
  const fill = on ? (COLOR[code] ?? "var(--muted-foreground)") : "none";
  const stroke = on ? "none" : "var(--border)";
  const p = { fill, stroke, strokeWidth: 2, strokeDasharray: on ? undefined : "4 3" };
  const shape = {
    claude_code: <path d="M4 52 L4 4 A48 48 0 0 1 52 52 Z" {...p} />,
    codex: <rect x="6" y="6" width="44" height="44" {...p} />,
    gemini_cli: <circle cx="28" cy="28" r="23" {...p} />,
    opencode: <path d="M4 50 A24 24 0 0 1 52 50 Z" {...p} />,
  }[code] ?? <circle cx="28" cy="28" r="23" {...p} />;
  return (
    <svg width={size} height={size} viewBox="0 0 56 56" aria-hidden="true" className="shrink-0">
      {shape}
    </svg>
  );
}
