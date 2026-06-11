import { useState, useEffect, useMemo, useRef, Fragment } from "react";
import { useTranslation } from "react-i18next";
import {
  platformApi,
  trayConfigApi,
  type Platform,
  type TrayConfig,
  type TrayItem,
  type TrayColor,
} from "../services/api";

const PRESET_COLORS: { value: string; cssVar: string }[] = [
  { value: "red", cssVar: "#ff453a" },
  { value: "green", cssVar: "#32d74b" },
  { value: "orange", cssVar: "#ff9f0a" },
];

const DEFAULT_FONT_SIZE = 9;

function defaultColor(): TrayColor {
  return { mode: "follow", value: "" };
}

function makePlatformItem(platformId: number, display: "balance" | "coding", order: number): TrayItem {
  return {
    item_type: "platform",
    platform_id: platformId,
    display,
    metric: null,
    color: defaultColor(),
    font_size: DEFAULT_FONT_SIZE,
    line_mode: "single",
    enabled: true,
    order,
  };
}

function makeTodayUsageItem(order: number): TrayItem {
  return {
    item_type: "today_usage",
    platform_id: null,
    display: "",
    metric: "tokens",
    color: defaultColor(),
    font_size: DEFAULT_FONT_SIZE,
    line_mode: "single",
    enabled: true,
    order,
  };
}

function isRiskyHex(hex: string): boolean {
  const m = /^#?([0-9a-fA-F]{6})$/.exec(hex.trim());
  if (!m) return false;
  const n = parseInt(m[1], 16);
  const r = (n >> 16) & 0xff;
  const g = (n >> 8) & 0xff;
  const b = n & 0xff;
  const luminance = 0.299 * r + 0.587 * g + 0.114 * b;
  return luminance < 40 || luminance > 215;
}

