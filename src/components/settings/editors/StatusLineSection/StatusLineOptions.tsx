import { Input } from "@/components/ui/input";
import { Toggle, Hint } from "../_shared";
import { F, S } from "../tokens";

export type StatusLineOptionKey = "padding" | "refreshInterval" | "hideVimModeIndicator";

const isInt = (v: unknown, min: number): v is number =>
  typeof v === "number" && Number.isInteger(v) && v >= min;

/** 与 Rust `statusline.rs::apply_options` 对称：只取用户设过且合法的项。 */
export function pickStatusLineOptions(stored: Record<string, any>): Record<string, any> {
  const out: Record<string, any> = {};
  if (isInt(stored.padding, 0)) out.padding = stored.padding;
  if (isInt(stored.refreshInterval, 1)) out.refreshInterval = stored.refreshInterval;
  if (typeof stored.hideVimModeIndicator === "boolean") out.hideVimModeIndicator = stored.hideVimModeIndicator;
  return out;
}

/** 空串 → undefined（清除该项，回官方默认）；其余取整数。 */
const parse = (raw: string): number | undefined => (raw === "" ? undefined : Number(raw));

export function StatusLineOptions({
  stored,
  onChange,
  t,
}: {
  stored: Record<string, any>;
  onChange: (key: StatusLineOptionKey, value: number | boolean | undefined) => void;
  t: (key: string, def: string) => string;
}) {
  const hide = stored.hideVimModeIndicator === true;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <label style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("statusline.padding", "Padding")}</label>
        <Input type="number" min={0} step={1} data-testid="sl-opt-padding"
          style={{ fontSize: F.body, padding: S.inputPad, width: 140 }}
          value={stored.padding ?? ""}
          onChange={(e) => onChange("padding", parse(e.target.value))} />
        <Hint>{t("statusline.paddingDesc", "Extra horizontal spacing in characters; leave empty for the official default 0")}</Hint>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <label style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("statusline.refreshInterval", "Refresh interval (seconds)")}</label>
        <Input type="number" min={1} step={1} data-testid="sl-opt-refresh"
          style={{ fontSize: F.body, padding: S.inputPad, width: 140 }}
          value={stored.refreshInterval ?? ""}
          onChange={(e) => onChange("refreshInterval", parse(e.target.value))} />
        <Hint>{t("statusline.refreshIntervalDesc", "Re-run the command every N seconds, minimum 1; leave empty to refresh on events only")}</Hint>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <span data-testid="sl-opt-hidevim" data-state={hide ? "on" : "off"}
          onClick={() => onChange("hideVimModeIndicator", !hide)}
          style={{ display: "inline-flex" }}>
          <Toggle active={hide} onChange={() => {}} />
        </span>
        <div>
          <div style={{ fontSize: F.label, color: "var(--text-primary)" }}>{t("statusline.hideVimModeIndicator", "Hide built-in Vim mode indicator")}</div>
          <Hint>{t("statusline.hideVimModeIndicatorDesc", "Enable when your script renders vim.mode itself, so -- INSERT -- is not shown twice")}</Hint>
        </div>
      </div>
    </div>
  );
}
