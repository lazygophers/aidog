// 一键写入卡（票 57-03 自 ToolsSection 拆出）：展示层，安装状态由容器持有（rail 切换不丢结果）。
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { cn } from "../../lib/utils";
import type { ReactNode } from "react";
import { CardHeader } from "./CardChrome";

export function InstallCard({
  installing,
  installMsg,
  installOk,
  onInstall,
}: {
  installing: boolean;
  installMsg: string;
  installOk: boolean;
  onInstall: () => void;
}) {
  const { t } = useTranslation();
  const install = onInstall;
  const cardHeader = (title: string, extra?: ReactNode) => <CardHeader title={title} extra={extra} />;

  const installCard = (
    <section data-testid="mcp-tools-card-install" className="rounded-xl border border-border bg-card shadow-sm">
      {cardHeader(t("tools.card.install"))}
      <div className="flex items-center gap-3.5 px-4 py-4">
        <div className="min-w-0 flex-1">
          <div className="text-sm">{t("tools.installHint")}</div>
          <div className="mt-1 text-sm text-secondary">
            {t("tools.install")} · <code className="rounded bg-secondary px-1 font-mono text-sm">~/.claude.json</code>{" "}
            <code className="rounded bg-secondary px-1 font-mono text-sm">~/.codex/config.toml</code>
          </div>
        </div>
        <Button data-testid="mcp-tools-install" disabled={installing} onClick={() => void install()}>
          {t("tools.install")}
        </Button>
      </div>
      {installMsg && (
        <div
          data-testid="mcp-tools-install-result"
          className={cn("px-4 pb-4 text-sm", installOk ? "text-[var(--success)]" : "text-destructive")}
        >
          {installMsg}
        </div>
      )}
    </section>
  );
  return installCard;
}
