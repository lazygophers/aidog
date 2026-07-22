import { useEffect, useState } from "react";
import type { TFunction } from "i18next";
import type { PeakWindow } from "../../domains/platforms/defaults";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** 周几按钮标签（0=Sunday…6=Saturday，与 PeakWindow.days_of_week 索引对齐）。 */
const WEEKDAY_LABELS = ["S", "M", "T", "W", "T", "F", "S"] as const;

/** 单窗口维度 radio 三态：
 *  - "none"   → days_of_week=undefined, days_of_month=undefined（每天适用）
 *  - "week"   → days_of_week 多选（0-6），days_of_month=undefined
 *  - "month"  → days_of_month 多选（1-31），days_of_week=undefined
 *
 *  互斥语义：切换维度时清空另一维度字段（PRD 决策：UI radio 单选）。 */
type Dimension = "none" | "week" | "month";

function dimensionOf(w: PeakWindow): Dimension {
  if (w.days_of_month && w.days_of_month.length > 0) return "month";
  if (w.days_of_week && w.days_of_week.length > 0) return "week";
  return "none";
}

function clampInt(v: number, min: number, max: number, fallback: number): number {
  if (Number.isNaN(v)) return fallback;
  return Math.max(min, Math.min(max, Math.trunc(v)));
}

/** WindowsEditModal — 列头点击弹窗编辑时段档 windows。
 *  内部持 localCopy state（windows 副本），编辑不直接改 props；确认 → onSave(localCopy) + onClose。
 *  Dialog 走 Radix Portal（替代 shared/Modal，liquid glass 居中由 Portal 保证）。 */
