// 策略库区：列出平台 preset 的 rate_limit_policies，导入 = 覆盖 extra.rate_limits（确认后）。
import { useEffect, useState } from "react";
import type { TFunction } from "i18next";
import { getDefaultRateLimitPolicies, type RateLimitPolicy } from "../../domains/platforms";
import type { Protocol, RateLimitsConfig } from "../../services/api";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "../../components/ui/alert-dialog";
import { formatNumber } from "../../utils/formatters";

/** 策略 → 平台限频配置（深拷贝；内置原件不变），记录来源策略 id 与 platform.json last_updated。 */
export function policyToRateLimits(p: RateLimitPolicy, lastUpdated: number): RateLimitsConfig {
  const { concurrency, rpm, tpm, models, windows } = structuredClone(p);
  return {
    concurrency, rpm, tpm,
    models: models && Object.keys(models).length > 0 ? models : undefined,
    windows: windows && windows.length > 0 ? windows : undefined,
    source_policy_id: p.id,
    source_last_updated: lastUpdated,
  };
}

const nums = (p: RateLimitPolicy) => [
  p.concurrency && `${formatNumber(p.concurrency)} CONC`,
  p.rpm && `${formatNumber(p.rpm)} RPM`,
  p.tpm && `${formatNumber(p.tpm)} TPM`,
].filter(Boolean) as string[];

export function RateLimitPolicyLibrary({ protocol, source, onImport, t }: {
  protocol: Protocol;
  /** 当前配置记录的来源（extra.rate_limits.source_*）。 */
  source?: { id?: string; at?: number };
  onImport: (cfg: RateLimitsConfig) => void;
  t: TFunction;
}) {
  const [lib, setLib] = useState<{ policies: RateLimitPolicy[]; lastUpdated: number }>({ policies: [], lastUpdated: 0 });
  const [pending, setPending] = useState<RateLimitPolicy | null>(null);
  useEffect(() => {
    let cancelled = false;
    getDefaultRateLimitPolicies(protocol).then(r => { if (!cancelled) setLib(r); }).catch(() => {});
    return () => { cancelled = true; };
  }, [protocol]);

  if (lib.policies.length === 0) return null;
  const stale = !!source?.id && (source.at ?? 0) < lib.lastUpdated;
  return (
    <div id="rl-policy-library" data-state={stale ? "stale" : source?.id ? "current" : "none"}
      style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 10 }}>
      <span style={{ fontSize: 11, opacity: 0.7, color: "var(--text-tertiary)" }}>
        {t("platform.rateLimits.policyLibrary", "策略库（内置限频策略）")}
      </span>
      {stale && (
        <div id="rl-policy-updated" style={{ fontSize: 12, padding: "6px 10px", borderRadius: "var(--radius-sm)", background: "var(--accent-subtle)", color: "var(--text-primary)" }}>
          {t("platform.rateLimits.policyUpdated", "内置策略已更新，可重新导入")}
        </div>
      )}
      {lib.policies.map(p => {
        const n = nums(p);
        return (
          <div key={p.id} id={`rl-policy-${p.id}`} data-state={source?.id === p.id ? "imported" : "idle"}
            style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", padding: "6px 10px", borderRadius: "var(--radius-sm)", background: "var(--card)", border: "1px solid var(--border)" }}>
            <strong style={{ fontSize: 12 }}>{p.label}</strong>
            <span style={{ fontSize: 11, color: "var(--text-secondary)", fontVariantNumeric: "tabular-nums" }}>
              {n.length > 0 ? n.join(" · ") : t("platform.rateLimits.policyNoNumbers", "官方未公开平台级数字")}
              {p.models && Object.keys(p.models).length > 0 && ` · ${t("platform.rateLimits.modelCount", "{{n}} 个模型覆盖", { n: Object.keys(p.models).length })}`}
            </span>
            <a href={p.source_url} target="_blank" rel="noreferrer" style={{ fontSize: 11, color: "var(--primary)" }}>
              {t("platform.rateLimits.policySource", "出处")}
            </a>
            <button type="button" id={`rl-policy-import-${p.id}`} className="btn-ghost"
              style={{ marginLeft: "auto", fontSize: 11, padding: "2px 10px" }} onClick={() => setPending(p)}>
              {t("platform.rateLimits.policyImport", "导入")}
            </button>
          </div>
        );
      })}
      <AlertDialog open={pending !== null} onOpenChange={next => { if (!next) setPending(null); }}>
        <AlertDialogContent id="rl-import-confirm" className="glass-elevated" style={{ maxWidth: 440, padding: "20px 22px" }}>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("platform.rateLimits.importConfirmTitle", "导入策略 {{label}}？", { label: pending?.label ?? "" })}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("platform.rateLimits.importConfirmDesc", "将覆盖当前限频配置（并发 / RPM / TPM、模型级、时段窗口）。导入后是你自己的一份，可再改；内置原件不变。")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel id="rl-import-confirm-cancel">{t("action.cancel", "取消")}</AlertDialogCancel>
            <AlertDialogAction id="rl-import-confirm-ok" onClick={() => { if (pending) onImport(policyToRateLimits(pending, lib.lastUpdated)); setPending(null); }}>
              {t("platform.rateLimits.policyImport", "导入")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
