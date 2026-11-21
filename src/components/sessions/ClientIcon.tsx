// 会话页客户端官方图标（票 11，取代几何记号）：本地打包 SVG，离线不破图。
// Claude Code / Gemini 复用平台 logo；Codex 用其出品方 OpenAI 的标志；OpenCode 取自 simpleicons。
import claudeCodeSvg from "../../assets/platforms/claude_code.svg";
import geminiSvg from "../../assets/platforms/gemini.svg";
import openaiSvg from "../../assets/platforms/openai.svg";
import opencodeSvg from "../../assets/platforms/opencode.svg";

export const SESSION_CLIENTS = [
  { code: "claude_code", name: "Claude Code" },
  { code: "codex", name: "Codex" },
  { code: "gemini_cli", name: "Gemini CLI" },
  { code: "opencode", name: "OpenCode" },
] as const;

const ICON: Record<string, string> = {
  claude_code: claudeCodeSvg,
  codex: openaiSvg,
  gemini_cli: geminiSvg,
  opencode: opencodeSvg,
};
// 黑色单色标志：深色模式反相，否则看不见（主题写 :root[data-mode]，不挂 .dark 类）。
const MONO = new Set(["codex", "opencode"]);

export function clientName(code: string): string {
  return SESSION_CLIENTS.find((c) => c.code === code)?.name ?? code;
}

export function ClientIcon({ code, size }: { code: string; size: number }) {
  const src = ICON[code];
  if (!src) return <span aria-hidden className="shrink-0" style={{ width: size, height: size }} />;
  return (
    <img
      src={src}
      alt=""
      aria-hidden
      width={size}
      height={size}
      className={`shrink-0 object-contain ${MONO.has(code) ? "[:root[data-mode=dark]_&]:invert" : ""}`}
      data-testid="client-icon"
      data-client={code}
    />
  );
}