export function WindowsEditModal({ open, windows, onSave, onClose, t }: {
  open: boolean;
  windows: PeakWindow[];
  onSave: (w: PeakWindow[]) => void;
  onClose: () => void;
  t: TFunction;
}) {
  const [local, setLocal] = useState<PeakWindow[]>(windows);

  useEffect(() => {
    if (open) setLocal(windows.map(w => ({ ...w })));
  }, [open, windows]);

  if (!open) return null;

  const updateWindow = (widx: number, patch: Partial<PeakWindow>) => {
    setLocal(cur => cur.map((w, i) => i === widx ? { ...w, ...patch } : w));
  };

  const removeWindow = (widx: number) => {
    setLocal(cur => cur.filter((_, i) => i !== widx));
  };

  const addWindow = () => {
    setLocal(cur => [...cur, { start_hour: 0, end_hour: 24, multiplier: 1.0 }]);
  };

  /** 切换维度 radio：清空另一维度字段（互斥）。 */
  const switchDimension = (widx: number, dim: Dimension) => {
    setLocal(cur => cur.map((w, i) => {
      if (i !== widx) return w;
      if (dim === "week")   return { ...w, days_of_week:   w.days_of_week   && w.days_of_week.length   > 0 ? w.days_of_week   : undefined, days_of_month: undefined };
      if (dim === "month")  return { ...w, days_of_month:  w.days_of_month  && w.days_of_month.length  > 0 ? w.days_of_month  : undefined, days_of_week: undefined };
      return { ...w, days_of_week: undefined, days_of_month: undefined };
    }));
  };

  const toggleWeekday = (widx: number, day: number) => {
    setLocal(cur => cur.map((w, i) => {
      if (i !== widx) return w;
      const curDays = w.days_of_week ?? [];
      const next = curDays.includes(day)
        ? curDays.filter(d => d !== day)
        : [...curDays, day].sort((a, b) => a - b);
      return { ...w, days_of_week: next.length === 0 ? undefined : next };
    }));
  };

  const toggleDayOfMonth = (widx: number, dom: number) => {
    setLocal(cur => cur.map((w, i) => {
      if (i !== widx) return w;
      const curDays = w.days_of_month ?? [];
      const next = curDays.includes(dom)
        ? curDays.filter(d => d !== dom)
        : [...curDays, dom].sort((a, b) => a - b);
      return { ...w, days_of_month: next.length === 0 ? undefined : next };
    }));
  };

  const handleSave = () => {
    onSave(local.map(w => ({
      ...w,
      // multiplier 不在弹窗内编辑，time_models 窗口默认 1.0；保留传入值兼容旧数据
      multiplier: typeof w.multiplier === "number" && w.multiplier > 0 ? w.multiplier : 1.0,
    })));
    onClose();
  };

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) onClose(); }}>
      <DialogContent style={{ maxWidth: 500, padding: 16 }}>
        <DialogHeader>
          <DialogTitle style={{ margin: "0 0 12px", fontSize: 14 }}>
            {t("platform.windows_edit_title", "编辑时段窗口")}
          </DialogTitle>
        </DialogHeader>

        {local.length === 0 && (
          <div style={{ fontSize: 12, color: "var(--text-tertiary)", fontStyle: "italic", marginBottom: 8 }}>
            {t("platform.windows_edit_empty", "无窗口 → 永不命中")}
          </div>
        )}

        {/* Windows 列表 */}
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          {local.map((w, widx) => {
            const dim = dimensionOf(w);
            return (
              <div
                key={widx}
                style={{
                  padding: 10, borderRadius: "var(--radius-sm)",
                  background: "var(--bg-glass)", border: "1px solid var(--border)",
                  display: "flex", flexDirection: "column", gap: 8,
                }}
              >
                {/* 起 / 止 hour:minute */}
                <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 8 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 11, color: "var(--text-secondary)" }}>
                    <span>{t("platform.start_hour", "起")}</span>
                    <Input className="input" type="number" min={0} max={23} style={{ width: 48, height: "auto" }}
                      value={w.start_hour}
                      onChange={e => updateWindow(widx, { start_hour: clampInt(Number(e.target.value), 0, 23, 0) })}
                    />
                    <span style={{ color: "var(--text-tertiary)" }}>:</span>
                    <Input className="input" type="number" min={0} max={59} style={{ width: 48, height: "auto" }}
                      value={w.start_minute ?? 0}
                      onChange={e => updateWindow(widx, { start_minute: clampInt(Number(e.target.value), 0, 59, 0) })}
                    />
                  </div>
                  <div style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 11, color: "var(--text-secondary)" }}>
                    <span>{t("platform.end_hour", "止")}</span>
                    <Input className="input" type="number" min={0} max={24} style={{ width: 48, height: "auto" }}
                      value={w.end_hour}
                      onChange={e => updateWindow(widx, { end_hour: clampInt(Number(e.target.value), 0, 24, 0) })}
                    />
                    <span style={{ color: "var(--text-tertiary)" }}>:</span>
                    <Input className="input" type="number" min={0} max={59} style={{ width: 48, height: "auto" }}
                      value={w.end_minute ?? 0}
                      onChange={e => updateWindow(widx, { end_minute: clampInt(Number(e.target.value), 0, 59, 0) })}
                    />
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    style={{ padding: "2px 6px", fontSize: 10, marginLeft: "auto", color: "var(--color-danger)" }}
                    onClick={() => removeWindow(widx)}
                    title={t("action.delete", "删除")}
                  >
                    ×
                  </Button>
                </div>

                {/* 维度 radio：无 / 周几 / 每月几日（互斥） */}
                <RadioGroup
                  value={dim}
                  onValueChange={(v) => switchDimension(widx, v as Dimension)}
                  style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 11 }}
                >
                  <span style={{ color: "var(--text-tertiary)", fontWeight: 600 }}>
                    {t("platform.windows_dimension", "维度")}
                  </span>
                  {([
                    { key: "none" as const,  label: t("platform.windows_dim_none", "无") },
                    { key: "week" as const,  label: t("platform.windows_dim_week", "周几") },
                    { key: "month" as const, label: t("platform.windows_dim_month", "每月几日") },
                  ]).map(opt => (
                    <div key={opt.key} style={{ display: "flex", alignItems: "center", gap: 3 }}>
                      <RadioGroupItem value={opt.key} id={`dim-${widx}-${opt.key}`} />
                      <label htmlFor={`dim-${widx}-${opt.key}`} style={{ cursor: "pointer", color: "var(--text-secondary)" }}>
                        {opt.label}
                      </label>
                    </div>
                  ))}
                </RadioGroup>

                {/* 维度对应编辑器：周几 / 每月几日 = ToggleGroup 风格的多选 toggle */}
                {dim === "week" && (
                  <div style={{ display: "flex", alignItems: "center", gap: 2, flexWrap: "wrap" }}>
                    {WEEKDAY_LABELS.map((d, di) => {
                      const active = (w.days_of_week ?? []).includes(di);
                      return (
                        <Button
                          key={di}
                          type="button"
                          variant={active ? "default" : "ghost"}
                          size="sm"
                          style={{ padding: "1px 5px", fontSize: 10, minWidth: 22, height: "auto" }}
                          onClick={() => toggleWeekday(widx, di)}
                          title={["周日", "周一", "周二", "周三", "周四", "周五", "周六"][di]}
                        >
                          {d}
                        </Button>
                      );
                    })}
                  </div>
                )}

                {dim === "month" && (
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 2 }}>
                    {Array.from({ length: 31 }, (_, i) => i + 1).map(dom => {
                      const active = (w.days_of_month ?? []).includes(dom);
                      return (
                        <Button
                          key={dom}
                          type="button"
                          variant={active ? "default" : "ghost"}
                          size="sm"
                          style={{ padding: "1px 5px", fontSize: 10, minWidth: 26, height: "auto" }}
                          onClick={() => toggleDayOfMonth(widx, dom)}
                        >
                          {dom}
                        </Button>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {/* 添加窗口 */}
        <Button
          variant="ghost"
          style={{ padding: "4px 10px", fontSize: 11, marginTop: 10, justifyContent: "flex-start" }}
          onClick={addWindow}
        >
          + {t("platform.windows_edit_add", "添加窗口")}
        </Button>

        {/* 底部操作 */}
        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", marginTop: 14 }}>
          <Button variant="ghost" onClick={onClose}>
            {t("action.cancel", "取消")}
          </Button>
          <Button onClick={handleSave}>
            {t("action.confirm", "确认")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
