// 策略库区：列出平台 preset 的 rate_limit_policies，导入 = 覆盖 extra.rate_limits（确认后；形状相同，直接拷贝）。
import { useEffect, useState } from "react";
import type { TFunction } from "i18next";
import { getDefaultRateLimitPolicies, type RateLimitPolicy } from "../../domains/platforms";
import type { Protocol, RateLimitsConfig, RateLimitModelOverride, RateLimitRule } from "../../services/api";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "../../components/ui/alert-dialog";
import { formatInteger } from "../../utils/formatters";
import { Button } from "../../components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "../../components/ui/dialog";

/** 策略 → 平台限频配置（深拷贝；内置原件不变），记录来源策略 id 与 platform.json last_updated。 */
export function policyToRateLimits(p: RateLimitPolicy, lastUpdated: number): RateLimitsConfig {
  const { concurrency, rpm, tpm, rules } = structuredClone(p);
  return {
    concurrency, rpm, tpm,
    rules: rules && rules.length > 0 ? rules : undefined,
    source_policy_id: p.id,
    source_last_updated: lastUpdated,
  };
}

const nums = (p: RateLimitModelOverride, t: TFunction) => [
  p.concurrency && `${t("platform.rateLimits.concurrencyShort", "Conc.")} ${formatInteger(p.concurrency)}`,
  p.rpm && `RPM ${formatInteger(p.rpm)}`,
  p.tpm && `TPM ${formatInteger(p.tpm)}`,
].filter(Boolean) as string[];

/** 全策略（平台级 + 每条规则）都无任何限频数字 = 导入不产生限制。 */
function policyIsEmpty(p: RateLimitPolicy, t: TFunction): boolean {
  return nums(p, t).length === 0 && (p.rules ?? []).every(r => nums(r, t).length === 0);
}

const COLLAPSE_AT = 5;
const detailStyle = { fontSize: 14, color: "var(--text-secondary)", fontVariantNumeric: "tabular-nums" } as const;

function ruleKind(r: RateLimitRule): "model" | "window" | "model-window" {
  const hasModels = (r.models ?? []).length > 0;
  const hasTime = r.start_hour !== undefined || r.end_hour !== undefined || r.days_of_week !== undefined;
  return hasModels && hasTime ? "model-window" : hasModels ? "model" : "window";
}

function RuleTime({ r, t }: { r: RateLimitRule; t: TFunction }) {
  return (
    <span>
      {r.start_hour !== undefined && r.end_hour !== undefined && `${r.start_hour}:00–${r.end_hour}:00`}
      {r.days_of_week && r.days_of_week.length > 0 && ` · ${t("platform.rateLimits.policyDays", "Days {{days}}", { days: r.days_of_week.join(",") })}`}
      {` · ${r.timezone ?? "UTC"}`}
    </span>
  );
}

