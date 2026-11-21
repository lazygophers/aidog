// 换客户端继续菜单（扩展-会话页 票 10，spec D5）：列出本机已安装的其它客户端。
// Claude Code ⇄ Codex 为转写（需「允许读取对话原文」开关，未开时项内引导并跳设置）；其余组合为交接（不读原文）。
// 无工作目录的会话无法在目标客户端打开，不提供；按钮在可展开行内，阻冒泡。
import { useTranslation } from "react-i18next";
import { ArrowRightLeft } from "lucide-react";
import type { ClientUsageSessionRow } from "../../services/api/clientUsage";
import { ClientIcon, SESSION_CLIENTS } from "./ClientIcon";
import { Button } from "../ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuTrigger } from "../ui/dropdown-menu";

const isTranscribePair = (from: string, to: string) =>
  (from === "claude_code" && to === "codex") || (from === "codex" && to === "claude_code");

export function CarryMenu({ r, installed, transcriptOn, onCarry, onNeedTranscript }: {
  r: ClientUsageSessionRow;
  installed: string[];
  transcriptOn: boolean;
  onCarry: (r: ClientUsageSessionRow, to: string) => void;
  onNeedTranscript: () => void;
}) {
  const { t } = useTranslation();
  const targets = r.workdir ? SESSION_CLIENTS.filter((c) => c.code !== r.clientCode && installed.includes(c.code)) : [];
  if (targets.length === 0) return null;
  const label = t("sessions.carry", "换客户端继续");
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          size="icon"
          variant="ghost"
          className="size-8 shrink-0"
          title={label}
          aria-label={label}
          data-testid="sessions-carry"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          <ArrowRightLeft aria-hidden className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-56" data-testid="sessions-carry-menu" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
        {/* portal 内的 React 事件仍冒泡到行：阻断，选菜单项不展开行 */}
        <DropdownMenuLabel className="text-sm font-normal text-muted-foreground">{label}</DropdownMenuLabel>
        {targets.map((c) => {
          const mode = !isTranscribePair(r.clientCode, c.code) ? "handoff" : transcriptOn ? "transcribe" : "need-consent";
          return (
            <DropdownMenuItem
              key={c.code}
              className="focus:bg-[var(--accent-subtle)] focus:text-foreground"
              data-testid="sessions-carry-item"
              data-target={c.code}
              data-mode={mode}
              onSelect={() => (mode === "need-consent" ? onNeedTranscript() : onCarry(r, c.code))}
            >
              <ClientIcon code={c.code} size={14} />
              <span className="flex-1">{c.name}</span>
              {mode === "need-consent" ? (
                <span className="text-sm text-muted-foreground" data-testid="sessions-carry-need-consent">{t("sessions.carryNeedConsent", "需开启原文读取")}</span>
              ) : (
                <span className="text-sm text-muted-foreground">{mode === "transcribe" ? t("sessions.carryTranscribe", "转写上下文") : t("sessions.carryHandoff", "交接文件路径")}</span>
              )}
            </DropdownMenuItem>
          );
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
