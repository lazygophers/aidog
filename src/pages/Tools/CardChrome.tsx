// 卡片共用小件（票 57-03）：卡头 / 卡脚 / 开关行，JSX 与拆分前逐字一致。
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { Switch } from "../../components/ui/switch";

export function CardFooter({
  card,
  dirty,
  savedCard,
  saveError,
  saveCard,
  saveTestId,
}: {
  card: "search" | "fetch";
  dirty: boolean;
  savedCard: "" | "search" | "fetch";
  saveError: string;
  saveCard: (c: "search" | "fetch") => void;
  saveTestId: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="mt-4 flex items-center gap-3 rounded-b-xl border-t border-border bg-secondary px-4 py-2.5">
      {dirty && (
        <span
          data-testid={`tools-dirty-${card}`}
          className="flex items-center gap-1.5 text-sm text-[var(--warning)]"
        >
          <span className="size-1.5 rounded-full bg-current" />
          {t("tools.unsaved.badge")}
        </span>
      )}
      <span className="flex-1" />
      {savedCard === card && !dirty && (
        <span data-testid="mcp-tools-saved" className="text-sm text-[var(--success)]">{t("tools.saved")}</span>
      )}
      {saveError && (
        <span data-testid="mcp-tools-save-error" className="text-sm text-destructive">{saveError}</span>
      )}
      <Button size="sm" data-testid={saveTestId} onClick={() => void saveCard(card)}>
        {t("tools.saveCard")}
      </Button>
    </div>
  );
}

export function CardHeader({ title, extra }: { title: string; extra?: ReactNode }) {
  return (
    <div className="flex items-center gap-2.5 border-b border-border px-4 py-3">
      <h3 className="text-sm font-semibold">{title}</h3>
      <span className="flex-1" />
      {extra}
    </div>
  );
}

export function SwitchRow({
  testid,
  checked,
  onToggle,
  label,
}: {
  testid: string;
  checked: boolean;
  onToggle: () => void;
  label: string;
}) {
  return (
    <label className="flex items-center gap-2.5 text-sm">
      <Switch data-testid={testid} checked={checked} onCheckedChange={onToggle} />
      {label}
    </label>
  );
}
