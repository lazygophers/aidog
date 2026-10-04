// ─── 限频配置弹窗（rate-limit-aware 票 04）──────────────────
// 编辑 extra.rate_limits（平台级 RPM/TPM + per-model 覆盖）与 extra.quota_windows
//（额度窗口族，端砚墨点 5h/7d）。数据形状见 services/api/platforms.ts（票 02 拍板），
// 调度侧消费在后续后端票落地；本弹窗先保证「配得上、存得对」。
// Dialog 走 Radix Portal（liquid glass 居中由 Portal 保证，CLAUDE.md modal 铁律）。
import { useState } from "react";
import type { TFunction } from "i18next";
import {
  type RateLimitsBundle,
  type QuotaWindowConfig,
} from "../../services/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** 弹窗内部编辑态：数字一律字符串持有（空串 = 未配），保存时解析。 */
export interface RateLimitsDraft {
  rpm: string;
  tpm: string;
  models: { model: string; rpm: string; tpm: string }[];
  windows: { window: string; budgetTokens: string; budgetPoints: string }[];
}

/** bundle → 弹窗编辑态。 */
export function bundleToDraft(b: RateLimitsBundle): RateLimitsDraft {
  return {
    rpm: b.rate_limits?.rpm ? String(b.rate_limits.rpm) : "",
    tpm: b.rate_limits?.tpm ? String(b.rate_limits.tpm) : "",
    models: Object.entries(b.rate_limits?.models ?? {}).map(([model, o]) => ({
      model,
      rpm: o.rpm ? String(o.rpm) : "",
      tpm: o.tpm ? String(o.tpm) : "",
    })),
    windows: (b.quota_windows ?? []).map(w => ({
      window: String(w.window),
      budgetTokens: w.budget_tokens ? String(w.budget_tokens) : "",
      budgetPoints: w.budget_points ? String(w.budget_points) : "",
    })),
  };
}

/** 弹窗编辑态 → bundle（净化：非法数字丢弃；空即 undefined，serialize 侧删键）。 */
export function draftToBundle(d: RateLimitsDraft): RateLimitsBundle {
  const num = (s: string): number | undefined => {
    const v = Math.floor(Number(s));
    return s.trim() !== "" && Number.isFinite(v) && v > 0 ? v : undefined;
  };
  const models: Record<string, { rpm?: number; tpm?: number }> = {};
  for (const m of d.models) {
    const key = m.model.trim();
    if (!key) continue;
    const o = { rpm: num(m.rpm), tpm: num(m.tpm) };
    if (o.rpm !== undefined || o.tpm !== undefined) models[key] = o;
  }
  const rate_limits = {
    rpm: num(d.rpm),
    tpm: num(d.tpm),
    models: Object.keys(models).length > 0 ? models : undefined,
  };
  const windows: QuotaWindowConfig[] = [];
  for (const w of d.windows) {
    const win = num(w.window);
    if (win === undefined) continue;
    // spec 票 02：budget_tokens / budget_points 二选一——同填时取 tokens 优先。
    const budget_tokens = num(w.budgetTokens);
    const q: QuotaWindowConfig = {
      window: win,
      budget_tokens,
      budget_points: budget_tokens !== undefined ? undefined : num(w.budgetPoints),
    };
    if (q.budget_tokens !== undefined || q.budget_points !== undefined) windows.push(q);
  }
  return {
    rate_limits: rate_limits.rpm !== undefined || rate_limits.tpm !== undefined || rate_limits.models ? rate_limits : undefined,
    quota_windows: windows.length > 0 ? windows : undefined,
  };
}

