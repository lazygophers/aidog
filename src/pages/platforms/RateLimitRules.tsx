// 限频规则列表：每条规则 = 模型（下拉多选 + 手输，支持 `前缀*`）和 / 或时段 + 并发 / RPM / TPM。
// 重叠时按列表顺序（靠前优先），可拖动排序。没填的上限 = 该规则生效时不限。
import { useState } from "react";
import type { TFunction } from "i18next";
import { SortableList } from "../../components/SortableList";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { rateLimitRuleError, type Protocol, type RateLimitRule } from "../../services/api";
import { ModelCombobox } from "./ModelCombobox";

/** 编辑态：数字一律字符串持有（空串 = 未填）；id 仅供拖动排序，不落库。 */
export interface RateLimitRuleDraft {
  id: string;
  models: string[];
  start: string; end: string; tz: string; days: number[];
  concurrency: string; rpm: string; tpm: string;
}

let seq = 0;
export const newRuleDraft = (): RateLimitRuleDraft => ({
  id: `rl-rule-${++seq}`, models: [], start: "", end: "", tz: "", days: [], concurrency: "", rpm: "", tpm: "",
});

const str = (n?: number) => (n ? String(n) : "");

export function ruleToDraft(r: RateLimitRule): RateLimitRuleDraft {
  return {
    ...newRuleDraft(),
    models: [...(r.models ?? [])],
    start: r.start_hour !== undefined ? String(r.start_hour) : "",
    end: r.end_hour !== undefined ? String(r.end_hour) : "",
    tz: r.timezone ?? "",
    days: [...(r.days_of_week ?? [])],
    concurrency: str(r.concurrency), rpm: str(r.rpm), tpm: str(r.tpm),
  };
}

/** 草稿 → 规则：非法数字丢弃、空字段不写出；条件缺失的规则也保留，由保存前校验报错。 */
export function draftToRule(d: RateLimitRuleDraft): RateLimitRule {
  const pos = (s: string) => {
    const v = Math.floor(Number(s));
    return s.trim() !== "" && Number.isFinite(v) && v > 0 ? v : undefined;
  };
  const hour = (s: string, max: number) => {
    const v = Number(s);
    return s.trim() !== "" && Number.isInteger(v) && v >= 0 && v <= max ? v : undefined;
  };
  const out: RateLimitRule = {};
  const models = d.models.map(m => m.trim()).filter(Boolean);
  if (models.length > 0) out.models = models;
  const sh = hour(d.start, 23), eh = hour(d.end, 24);
  if (sh !== undefined) out.start_hour = sh;
  if (eh !== undefined) out.end_hour = eh;
  if (d.tz.trim()) out.timezone = d.tz.trim();
  if (d.days.length > 0) out.days_of_week = [...d.days];
  const c = pos(d.concurrency), rpm = pos(d.rpm), tpm = pos(d.tpm);
  if (c !== undefined) out.concurrency = c;
  if (rpm !== undefined) out.rpm = rpm;
  if (tpm !== undefined) out.tpm = tpm;
  return out;
}

const numStyle = { fontSize: 12, height: 28, minWidth: 70, textAlign: "center", fontVariantNumeric: "tabular-nums" } as React.CSSProperties;
const miniLabel = { fontSize: 11, color: "var(--text-tertiary)" } as const;

function ModelsPicker({ i, value, onChange, options, protocol, t }: {
  i: number; value: string[]; onChange: (v: string[]) => void; options: string[]; protocol: Protocol; t: TFunction;
}) {
  const [draft, setDraft] = useState("");
  const add = (v: string) => {
    const m = v.trim();
    setDraft("");
    if (m && !value.includes(m)) onChange([...value, m]);
  };
  const toggle = (v: string) => (value.includes(v) ? onChange(value.filter(m => m !== v)) : add(v));
  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: 4, alignItems: "center", flex: "1 1 260px", minWidth: 200 }}>
      {value.map(m => (
        <span key={m} data-testid={`rl-rule-${i}-chip-${m}`} className="model-chip"
          style={{ display: "inline-flex", alignItems: "center", gap: 4, padding: "2px 8px", fontSize: 12, borderRadius: "var(--radius-sm)", border: "1px solid var(--border)" }}>
          <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 11 }}>{m}</span>
          <button type="button" data-testid={`rl-rule-${i}-chip-remove-${m}`} className="model-chip-remove"
            aria-label={t("platform.modelChain.remove", "移除")}
            onClick={() => onChange(value.filter(x => x !== m))}
            style={{ cursor: "pointer", color: "var(--text-tertiary)", background: "transparent", border: 0, padding: "0 2px" }}>✕</button>
        </span>
      ))}
      <div style={{ flex: "1 1 160px", minWidth: 160, maxWidth: 300 }}>
        <ModelCombobox
          value={draft} onChange={setDraft} onSelect={toggle}
          source={options} selected={value} protocol={protocol}
          placeholder={t("platform.rateLimits.ruleModelsPlaceholder", "模型（可选，回车添加，支持 前缀*）")}
          inputProps={{
            "data-testid": `rl-rule-${i}-model-input`,
            onKeyDown: e => { if (e.key === "Enter") { e.preventDefault(); add(draft); } },
          }}
          t={t}
        />
      </div>
    </div>
  );
}