export function TrayConfigTab() {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [config, setConfig] = useState<TrayConfig>({ separator: "  ", items: [] });
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [expandedIdx, setExpandedIdx] = useState<number | null>(null);

  // Drag state (pointer-event pattern, matching Groups/Platforms)
  const [drag, setDrag] = useState<{ from: number; to: number } | null>(null);
  const dragStartRef = useRef<{ y: number; index: number } | null>(null);
  const didDragRef = useRef(false);

  // Add item dropdown
  const [showAddMenu, setShowAddMenu] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const list = await platformApi.list();
        setPlatforms(list.filter((p) => p.enabled));
      } catch (e) {
        console.error(e);
      }
      try {
        const cfg = await trayConfigApi.get();
        setConfig(cfg);
      } catch (e) {
        console.error(e);
      }
      setLoading(false);
    })();
  }, []);

  const persist = async (next: TrayConfig) => {
    setConfig(next);
    try {
      await trayConfigApi.set(next);
    } catch (e) {
      console.error(e);
      setMessage(String(e));
    }
  };

  const withOrders = (items: TrayItem[]): TrayItem[] =>
    items.map((it, idx) => ({ ...it, order: idx }));

  const updateItem = (index: number, patch: Partial<TrayItem>) => {
    const items = config.items.map((it, i) => (i === index ? { ...it, ...patch } : it));
    persist({ ...config, items: withOrders(items) });
  };

  const removeItem = (index: number) => {
    const items = config.items.filter((_, i) => i !== index);
    if (expandedIdx !== null && expandedIdx >= items.length) setExpandedIdx(null);
    persist({ ...config, items: withOrders(items) });
  };

  const addPlatform = (pid: number) => {
    const display = "balance" as const;
    const items = [...config.items, makePlatformItem(pid, display, config.items.length)];
    persist({ ...config, items: withOrders(items) });
    setShowAddMenu(false);
  };

  const addTodayUsage = () => {
    const items = [...config.items, makeTodayUsageItem(config.items.length)];
    persist({ ...config, items: withOrders(items) });
    setShowAddMenu(false);
  };

  // ── Preview computation ──
  const preview = useMemo(() => {
    const enabled = config.items
      .filter((i) => i.enabled)
      .sort((a, b) => a.order - b.order);

    let totalLines = 0;
    const segs = enabled.map((item) => {
      const isTwo = item.line_mode === "two";
      const lines = isTwo ? 2 : 1;
      totalLines += lines;
      let label = "";
      let value = "";
      if (item.item_type === "platform" && item.platform_id) {
        const p = platforms.find((pp) => pp.id === item.platform_id);
        label = p?.name ?? `#${item.platform_id}`;
        value = item.display === "coding" ? "剩 --%" : "--.--";
      } else if (item.item_type === "today_usage") {
        label = "今日";
        value = "-- tok";
      }
      const text = isTwo ? `${label}\n${value}` : `${label} ${value}`;
      return { text, lines, isTwo };
    });

    return { segments: segs, totalLines, overBudget: totalLines > 2 };
  }, [config, platforms]);

  // ── Drag handlers (pointer-event pattern) ──
  const handlePointerDown = (e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragStartRef.current = { y: e.clientY, index };
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    const start = dragStartRef.current;
    if (!start) return;
    if (!drag) {
      if (Math.abs(e.clientY - start.y) < 5) return;
      setDrag({ from: start.index, to: start.index });
      didDragRef.current = true;
    }
    // Determine insertion point from pointer position
    const el = document.querySelectorAll("[data-tray-item]");
    let closest = drag?.from ?? start.index;
    for (let i = 0; i < el.length; i++) {
      const rect = el[i].getBoundingClientRect();
      const mid = rect.top + rect.height / 2;
      if (e.clientY > mid) closest = i;
      else break;
    }
    setDrag((prev) => (prev ? { ...prev, to: closest } : null));
  };

  const handlePointerUp = () => {
    if (drag) {
      const effectiveTo = drag.from < drag.to ? drag.to - 1 : drag.to;
      if (drag.from !== effectiveTo) {
        const items = [...config.items];
        const [moved] = items.splice(drag.from, 1);
        items.splice(effectiveTo, 0, moved);
        persist({ ...config, items: withOrders(items) });
        setExpandedIdx(drag.from < drag.to ? effectiveTo : effectiveTo);
      }
      setDrag(null);
    }
    dragStartRef.current = null;
    setTimeout(() => { didDragRef.current = false; }, 50);
  };

  const platformName = (id: number | null): string => {
    if (id === null) return "";
    const p = platforms.find((pp) => pp.id === id);
    return p ? p.name : `#${id}`;
  };

  // Items already in config (to filter from add dropdown)
  const usedPlatformIds = new Set(
    config.items.filter((i) => i.item_type === "platform").map((i) => i.platform_id)
  );
  const hasTodayUsage = config.items.some((i) => i.item_type === "today_usage");
  const availablePlatforms = platforms.filter((p) => !usedPlatformIds.has(p.id));

  if (loading) {
    return (
      <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>
        {t("common.loading", "加载中...")}
      </div>
    );
  }

  const gripSvg = (
    <svg width="14" height="20" viewBox="0 0 14 20" fill="currentColor">
      <circle cx="4" cy="3" r="1.8" />
      <circle cx="4" cy="10" r="1.8" />
      <circle cx="4" cy="17" r="1.8" />
      <circle cx="10" cy="3" r="1.8" />
      <circle cx="10" cy="10" r="1.8" />
      <circle cx="10" cy="17" r="1.8" />
    </svg>
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720 }}>
      {/* ── Preview Bar ── */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>
          {t("tray.preview", "实时预览")}
        </div>
        {/* Simulated macOS menu bar */}
        <div
          style={{
            background: "rgba(30, 30, 30, 0.95)",
            borderRadius: 8,
            padding: "6px 14px",
            minHeight: 32,
            display: "flex",
            alignItems: "center",
            fontFamily: '-apple-system, "SF Pro Text", system-ui, sans-serif',
            fontSize: 12,
            color: "rgba(255, 255, 255, 0.85)",
            gap: 4,
            flexWrap: "wrap",
            lineHeight: 1.3,
            position: "relative",
            overflow: "hidden",
          }}
        >
          {preview.segments.length === 0 ? (
            <span style={{ color: "rgba(255, 255, 255, 0.35)", fontStyle: "italic" }}>
              {t("tray.previewEmpty", "暂无展示项，托盘将显示图标")}
            </span>
          ) : (
            preview.segments.map((seg, i) => (
              <Fragment key={i}>
                {i > 0 && (
                  <span style={{ color: "rgba(255, 255, 255, 0.3)" }}>
                    {config.separator}
                  </span>
                )}
                <span style={{ whiteSpace: "pre-line" }}>{seg.text}</span>
              </Fragment>
            ))
          )}
        </div>

        {/* Status row */}
        <div style={{ display: "flex", alignItems: "center", gap: 16, flexWrap: "wrap" }}>
          {/* Line budget */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              fontSize: 11,
              color: preview.overBudget
                ? "#ff9f0a"
                : preview.totalLines === 2
                  ? "var(--accent)"
                  : "var(--text-secondary)",
            }}
          >
            <span style={{ fontWeight: 600 }}>
              {t("tray.lineBudget", "行数")} {preview.totalLines}/2
            </span>
            {preview.overBudget && (
              <span style={{ color: "#ff9f0a" }}>
                {t("tray.overBudgetHint", "超限，部分两行项将降为单行")}
              </span>
            )}
          </div>
          {/* Separator */}
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("tray.separator", "分隔符")}
            </label>
            <input
              className="input"
              type="text"
              value={config.separator}
              onChange={(e) => setConfig({ ...config, separator: e.target.value })}
              onBlur={() => persist(config)}
              style={{ width: 60, fontSize: 12, padding: "3px 8px" }}
            />
          </div>
        </div>
      </div>

      {/* ── Items List ── */}
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {config.items.length === 0 && (
          <div
            className="glass-surface"
            style={{
              padding: "24px 20px",
              textAlign: "center",
              color: "var(--text-tertiary)",
              fontSize: 13,
            }}
          >
            {t("tray.noItems", "暂无展示项，点击下方按钮添加")}
          </div>
        )}

        {config.items.map((item, i) => {
          const isExpanded = expandedIdx === i;
          const isDragging = drag?.from === i;
          const isDragTarget = drag?.to === i && drag?.from !== i;
          const isPlatform = item.item_type === "platform";
          const riskyHex = item.color.mode === "custom" && isRiskyHex(item.color.value);

          // Summary text for collapsed state
          const summary = isPlatform
            ? item.display === "coding"
              ? t("tray.displayCoding", "Coding")
              : t("tray.displayBalance", "余额")
            : t("tray.todayUsage", "Tokens");

          return (
            <Fragment key={`${item.item_type}-${item.platform_id ?? "x"}-${i}`}>
              {/* Insertion line above */}
              {drag && isDragTarget && drag.from !== i && (
                <div className="insertion-line" />
              )}

              <div
                data-tray-item
                className={`card-item${isDragging ? " is-dragging" : ""}`}
                style={{
                  position: "relative",
                  display: "flex",
                  flexDirection: "column",
                  gap: 0,
                  opacity: isDragging ? undefined : item.enabled ? 1 : 0.5,
                  paddingLeft: 40, // space for grip handle
                  transition: "all 200ms ease",
                }}
              >
                {/* Grip handle */}
                <div
                  className={`drag-handle${drag?.from === i ? " is-active" : ""}`}
                  onPointerDown={(e) => handlePointerDown(e, i)}
                  onPointerMove={handlePointerMove}
                  onPointerUp={handlePointerUp}
                >
                  {gripSvg}
                </div>

                {/* Header row — always visible */}
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    cursor: "pointer",
                    userSelect: "none",
                  }}
                  onClick={() => {
                    if (!didDragRef.current) {
                      setExpandedIdx(isExpanded ? null : i);
                    }
                  }}
                >
                  {/* Item name */}
                  <span style={{ fontSize: 13, fontWeight: 600, flex: 1 }}>
                    {isPlatform ? platformName(item.platform_id) : t("tray.todayUsage", "今日消耗")}
                  </span>

                  {/* Summary badge */}
                  <span
                    className="badge badge-muted"
                    style={{ fontSize: 10 }}
                  >
                    {summary}
                  </span>

                  {/* Line mode hint */}
                  {item.line_mode === "two" && (
                    <span
                      className="badge badge-accent"
                      style={{ fontSize: 10 }}
                    >
                      {t("tray.lineModeTwo", "两行")}
                    </span>
                  )}

                  {/* Expand chevron */}
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 14 14"
                    fill="none"
                    stroke="var(--text-tertiary)"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    style={{
                      transition: "transform 200ms ease",
                      transform: isExpanded ? "rotate(180deg)" : "rotate(0deg)",
                      flexShrink: 0,
                    }}
                  >
                    <path d="M3.5 5.25L7 8.75L10.5 5.25" />
                  </svg>

                  {/* Enabled toggle */}
                  <div
                    className={`toggle ${item.enabled ? "active" : ""}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      updateItem(i, { enabled: !item.enabled });
                    }}
                    role="switch"
                    aria-checked={item.enabled}
                    tabIndex={0}
                    style={{ width: 32, height: 18, flexShrink: 0 }}
                  />

                  {/* Delete */}
                  <button
                    className="btn btn-ghost btn-icon"
                    style={{
                      fontSize: 12,
                      color: "var(--danger, #ff453a)",
                      width: 24,
                      height: 24,
                      padding: 0,
                      flexShrink: 0,
                    }}
                    onClick={(e) => {
                      e.stopPropagation();
                      removeItem(i);
                    }}
                  >
                    ×
                  </button>
                </div>

                {/* Expanded config */}
                {isExpanded && (
                  <div
                    style={{
                      marginTop: 10,
                      paddingTop: 10,
                      borderTop: "1px solid var(--border)",
                      display: "flex",
                      gap: 16,
                      alignItems: "flex-start",
                      flexWrap: "wrap",
                    }}
                    onClick={(e) => e.stopPropagation()}
                  >
                    {/* Display mode (platform only) */}
                    {isPlatform && (
                      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                          {t("tray.display", "展示")}
                        </label>
                        <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                          {(["balance", "coding"] as const).map((d) => (
                            <button
                              key={d}
                              className="btn btn-ghost"
                              style={{
                                padding: "3px 10px",
                                fontSize: 11,
                                borderRadius: 0,
                                background: item.display === d ? "var(--accent)" : "transparent",
                                color: item.display === d ? "#fff" : "var(--text-secondary)",
                              }}
                              onClick={() => updateItem(i, { display: d })}
                            >
                              {d === "balance"
                                ? t("tray.displayBalance", "余额")
                                : t("tray.displayCoding", "Coding")}
                            </button>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Line mode */}
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                        {t("tray.lineMode", "行模式")}
                      </label>
                      <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                        {(["single", "two"] as const).map((lm) => (
                          <button
                            key={lm}
                            className="btn btn-ghost"
                            style={{
                              padding: "3px 10px",
                              fontSize: 11,
                              borderRadius: 0,
                              background: item.line_mode === lm ? "var(--accent)" : "transparent",
                              color: item.line_mode === lm ? "#fff" : "var(--text-secondary)",
                            }}
                            onClick={() => updateItem(i, { line_mode: lm })}
                          >
                            {lm === "single"
                              ? t("tray.lineModeSingle", "单行")
                              : t("tray.lineModeTwo", "两行")}
                          </button>
                          ))}
                      </div>
                    </div>

                    {/* Font size */}
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                        {t("tray.fontSize", "字号")}
                      </label>
                      <input
                        className="input"
                        type="number"
                        min={6}
                        max={20}
                        value={item.font_size}
                        onChange={(e) =>
                          updateItem(i, { font_size: Math.max(6, Math.min(20, Number(e.target.value))) })
                        }
                        style={{ width: 52, fontSize: 12, padding: "3px 8px" }}
                      />
                    </div>

                    {/* Color */}
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                        {t("tray.color", "颜色")}
                      </label>
                      <select
                        className="input"
                        value={item.color.mode}
                        onChange={(e) => {
                          const mode = e.target.value as TrayColor["mode"];
                          const value =
                            mode === "preset"
                              ? PRESET_COLORS[0].value
                              : mode === "custom"
                                ? (item.color.value || "#ffffff")
                                : "";
                          updateItem(i, { color: { mode, value } });
                        }}
                        style={{ width: 100, padding: "3px 8px", fontSize: 11 }}
                      >
                        <option value="follow">{t("tray.colorFollow", "跟随系统")}</option>
                        <option value="preset">{t("tray.colorPreset", "预设色")}</option>
                        <option value="custom">{t("tray.colorCustom", "自定义")}</option>
                      </select>

                      {item.color.mode === "preset" && (
                        <select
                          className="input"
                          value={item.color.value}
                          onChange={(e) =>
                            updateItem(i, { color: { mode: "preset", value: e.target.value } })
                          }
                          style={{ width: 80, padding: "3px 8px", fontSize: 11 }}
                        >
                          {PRESET_COLORS.map((c) => (
                            <option key={c.value} value={c.value}>{c.value}</option>
                          ))}
                        </select>
                      )}

                      {item.color.mode === "custom" && (
                        <input
                          type="color"
                          value={/^#[0-9a-fA-F]{6}$/.test(item.color.value) ? item.color.value : "#ffffff"}
                          onChange={(e) =>
                            updateItem(i, { color: { mode: "custom", value: e.target.value } })
                          }
                          style={{
                            width: 28,
                            height: 22,
                            padding: 0,
                            border: "1px solid var(--border)",
                            borderRadius: 4,
                            background: "transparent",
                          }}
                        />
                      )}
                    </div>
                  </div>
                )}

                {/* Risky color warning */}
                {isExpanded && riskyHex && (
                  <div style={{ fontSize: 11, color: "var(--warning, #ff9f0a)", marginTop: 6 }}>
                    {t("tray.colorWarning", "该颜色在部分菜单栏主题下可能不清晰")}
                  </div>
                )}
              </div>
            </Fragment>
          );
        })}
      </div>

      {/* ── Add Item ── */}
      <div style={{ position: "relative" }}>
        <button
          className="btn btn-primary"
          onClick={() => setShowAddMenu(!showAddMenu)}
          style={{ fontSize: 12, gap: 6 }}
        >
          <span style={{ fontSize: 16, lineHeight: 1 }}>+</span>
          {t("tray.addItem", "添加展示项")}
        </button>

        {showAddMenu && (
          <>
            {/* Backdrop to close menu */}
            <div
              style={{ position: "fixed", inset: 0, zIndex: 998 }}
              onClick={() => { setShowAddMenu(false); }}
            />
            <div
              className="glass-elevated"
              style={{
                position: "absolute",
                top: "100%",
                left: 0,
                marginTop: 6,
                minWidth: 240,
                padding: 8,
                zIndex: 999,
                display: "flex",
                flexDirection: "column",
                gap: 2,
              }}
            >
              {/* Platform options */}
              {availablePlatforms.map((p) => (
                <button
                  key={p.id}
                  className="btn btn-ghost"
                  style={{ justifyContent: "flex-start", fontSize: 12, padding: "8px 12px" }}
                  onClick={() => addPlatform(p.id)}
                >
                  {p.name}
                </button>
              ))}

              {/* Today usage */}
              {!hasTodayUsage && (
                <button
                  className="btn btn-ghost"
                  style={{
                    justifyContent: "flex-start",
                    fontSize: 12,
                    padding: "8px 12px",
                    borderTop: availablePlatforms.length > 0 ? "1px solid var(--border)" : undefined,
                    marginTop: availablePlatforms.length > 0 ? 4 : undefined,
                  }}
                  onClick={addTodayUsage}
                >
                  {t("tray.todayUsage", "今日消耗 (Tokens)")}
                </button>
              )}

              {/* Nothing available */}
              {availablePlatforms.length === 0 && hasTodayUsage && (
                <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "8px 12px" }}>
                  {t("tray.allAdded", "所有可用的展示项已添加")}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {message && <div className="toast">{message}</div>}
    </div>
  );
}
