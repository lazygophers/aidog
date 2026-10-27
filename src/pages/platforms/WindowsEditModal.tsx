import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { TFunction } from "i18next";
import type { TimeWindow } from "../../domains/platforms/defaults";
import { type TzMode, utcToDisplay, displayToUtc, WINDOW_TIMEZONES } from "../../utils/timeWindow";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { makeRipple } from "../../components/shared";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** 周几按钮标签：locale `platform.weekday_short.<i>`（0=Sunday…6=Saturday，与 TimeWindow.days_of_week 索引对齐）。 */
const WEEKDAY_INDEX = [0, 1, 2, 3, 4, 5, 6] as const;

const pad = (n: number) => String(n).padStart(2, "0");

/** 单窗口维度 radio 三态：
 *  - "none"   → days_of_week=undefined, days_of_month=undefined（每天适用）
 *  - "week"   → days_of_week 多选（0-6），days_of_month=undefined
 *  - "month"  → days_of_month 多选（1-31），days_of_week=undefined
 *
 *  互斥语义：切换维度时清空另一维度字段（PRD 决策：UI radio 单选）。 */
type Dimension = "none" | "week" | "month";

function dimensionOf(w: TimeWindow): Dimension {
  if (w.days_of_month && w.days_of_month.length > 0) return "month";
  if (w.days_of_week && w.days_of_week.length > 0) return "week";
  return "none";
}

function clampInt(v: number, min: number, max: number, fallback: number): number {
  if (Number.isNaN(v)) return fallback;
  return Math.max(min, Math.min(max, Math.trunc(v)));
}

/** 键入文本 → {h,m}。支持「9」「9:5」「937」「1530」（3-4 位时末两位=分钟）与全角冒号/点；
 *  小时钳 0..maxHour、分钟钳 0..59；止侧（maxHour=24）24 点时分钟归 0。非法返 null。 */
function parseTimeInput(raw: string, maxHour: number, fallbackM: number): { h: number; m: number } | null {
  const s = raw.trim().replace(/[：.]/g, ":");
  let mt = s.match(/^(\d{1,2}):(\d{1,2})$/) ?? s.match(/^(\d{1,2})$/);
  if (!mt) {
    const d = s.match(/^(\d{3,4})$/);
    if (d) mt = ["", d[1].slice(0, -2), d[1].slice(-2)];
  }
  if (!mt) return null;
  const h = clampInt(+mt[1], 0, maxHour, 0);
  let m = mt[2] === undefined ? fallbackM : clampInt(+mt[2], 0, 59, fallbackM);
  // 半开区间 24:xx 只有 24:00 合法（与旧 stepper 同语义）
  if (maxHour === 24 && h === 24) m = 0;
  return { h, m };
}

const TP_MINUTES = [0, 15, 30, 45];

/** 单侧时间选择器（C 方向稿 v2）：可直接键入（Enter/失焦提交并钳制，Esc 撤销）+ 点开「时×分」两列弹层。
 *  值变更走 onSet 回调（display 层），UTC 换算由调用方 fromDisp 负责。
 *  maxHour=24 的止侧：小时多一档 24 终点，选中时分钟锁 00。 */
