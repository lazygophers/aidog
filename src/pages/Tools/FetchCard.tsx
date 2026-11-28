// 抓网页卡（票 57-03 自 ToolsSection 拆出）：展示层，草稿由容器 hook 持有。
import { useTranslation } from "react-i18next";
import type { ReactNode } from "react";
import { CardFooter, CardHeader, SwitchRow } from "./CardChrome";
import type { ToolsCard, ToolsCardId } from "./useToolsCard";

export type FetchDraft = { enabled: boolean; format: "markdown" | "html" | "text" };

export function FetchCard({
  card,
  savedCard,
  saveError,
  onSave,
}: {
  card: ToolsCard<FetchDraft>;
  savedCard: "" | ToolsCardId;
  saveError: string;
  onSave: (c: ToolsCardId) => void;
}) {
  const { t } = useTranslation();
  const fetchOn = card.draft.enabled;
  const setFetchOn = (v: boolean | ((prev: boolean) => boolean)) =>
    card.setDraft((d) => ({ ...d, enabled: typeof v === "function" ? v(d.enabled) : v }));
  const fetchFormat = card.draft.format;
  const setFetchFormat = (v: FetchDraft["format"]) => card.setDraft((d) => ({ ...d, format: v }));
  const dirty = { search: false, fetch: card.dirty };
  const cardFooter = (c: "search" | "fetch", saveTestId: string) => (
    <CardFooter card={c} dirty={dirty[c]} savedCard={savedCard} saveError={saveError} saveCard={onSave} saveTestId={saveTestId} />
  );
  const cardHeader = (title: string, extra?: ReactNode) => <CardHeader title={title} extra={extra} />;
  const switchRow = (testid: string, checked: boolean, onToggle: () => void, label: string) => (
    <SwitchRow testid={testid} checked={checked} onToggle={onToggle} label={label} />
  );

  const fetchCard = (
    <section data-testid="mcp-tools-card-fetch" className="rounded-xl border border-border bg-card shadow-sm">
      {cardHeader(
        t("tools.card.fetch"),
        switchRow("mcp-tools-fetch-enabled", fetchOn, () => setFetchOn((v) => !v), t("tools.fetch.enabled")),
      )}
      <div className="flex flex-col gap-3 px-4 pb-4 pt-4">
        <p className="text-sm text-secondary">{t("tools.fetch.desc")}</p>
        <label className="flex items-center gap-2.5 text-sm">
          {t("tools.fetch.format")}
          <select
            data-testid="tools-fetch-format"
            value={fetchFormat}
            onChange={(e) => setFetchFormat(e.target.value as "markdown" | "html" | "text")}
            className="h-8 flex-1 rounded-md border border-input bg-card px-2.5 text-sm outline-none transition-colors focus:border-primary"
          >
            <option value="markdown">{t("tools.fetch.formatMarkdown")}</option>
            <option value="html">{t("tools.fetch.formatHtml")}</option>
            <option value="text">{t("tools.fetch.formatText")}</option>
          </select>
        </label>
      </div>
      {cardFooter("fetch", "tools-fetch-save")}
    </section>
  );
  return fetchCard;
}