function PolicyDetail({ p, expanded, onToggle, t }: { p: RateLimitPolicy; expanded: boolean; onToggle: () => void; t: TFunction }) {
  const n = nums(p, t);
  const rules = p.rules ?? [];
  const empty = policyIsEmpty(p, t);
  const shown = expanded ? rules : rules.slice(0, COLLAPSE_AT);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 3, width: "100%" }}>
      {empty ? (
        <span id={`rl-policy-empty-${p.id}`} style={detailStyle}>
          {t("platform.rateLimits.policyEmpty", "No rate-limit numbers published by the vendor; importing adds no limits")}
        </span>
      ) : (
        <span id={`rl-policy-platform-${p.id}`} style={detailStyle}>
          {n.length > 0 ? n.join(" · ") : t("platform.rateLimits.policyNoNumbers", "No platform-level numbers published")}
        </span>
      )}
      {rules.length > 0 && (
        <div id={`rl-policy-rules-${p.id}`} style={{ display: "flex", flexDirection: "column", gap: 1 }}>
          <span style={{ fontSize: 14, color: "var(--text-tertiary)" }}>
            {t("platform.rateLimits.policyRules", "Limit rules ({{n}})", { n: rules.length })}
          </span>
          {shown.map((r, i) => (
            <div key={i} id={`rl-policy-rule-${p.id}-${i}`} data-kind={ruleKind(r)}
              style={{ ...detailStyle, display: "flex", gap: 8, justifyContent: "space-between", flexWrap: "wrap", paddingLeft: 8 }}>
              <span style={{ overflowWrap: "anywhere" }}>
                {(r.models ?? []).join(", ")}
                {(r.models ?? []).length > 0 && ruleKind(r) === "model-window" && " · "}
                {ruleKind(r) !== "model" && <RuleTime r={r} t={t} />}
              </span>
              <span>{nums(r, t).join(" · ")}</span>
            </div>
          ))}
          {rules.length > COLLAPSE_AT && (
            <button type="button" id={`rl-policy-toggle-${p.id}`} data-state={expanded ? "open" : "closed"} className="btn-ghost"
              style={{ alignSelf: "flex-start", fontSize: 14, padding: "0 8px" }} onClick={onToggle}>
              {expanded
                ? t("platform.rateLimits.policyCollapse", "Collapse")
                : t("platform.rateLimits.policyExpand", "Show all {{n}} rules", { n: rules.length })}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export function RateLimitPolicyLibrary({ protocol, source, onImport, t, headerOnly }: {
  protocol: Protocol;
  /** 当前配置记录的来源（extra.rate_limits.source_*）。 */
  source?: { id?: string; at?: number };
  onImport: (cfg: RateLimitsConfig) => void;
  t: TFunction;
  /** true = 仅渲染右上角入口按钮（策略列表与确认弹窗由独立挂载实例渲染）。 */
  headerOnly?: boolean;
}) {
  const [lib, setLib] = useState<{ policies: RateLimitPolicy[]; lastUpdated: number }>({ policies: [], lastUpdated: 0 });
  const [pending, setPending] = useState<RateLimitPolicy | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [open, setOpen] = useState(false);
  const [emptyFeedback, setEmptyFeedback] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    getDefaultRateLimitPolicies(protocol).then(r => { if (!cancelled) setLib(r); }).catch(() => {});
    return () => { cancelled = true; };
  }, [protocol]);

  if (lib.policies.length === 0 || (headerOnly && lib.policies.every(p => policyIsEmpty(p, t)))) return null;
  const stale = !!source?.id && (source.at ?? 0) < lib.lastUpdated;
  const staleTip = stale ? t("platform.rateLimits.policyUpdated", "Built-in policy updated, re-import available") : undefined;
  if (headerOnly) {
    // 右上角入口形态（对齐高峰「Import default」按钮）：无策略 / 已导入时给态徽标。
    const current = source?.id && lib.policies.some(p => p.id === source.id);
    return (
      <div id="rl-policy-header" data-state={stale ? "stale" : current ? "current" : "none"} style={{ display: "flex", alignItems: "center", gap: 6 }}>
        {staleTip && (
          <span id="rl-policy-updated-badge" title={staleTip} style={{ fontSize: 14, padding: "2px 8px", borderRadius: "var(--radius-sm)", background: "var(--accent-subtle)", color: "var(--text-primary)" }}>
            {staleTip}
          </span>
        )}
        <Button variant="ghost" size="sm" style={{ padding: "2px 8px", fontSize: 14, whiteSpace: "nowrap", height: "auto" }} onClick={() => setOpen(true)}>
          {t("platform.peak_import_default", "Import default")}
        </Button>
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogContent aria-describedby={undefined} style={{ maxHeight: "80vh", overflowY: "auto" }}>
            <DialogTitle>{t("platform.rateLimits.policyLibrary", "Policy library (built-in rate-limit policies)")}</DialogTitle>
            <RateLimitPolicyLibrary protocol={protocol} source={source} onImport={cfg => { onImport(cfg); setOpen(false); }} t={t} />
          </DialogContent>
        </Dialog>
      </div>
    );
  }
  return (
    <div id="rl-policy-library" data-state={stale ? "stale" : source?.id ? "current" : "none"}
      style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 10 }}>
      <span style={{ fontSize: 14, opacity: 0.7, color: "var(--text-tertiary)" }}>
        {t("platform.rateLimits.policyLibrary", "Policy library (built-in rate-limit policies)")}
      </span>
      {stale && (
        <div id="rl-policy-updated" style={{ fontSize: 14, padding: "6px 10px", borderRadius: "var(--radius-sm)", background: "var(--accent-subtle)", color: "var(--text-primary)" }}>
          {staleTip}
        </div>
      )}
      {lib.policies.map(p => {
        const empty = policyIsEmpty(p, t);
        return (
          <div key={p.id} id={`rl-policy-${p.id}`} data-state={source?.id === p.id ? "imported" : empty ? "empty" : "idle"}
            style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", padding: "6px 10px", borderRadius: "var(--radius-sm)", background: "var(--card)", border: "1px solid var(--border)" }}>
            <strong style={{ fontSize: 14 }}>{p.label}</strong>
            <a href={p.source_url} target="_blank" rel="noreferrer" style={{ fontSize: 14, color: "var(--primary)" }}>
              {t("platform.rateLimits.policySource", "Source")}
            </a>
            <button type="button" id={`rl-policy-import-${p.id}`} className="btn-ghost"
              style={{ marginLeft: "auto", fontSize: 14, padding: "2px 10px" }}
              title={empty ? t("platform.rateLimits.policyEmpty", "No rate-limit numbers published by the vendor; importing adds no limits") : undefined}
              onClick={() => { if (empty) setEmptyFeedback(p.id); else { setEmptyFeedback(null); setPending(p); } }}>
              {t("platform.rateLimits.policyImport", "Import")}
            </button>
            <PolicyDetail p={p} expanded={!!expanded[p.id]} onToggle={() => setExpanded(e => ({ ...e, [p.id]: !e[p.id] }))} t={t} />
            {emptyFeedback === p.id && (
              <span id="rl-policy-empty-feedback" role="status" style={{ fontSize: 14, color: "var(--text-primary)" }}>
                {t("platform.rateLimits.policyEmpty", "No rate-limit numbers published by the vendor; importing adds no limits")}
              </span>
            )}
          </div>
        );
      })}
      <AlertDialog open={pending !== null} onOpenChange={next => { if (!next) setPending(null); }}>
        <AlertDialogContent id="rl-import-confirm" className="glass-elevated" style={{ maxWidth: 440, padding: "20px 22px" }}>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("platform.rateLimits.importConfirmTitle", "Import policy {{label}}?", { label: pending?.label ?? "" })}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("platform.rateLimits.importConfirmDesc", "This overwrites the current rate-limit config (concurrency / RPM / TPM, per-model, time windows). The imported copy is yours to edit; the built-in original is unchanged.")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel id="rl-import-confirm-cancel">{t("action.cancel", "Cancel")}</AlertDialogCancel>
            <AlertDialogAction id="rl-import-confirm-ok" onClick={() => { if (pending) onImport(policyToRateLimits(pending, lib.lastUpdated)); setPending(null); }}>
              {t("platform.rateLimits.policyImport", "Import")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