export function RateLimitsEditModal({ open, bundle, onSave, onClose, t }: {
  open: boolean;
  bundle: RateLimitsBundle;
  onSave: (b: RateLimitsBundle) => void;
  onClose: () => void;
  t: TFunction;
}) {
  const [draft, setDraft] = useState<RateLimitsDraft>(() => bundleToDraft(bundle));
  // 每次打开重置为最新 bundle（关闭期间外部可能已变）
  const [wasOpen, setWasOpen] = useState(false);
  if (open && !wasOpen) {
    setWasOpen(true);
    setDraft(bundleToDraft(bundle));
  } else if (!open && wasOpen) {
    setWasOpen(false);
  }

  const upd = <K extends keyof RateLimitsDraft>(k: K, v: RateLimitsDraft[K]) =>
    setDraft(d => ({ ...d, [k]: v }));

  const inputStyle = { fontSize: 12, height: 28, minWidth: 70 };
  const labelStyle = { fontSize: 11, opacity: 0.7 };

  return (
    <Dialog open={open} onOpenChange={o => { if (!o) onClose(); }}>
      <DialogContent style={{ maxWidth: 520 }}>
        <DialogHeader>
          <DialogTitle>{t("platform.rateLimits.title", "限频配置")}</DialogTitle>
          <DialogDescription>
            {t("platform.rateLimits.desc", "按平台套餐配置 RPM/TPM 与额度窗口；调度侧超限自动排除候选（后续版本生效）。")}
          </DialogDescription>
        </DialogHeader>

        <div style={{ display: "flex", flexDirection: "column", gap: 12, maxHeight: "55vh", overflowY: "auto", padding: "0 4px" }}>
          {/* 平台级 RPM/TPM */}
          <div style={{ display: "flex", gap: 10 }}>
            <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: 1 }}>
              <span style={labelStyle}>{t("platform.rateLimits.rpm", "RPM（请求/分钟）")}</span>
              <Input type="number" min={0} value={draft.rpm} onChange={e => upd("rpm", e.target.value)} style={inputStyle} placeholder="—" />
            </label>
            <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: 1 }}>
              <span style={labelStyle}>{t("platform.rateLimits.tpm", "TPM（token/分钟）")}</span>
              <Input type="number" min={0} value={draft.tpm} onChange={e => upd("tpm", e.target.value)} style={inputStyle} placeholder="—" />
            </label>
          </div>

          {/* 模型级覆盖 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span style={labelStyle}>{t("platform.rateLimits.modelOverrides", "模型级覆盖（缺省回落平台级）")}</span>
            {draft.models.map((m, i) => (
              <div key={i} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <Input value={m.model} placeholder="model id" onChange={e => upd("models", draft.models.map((x, j) => j === i ? { ...x, model: e.target.value } : x))} style={{ ...inputStyle, flex: 1, minWidth: 110 }} />
                <Input type="number" value={m.rpm} placeholder="RPM" onChange={e => upd("models", draft.models.map((x, j) => j === i ? { ...x, rpm: e.target.value } : x))} style={inputStyle} />
                <Input type="number" value={m.tpm} placeholder="TPM" onChange={e => upd("models", draft.models.map((x, j) => j === i ? { ...x, tpm: e.target.value } : x))} style={inputStyle} />
                <Button variant="ghost" size="icon" style={{ height: 28, width: 28 }} onClick={() => upd("models", draft.models.filter((_, j) => j !== i))}>×</Button>
              </div>
            ))}
            <Button variant="outline" style={{ fontSize: 12, height: 28, width: "fit-content" }} onClick={() => upd("models", [...draft.models, { model: "", rpm: "", tpm: "" }])}>
              + {t("platform.rateLimits.addModel", "加模型覆盖")}
            </Button>
          </div>

          {/* 额度窗口 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span style={labelStyle}>{t("platform.rateLimits.quotaWindows", "额度窗口（5h=18000 / 7d=604800 秒）")}</span>
            {draft.windows.map((w, i) => (
              <div key={i} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <Input type="number" value={w.window} placeholder={t("platform.rateLimits.windowSec", "秒")} onChange={e => upd("windows", draft.windows.map((x, j) => j === i ? { ...x, window: e.target.value } : x))} style={inputStyle} />
                <Input type="number" value={w.budgetTokens} placeholder={t("platform.rateLimits.budgetTokens", "token 预算")} onChange={e => upd("windows", draft.windows.map((x, j) => j === i ? { ...x, budgetTokens: e.target.value } : x))} style={inputStyle} />
                <Input type="number" value={w.budgetPoints} placeholder={t("platform.rateLimits.budgetPoints", "点数预算")} onChange={e => upd("windows", draft.windows.map((x, j) => j === i ? { ...x, budgetPoints: e.target.value } : x))} style={inputStyle} />
                <Button variant="ghost" size="icon" style={{ height: 28, width: 28 }} onClick={() => upd("windows", draft.windows.filter((_, j) => j !== i))}>×</Button>
              </div>
            ))}
            <div style={{ display: "flex", gap: 6 }}>
              <Button variant="outline" style={{ fontSize: 12, height: 28 }} onClick={() => upd("windows", [...draft.windows, { window: "18000", budgetTokens: "", budgetPoints: "" }])}>
                + 5h
              </Button>
              <Button variant="outline" style={{ fontSize: 12, height: 28 }} onClick={() => upd("windows", [...draft.windows, { window: "604800", budgetTokens: "", budgetPoints: "" }])}>
                + 7d
              </Button>
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose} style={{ fontSize: 12, height: 30 }}>{t("action.cancel", "取消")}</Button>
          <Button onClick={() => { onSave(draftToBundle(draft)); onClose(); }} style={{ fontSize: 12, height: 30 }}>{t("action.save", "保存")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
