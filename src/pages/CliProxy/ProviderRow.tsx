import { memo } from "react";
import { useReveal, makeRipple } from "@/utils/motion";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import type { CliProxyProvider } from "@/services/api";
import { quotaTypeOf } from "./quotaTypeOf";

// ProviderRow: 单行包装（萤火虫化 SectionCard idiom, 见 memory section-card-reveal-wrapper-idiom）
// 每行独立 useReveal(idx*60) 错峰；glass-surface + hover-lift + reveal；CTA ripple。
// ponytail: memo + 原始类型 props（boolean/string/回调），父级 state 变更不击穿未变行。
interface ProviderRowProps {
  p: CliProxyProvider;
  idx: number;
  selectMode: boolean;
  selected: boolean;
  busy: boolean;
  onToggle: () => void;
  onTest: () => void;
  onCreatePlatform: () => void;
  onEdit: () => void;
  onDelete: () => void;
  t: (k: string, o?: any) => string;
}
export const ProviderRow = memo(function ProviderRow({
  p, idx, selectMode, selected, busy,
  onToggle, onTest, onCreatePlatform, onEdit, onDelete, t,
}: ProviderRowProps) {
  const { ref, shown } = useReveal<HTMLDivElement>(idx * 60);
  return (
    <div
      ref={ref}
      className={`glass-surface hover-lift reveal${shown ? " in" : ""}`}
      style={{
        padding: "12px 14px",
        display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap",
      }}
    >
      {selectMode && (
        <Checkbox
          checked={selected}
          onCheckedChange={onToggle}
          style={{ flexShrink: 0 }}
        />
      )}
      <div style={{ minWidth: 160 }}>
        <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>{p.name}</div>
        <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{p.wire_protocol}</div>
      </div>
      <div style={{ flex: 1, minWidth: 200, fontSize: 12, color: "var(--text-secondary)", wordBreak: "break-all" }}>
        {p.base_url}
      </div>
      <span style={{
        padding: "2px 8px", borderRadius: 6, fontSize: 11,
        border: `1px solid ${p.status === "active" ? "var(--color-success)" : "var(--text-tertiary)"}`,
        color: p.status === "active" ? "var(--color-success)" : "var(--text-tertiary)",
      }}>
        {p.status === "active" ? t("cliProxy.statusActive") : t("cliProxy.statusDisabled")}
      </span>
      {quotaTypeOf(p.quota) === "newapi" && (
        <span style={{
          padding: "2px 8px", borderRadius: 6, fontSize: 11,
          border: "1px solid var(--accent)", color: "var(--accent)",
        }}>
          {t("cliProxy.quotaTypeNewapi")}
        </span>
      )}
      <div style={{ display: "flex", gap: 6 }}>
        <Button
          variant="ghost"
          className="ripple"
          onClick={(e) => { makeRipple(e); onTest(); }}
          disabled={busy}
          title={t("cliProxy.test")}
          style={{ height: "auto", padding: "4px 10px", fontSize: 12 }}
        >
          {t("cliProxy.test")}
        </Button>
        <Button
          variant="ghost"
          className="ripple"
          onClick={(e) => { makeRipple(e); onCreatePlatform(); }}
          disabled={busy}
          title={t("cliProxy.createPlatform")}
          style={{ height: "auto", padding: "4px 10px", fontSize: 12 }}
        >
          {t("cliProxy.createPlatform")}
        </Button>
        <Button
          variant="ghost"
          className="ripple"
          onClick={(e) => { makeRipple(e); onEdit(); }}
          disabled={busy}
          style={{ height: "auto", padding: "4px 10px", fontSize: 12 }}
        >
          {t("cliProxy.edit")}
        </Button>
        <Button
          variant="destructive"
          className="ripple"
          onClick={(e) => { makeRipple(e); onDelete(); }}
          disabled={busy}
          style={{ height: "auto", padding: "4px 10px", fontSize: 12 }}
        >
          {t("cliProxy.delete")}
        </Button>
      </div>
    </div>
  );
});