export function RateLimitRulesList({ rules, onChange, modelOptions, protocol, t }: {
  rules: RateLimitRuleDraft[];
  onChange: (next: RateLimitRuleDraft[]) => void;
  modelOptions: string[];
  protocol: Protocol;
  t: TFunction;
}) {
  const upd = (i: number, patch: Partial<RateLimitRuleDraft>) => onChange(rules.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const errText = (e: "noCondition" | "badWildcard") => e === "noCondition"
    ? t("platform.rateLimits.ruleNoCondition", "至少选一个模型或填一个时段，否则无法保存")
    : t("platform.rateLimits.ruleBadWildcard", "通配符只能是末尾单个 *，且前缀不能为空（如 glm-*）");
  return (
    <div data-testid="rl-rules" style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <SortableList
        items={rules}
        onReorder={onChange}
        renderItem={(r, handle) => {
          const i = rules.findIndex(x => x.id === r.id);
          const err = rateLimitRuleError(draftToRule(r));
          return (
            <div data-testid={`rl-rule-${i}`} style={{ display: "flex", flexDirection: "column", gap: 2, padding: "6px 8px", marginBottom: 4, borderRadius: "var(--radius-sm)", border: "1px solid var(--border)", background: "var(--card)" }}>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
                <span ref={handle.ref} data-testid={`rl-rule-${i}-handle`} {...handle.attributes} {...handle.listeners}
                  aria-label={t("platform.rateLimits.ruleDrag", "拖动排序（靠前优先）")}
                  title={t("platform.rateLimits.ruleDrag", "拖动排序（靠前优先）")}
                  style={{ cursor: "grab", color: "var(--text-tertiary)", touchAction: "none", userSelect: "none" }}>⠿</span>
                <ModelsPicker i={i} value={r.models} onChange={models => upd(i, { models })} options={modelOptions} protocol={protocol} t={t} />
                <Button variant="ghost" size="icon" className="btn-danger" data-testid={`rl-rule-${i}-remove`} style={{ height: 28, width: 28, marginLeft: "auto" }}
                  aria-label={t("platform.delete", "删除")} onClick={() => onChange(rules.filter((_, j) => j !== i))}>×</Button>
              </div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
                <Input data-testid={`rl-rule-${i}-start`} type="number" min={0} max={23} value={r.start} placeholder="0" aria-label={t("platform.start_hour", "起")} onChange={e => upd(i, { start: e.target.value })} style={{ ...numStyle, width: 56, minWidth: 56 }} />
                <span style={miniLabel}>–</span>
                <Input data-testid={`rl-rule-${i}-end`} type="number" min={0} max={24} value={r.end} placeholder="24" aria-label={t("platform.end_hour", "止")} onChange={e => upd(i, { end: e.target.value })} style={{ ...numStyle, width: 56, minWidth: 56 }} />
                <Input data-testid={`rl-rule-${i}-tz`} value={r.tz} placeholder="UTC / Asia/Shanghai" aria-label={t("platform.rateLimits.windowTz", "时区")} onChange={e => upd(i, { tz: e.target.value })} style={{ fontSize: 11, height: 28, width: 130, fontFamily: "var(--font-mono, monospace)" }} />
                <div style={{ display: "flex", gap: 2 }}>
                  {[0, 1, 2, 3, 4, 5, 6].map(d => (
                    <button key={d} type="button" data-testid={`rl-rule-${i}-day-${d}`} data-state={r.days.includes(d) ? "on" : "off"}
                      className="btn-ghost" style={{ padding: "0 5px", minWidth: 22, fontSize: 11, ...(r.days.includes(d) ? { background: "var(--accent-subtle)", color: "var(--text-primary)" } : {}) }}
                      onClick={() => upd(i, { days: (r.days.includes(d) ? r.days.filter(x => x !== d) : [...r.days, d]).sort((a, b) => a - b) })}>
                      {t(`platform.weekday_short.${d}`)}
                    </button>
                  ))}
                </div>
                <Input data-testid={`rl-rule-${i}-concurrency`} type="number" min={0} value={r.concurrency} placeholder={t("platform.rateLimits.concurrencyShort", "并发")} onChange={e => upd(i, { concurrency: e.target.value })} style={numStyle} />
                <Input data-testid={`rl-rule-${i}-rpm`} type="number" min={0} value={r.rpm} placeholder="RPM" onChange={e => upd(i, { rpm: e.target.value })} style={numStyle} />
                <Input data-testid={`rl-rule-${i}-tpm`} type="number" min={0} value={r.tpm} placeholder="TPM" onChange={e => upd(i, { tpm: e.target.value })} style={numStyle} />
              </div>
              {err && (
                <span role="alert" data-testid={`rl-rule-${i}-error`} data-state={err} style={{ fontSize: 11, color: "var(--destructive, #c0392b)" }}>{errText(err)}</span>
              )}
            </div>
          );
        }}
      />
      <button type="button" data-testid="rl-rules-add" className="btn-ghost" style={{ alignSelf: "flex-start", fontSize: 11, padding: "2px 10px" }}
        onClick={() => onChange([...rules, newRuleDraft()])}>
        + {t("platform.rateLimits.addRule", "加限频规则")}
      </button>
      <p data-testid="rl-rules-hint" style={{ ...miniLabel, margin: 0 }}>
        {t("platform.rateLimits.rulesHint", "没填的项在该规则生效时不限；多条规则重叠时按列表顺序，靠前的优先（可拖动）。纯时段规则命中时替换平台级上限；模型 + 时段优先于仅模型。")}
      </p>
    </div>
  );
}