export function TimePicker({ suffix, hour, minute, maxHour, onSet, t }: {
  suffix: string;
  hour: number;
  minute: number;
  maxHour: number;
  onSet: (hour: number, minute: number) => void;
  t: TFunction;
}) {
  const [open, setOpen] = useState(false);
  // null = 跟随 props；非 null = 键入中（不即时格式化，提交时才解析）
  const [text, setText] = useState<string | null>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const end = maxHour === 24;

  // 点外部收起（mousedown：早于 blur/commit，先收弹层再提交键入值）
  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (boxRef.current && !boxRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  const commit = () => {
    if (text === null) return;
    const parsed = parseTimeInput(text, maxHour, minute);
    setText(null);
    if (parsed && (parsed.h !== hour || parsed.m !== minute)) onSet(parsed.h, parsed.m);
  };

  const hours = Array.from({ length: maxHour + 1 }, (_, i) => i);
  const minuteLocked = end && hour === 24;

  return (
    <div
      ref={boxRef}
      className="aidog-stepper-val"
      style={{
        position: "relative",
        display: "inline-flex", alignItems: "center", gap: 6,
        background: "var(--card)", border: "1px solid var(--border)",
        borderRadius: "var(--radius-sm)", padding: "5px 10px",
        fontVariantNumeric: "tabular-nums",
      }}
    >
      <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>{suffix}</span>
      <input
        aria-label={suffix}
        inputMode="numeric"
        value={text ?? `${pad(hour)}:${pad(minute)}`}
        onChange={e => setText(e.target.value)}
        onFocus={() => setOpen(true)}
        onKeyDown={e => {
          if (e.key === "Enter" || e.key === "Escape") {
            if (e.key === "Escape") setText(null); else commit();
            setOpen(false);
            (e.target as HTMLInputElement).blur();
          }
        }}
        onBlur={commit}
        title={t("platform.windows_step_hint", "可直接键入，或点开选择（分钟 15 分钟档）")}
        style={{
          width: 48, background: "transparent", border: 0, outline: "none", padding: 0,
          color: "inherit", font: "inherit", fontSize: 15, fontWeight: 600, textAlign: "center",
        }}
      />
      <span
        style={{ color: "var(--text-tertiary)", fontSize: 9, cursor: "pointer" }}
        onClick={() => setOpen(o => !o)}
      >
        ▾
      </span>
      {open && (
        <div className="aidog-tpop" role="listbox" aria-label={suffix}>
          <div className="aidog-tpop-col">
            {hours.map(h => (
              <button
                key={h}
                type="button"
                role="option"
                aria-selected={h === hour}
                className={h === hour ? "on" : ""}
                data-h={h}
                onClick={() => { if (h !== hour) onSet(h, end && h === 24 ? 0 : minute); }}
              >
                {pad(h)}
              </button>
            ))}
          </div>
          <div className="aidog-tpop-col">
            {TP_MINUTES.map(v => {
              const dis = minuteLocked && v !== 0;
              return (
                <button
                  key={v}
                  type="button"
                  role="option"
                  aria-selected={!dis && v === minute}
                  className={!dis && v === minute ? "on" : ""}
                  data-m={v}
                  disabled={dis}
                  onClick={() => {
                    if (v !== minute) onSet(hour, v);
                    setOpen(false);
                  }}
                >
                  {pad(v)}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

/** WindowsEditModal — 列头点击弹窗编辑时段档 windows（C 方向稿：摘要行 + 时间步进器 + 生效日分段控件）。
 *  内部持 localCopy state（windows 副本），编辑不直接改 props；确认 → onSave(localCopy) + onClose。
 *  Dialog 走 Radix Portal（替代 shared/Modal，liquid glass 居中由 Portal 保证）。 */
export function WindowsEditModal({ open, windows, onSave, onClose, tzMode, setTzMode, t }: {
  open: boolean;
  windows: TimeWindow[];
  onSave: (w: TimeWindow[]) => void;
  onClose: () => void;
  tzMode: TzMode;
  setTzMode: Dispatch<SetStateAction<TzMode>>;
  t: TFunction;
}) {
  const [local, setLocal] = useState<TimeWindow[]>(windows);
  // UI-only 维度选中态（与 local 同索引同步）：数据层「周几-但一天没选」与「每天」同形，
  // 纯派生态 dimensionOf 推不出中间态，故显式维护，禁落盘（design.md 方案）。
  const [uiDim, setUiDim] = useState<Dimension[]>([]);

  useEffect(() => {
    if (open) {
      setLocal(windows.map(w => ({ ...w })));
      setUiDim(windows.map(dimensionOf));
    }
  }, [open, windows]);

  if (!open) return null;

  const updateWindow = (widx: number, patch: Partial<TimeWindow>) => {
    setLocal(cur => cur.map((w, i) => i === widx ? { ...w, ...patch } : w));
  };

  const removeWindow = (widx: number) => {
    setLocal(cur => cur.filter((_, i) => i !== widx));
    setUiDim(cur => cur.filter((_, i) => i !== widx));
  };

  const addWindow = () => {
    // 新窗口默认：本地时区 + 工作日（周一-周五）。timezone 显式落本地 IANA 名
    // （带 timezone 的窗口存值即该时区本地时刻）；Intl 拿不到时回落 UTC 防语义错位。
    const localTz = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
    setLocal(cur => [...cur, {
      start_hour: 0, end_hour: 24, multiplier: 1.0,
      timezone: localTz, days_of_week: [1, 2, 3, 4, 5],
    }]);
    setUiDim(cur => [...cur, "week"]);
  };

  /** 切换维度 radio：清空另一维度字段（互斥），同步更新 UI-only 选中态。 */
  const switchDimension = (widx: number, dim: Dimension) => {
    setLocal(cur => cur.map((w, i) => {
      if (i !== widx) return w;
      if (dim === "week")   return { ...w, days_of_week:   w.days_of_week   && w.days_of_week.length   > 0 ? w.days_of_week   : undefined, days_of_month: undefined };
      if (dim === "month")  return { ...w, days_of_month:  w.days_of_month  && w.days_of_month.length  > 0 ? w.days_of_month  : undefined, days_of_week: undefined };
      return { ...w, days_of_week: undefined, days_of_month: undefined };
    }));
    setUiDim(cur => cur.map((d, i) => i === widx ? dim : d));
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
      // multiplier 不在弹窗内编辑，time_windows 窗口默认 1.0；保留传入值兼容旧数据
      multiplier: typeof w.multiplier === "number" && w.multiplier > 0 ? w.multiplier : 1.0,
    })));
    onClose();
  };

  const DIM_OPTIONS = [
    { key: "none" as const,  label: t("platform.windows_dim_none", "每天") },
    { key: "week" as const,  label: t("platform.windows_dim_week", "周几") },
    { key: "month" as const, label: t("platform.windows_dim_month", "每月几日") },
  ];

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) onClose(); }}>
      <DialogContent className="aidog-anim-pop" style={{ maxWidth: 520, padding: 16 }}>
        <DialogHeader>
          <DialogTitle style={{ margin: "0 0 12px", fontSize: 14, display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
            <span>{t("platform.windows_edit_title", "编辑时段窗口")}</span>
            {/* tz 切换：与 formSections.tsx PeakSection 同款样式，windows 存储恒 UTC+0，仅展示/输入换算 */}
            <div style={{ display: "flex", gap: 4, padding: 2, background: "var(--bg-glass)", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)" }}>
              {(["local", "utc"] as const).map(m => (
                <Button
                  key={m}
                  type="button"
                  variant="ghost"
                  size="sm"
                  style={tzMode === m
                    ? { padding: "2px 8px", fontSize: 11, height: "auto", fontWeight: 400, background: "var(--foreground)", color: "var(--background)", borderColor: "transparent" }
                    : { padding: "2px 8px", fontSize: 11, height: "auto", fontWeight: 400 }}
                  onClick={() => setTzMode(m)}
                >
                  {m === "local" ? t("platform.timezone_local", "本地") : t("platform.timezone_utc", "UTC+0")}
                </Button>
              ))}
            </div>
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
            const dim = uiDim[widx] ?? dimensionOf(w);
            // 起 / 止 hour:minute —— 无 timezone（= UTC 存储）按 tzMode 双向换算；
            // 带 timezone：存值即该时区本地值，直接读写不换算
            const toDisp = (h: number, m: number) =>
              w.timezone ? { hour: h, minute: m } : utcToDisplay(h, m, tzMode);
            const fromDisp = (h: number, m: number) =>
              w.timezone ? { hour: h, minute: m } : displayToUtc(h, m, tzMode);
            const startDisp = toDisp(w.start_hour, w.start_minute ?? 0);
            const endDisp = toDisp(w.end_hour, w.end_minute ?? 0);
            // 跨天判定用原始存储值比较（与 formSections.tsx formatWindowPreview 同口径），不在 display 层比较
            const isNextDay = w.end_hour < w.start_hour;
            // 摘要行的生效日文案：周几/几日用 locale 短标签连接，「每天」直取维度标签
            const scopeText = dim === "week"
              ? (w.days_of_week && w.days_of_week.length > 0
                  ? w.days_of_week.map(d => t(`platform.weekday_short.${d}`)).join("·")
                  : t("platform.windows_dim_week", "周几"))
              : dim === "month"
                ? (w.days_of_month && w.days_of_month.length > 0
                    ? w.days_of_month.join("·")
                    : t("platform.windows_dim_month", "每月几日"))
                : t("platform.windows_dim_none", "每天");
            const tzLabel = w.timezone ?? (tzMode === "local"
              ? t("platform.timezone_local", "本地")
              : t("platform.timezone_utc", "UTC+0"));
            return (
              <div
                key={widx}
                className="aidog-anim-rise"
                style={{
                  padding: 12, borderRadius: "var(--radius-md)",
                  background: "var(--bg-glass)", border: "1px solid var(--border)",
                  display: "flex", flexDirection: "column", gap: 10,
                }}
              >
                {/* 人话摘要行：生效日 + 起止 + 时区（跨天带「次日」徽标） */}
                <div style={{
                  display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap",
                  fontSize: 12, color: "var(--text-secondary)", padding: "5px 10px",
                  background: "var(--accent-subtle)", borderRadius: "var(--radius-sm)",
                }}>
                  <span style={{
                    width: 6, height: 6, borderRadius: "50%", flex: "none",
                    background: "var(--primary)", boxShadow: "0 0 8px var(--primary)",
                  }} />
                  <b style={{ color: "var(--foreground)", fontWeight: 600 }}>{scopeText}</b>
                  <span style={{ fontVariantNumeric: "tabular-nums", color: "var(--foreground)", fontWeight: 600 }}>
                    {pad(startDisp.hour)}:{pad(startDisp.minute)} – {pad(endDisp.hour)}:{pad(endDisp.minute)}
                  </span>
                  {isNextDay && (
                    <span style={{
                      fontSize: 10, padding: "1px 6px", borderRadius: 4,
                      border: "1px solid color-mix(in srgb, var(--color-warning) 40%, transparent)",
                      color: "var(--color-warning)",
                      background: "color-mix(in srgb, var(--color-warning) 8%, transparent)",
                    }}>
                      {t("platform.peak_next_day", "次日")}
                    </span>
                  )}
                  <span>· {tzLabel}</span>
                </div>

                {/* 双时间选择器 + 时区 + 删除 */}
                <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
                  <TimePicker
                    suffix={t("platform.start_hour", "起")} hour={startDisp.hour} minute={startDisp.minute} maxHour={23}
                    t={t}
                    onSet={(h, m) => {
                      const utc = fromDisp(h, m);
                      updateWindow(widx, { start_hour: utc.hour, start_minute: utc.minute });
                    }}
                  />
                  <span style={{ color: "var(--text-tertiary)", fontSize: 12 }}>→</span>
                  <TimePicker
                    suffix={t("platform.end_hour", "止")} hour={endDisp.hour} minute={endDisp.minute} maxHour={24}
                    t={t}
                    onSet={(h, m) => {
                      const utc = fromDisp(h, m);
                      updateWindow(widx, { end_hour: utc.hour, end_minute: utc.minute });
                    }}
                  />
                  {/* 窗口时区：__utc__ 哨兵 = 无 timezone 字段（= UTC，向后兼容）；
                      切换时区只改解释基准，存值数字不动（与 formSections PeakSection 同口径）。 */}
                  <Select
                    value={w.timezone ?? "__utc__"}
                    onValueChange={(v) => updateWindow(widx, { timezone: v === "__utc__" ? undefined : v })}
                  >
                    <SelectTrigger className="input" style={{ width: 150, fontSize: 11, height: 28, marginLeft: "auto" }}>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {WINDOW_TIMEZONES.map((tz) => (
                        <SelectItem key={tz} value={tz}>
                          {tz === "__utc__" ? t("platform.window_timezone_utc_default", "UTC（默认）") : tz}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Button
                    variant="ghost"
                    size="sm"
                    style={{ padding: "2px 6px", fontSize: 10, color: "var(--color-danger)" }}
                    onClick={() => removeWindow(widx)}
                    title={t("action.delete", "删除")}
                  >
                    ×
                  </Button>
                </div>

                {/* 维度分段控件：无 / 周几 / 每月几日（互斥）。
                    容器带 radiogroup 语义（按钮 role=radio 必须有分组父级，ARIA 要求）。 */}
                <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                  <span style={{ fontSize: 11, color: "var(--text-tertiary)", fontWeight: 600 }}>
                    {t("platform.windows_dimension", "生效日")}
                  </span>
                  <div
                    role="radiogroup"
                    aria-label={t("platform.windows_dimension", "生效日")}
                    style={{ display: "flex", gap: 4, padding: 2, background: "var(--card)", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)" }}
                  >
                    {DIM_OPTIONS.map(opt => (
                      <Button
                        key={opt.key}
                        type="button"
                        id={`dim-${widx}-${opt.key}`}
                        role="radio"
                        aria-checked={dim === opt.key}
                        data-state={dim === opt.key ? "on" : "off"}
                        variant="ghost"
                        size="sm"
                        style={dim === opt.key
                          ? { padding: "2px 10px", fontSize: 11, height: "auto", fontWeight: 400, background: "var(--foreground)", color: "var(--background)", borderColor: "transparent" }
                          : { padding: "2px 10px", fontSize: 11, height: "auto", fontWeight: 400 }}
                        onClick={() => switchDimension(widx, opt.key)}
                      >
                        {opt.label}
                      </Button>
                    ))}
                  </div>
                </div>

                {/* 维度对应编辑器：周几 / 每月几日 = 多选 toggle */}
                {dim === "week" && (
                  <div style={{ display: "flex", alignItems: "center", gap: 2, flexWrap: "wrap" }}>
                    {WEEKDAY_INDEX.map(di => {
                      const active = (w.days_of_week ?? []).includes(di);
                      return (
                        <Button
                          key={di}
                          type="button"
                          variant={active ? "default" : "ghost"}
                          size="sm"
                          style={{ padding: "1px 6px", fontSize: 10, minWidth: 24, height: "auto", fontWeight: 600 }}
                          onClick={() => toggleWeekday(widx, di)}
                          title={t(`platform.weekday_short.${di}`)}
                        >
                          {t(`platform.weekday_short.${di}`)}
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

        {/* 添加窗口（稿 1 .add：全宽虚线框，hover 变紫） */}
        <Button
          variant="ghost"
          className="aidog-dashed-add"
          style={{ width: "100%", padding: "4px 10px", fontSize: 11, marginTop: 4, justifyContent: "flex-start" }}
          onClick={addWindow}
        >
          + {t("platform.windows_edit_add", "添加窗口")}
        </Button>

        {/* 底部操作 */}
        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", marginTop: 14 }}>
          <Button variant="ghost" onClick={onClose}>
            {t("action.cancel", "取消")}
          </Button>
          {/* 稿 1 footer .primary：单色反转（前景色底/背景色字），非紫渐变 */}
          <Button
            className="ripple"
            style={{ background: "var(--foreground)", color: "var(--background)", borderColor: "var(--foreground)", fontWeight: 500 }}
            onClick={(e) => { makeRipple(e); handleSave(); }}
          >
            {t("action.confirm", "确认")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
