import { useState, useEffect } from "react";
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

function makePlatformItem(platformId: number, order: number): TrayItem {
  return {
    item_type: "platform",
    platform_id: platformId,
    display: "balance",
    metric: null,
    color: defaultColor(),
    font_size: DEFAULT_FONT_SIZE,
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
    enabled: true,
    order,
  };
}

/** hex 简单可读性判断：极暗或极亮在某些菜单栏主题下对比度差 */
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
  const [config, setConfig] = useState<TrayConfig>({
    layout: "single_line",
    separator: "  ",
    items: [],
  });
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  const [addPlatformId, setAddPlatformId] = useState<number | null>(null);

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

  /** 归一 order 字段为数组索引 */
  const withOrders = (items: TrayItem[]): TrayItem[] =>
    items.map((it, idx) => ({ ...it, order: idx }));

  const updateItem = (index: number, patch: Partial<TrayItem>) => {
    const items = config.items.map((it, i) => (i === index ? { ...it, ...patch } : it));
    persist({ ...config, items: withOrders(items) });
  };

  const removeItem = (index: number) => {
    const items = config.items.filter((_, i) => i !== index);
    persist({ ...config, items: withOrders(items) });
  };

  const addPlatform = () => {
    if (addPlatformId === null) return;
    const items = [...config.items, makePlatformItem(addPlatformId, config.items.length)];
    persist({ ...config, items: withOrders(items) });
    setAddPlatformId(null);
  };

  const hasTodayUsage = config.items.some((it) => it.item_type === "today_usage");

  const toggleTodayUsage = () => {
    if (hasTodayUsage) {
      const items = config.items.filter((it) => it.item_type !== "today_usage");
      persist({ ...config, items: withOrders(items) });
    } else {
      const items = [...config.items, makeTodayUsageItem(config.items.length)];
      persist({ ...config, items: withOrders(items) });
    }
  };

  const reorder = (from: number, to: number) => {
    if (from === to) return;
    const items = [...config.items];
    const [moved] = items.splice(from, 1);
    items.splice(to, 0, moved);
    persist({ ...config, items: withOrders(items) });
  };

  const platformName = (id: number | null): string => {
    if (id === null) return "";
    const p = platforms.find((pp) => pp.id === id);
    return p ? p.name : `#${id}`;
  };

  if (loading) {
    return (
      <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>
        {t("common.loading", "加载中...")}
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720 }}>
      {/* Layout & separator */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("tray.layout", "布局")}</div>
        <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap" }}>
          <div style={{ display: "flex", gap: 0, border: "1px solid var(--border)", borderRadius: 8, overflow: "hidden" }}>
            {(["single_line", "two_line"] as const).map((layout) => (
              <button
                key={layout}
                className="btn btn-ghost"
                style={{
                  padding: "6px 14px",
                  fontSize: 12,
                  borderRadius: 0,
                  background: config.layout === layout ? "var(--accent)" : "transparent",
                  color: config.layout === layout ? "#fff" : "var(--text-secondary)",
                }}
                onClick={() => persist({ ...config, layout })}
              >
                {layout === "single_line" ? t("tray.layoutSingle", "单行") : t("tray.layoutTwo", "两行")}
              </button>
            ))}
          </div>
          {config.layout === "single_line" && (
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("tray.separator", "分隔符")}
              </label>
              <input
                className="input"
                type="text"
                value={config.separator}
                onChange={(e) => setConfig({ ...config, separator: e.target.value })}
                onBlur={() => persist(config)}
                style={{ width: 80 }}
              />
            </div>
          )}
        </div>
      </div>

      {/* Add platform / today usage */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("tray.addItem", "添加展示项")}</div>
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          <select
            className="input"
            value={addPlatformId === null ? "" : String(addPlatformId)}
            onChange={(e) => setAddPlatformId(e.target.value ? Number(e.target.value) : null)}
            style={{ width: 200, padding: "4px 8px", fontSize: 12 }}
          >
            <option value="">{t("tray.selectPlatform", "选择平台...")}</option>
            {platforms.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
          <button
            className="btn btn-primary"
            disabled={addPlatformId === null}
            onClick={addPlatform}
            style={{ fontSize: 12 }}
          >
            {t("tray.addPlatform", "添加平台")}
          </button>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
          <div>
            <div style={{ fontSize: 12, fontWeight: 600 }}>{t("tray.todayUsage", "今日消耗 (Tokens)")}</div>
            <div className="text-tertiary" style={{ fontSize: 11, marginTop: 1 }}>
              {t("tray.todayUsageDesc", "展示今日累计 input + output tokens")}
            </div>
          </div>
          <div
            className={`toggle ${hasTodayUsage ? "active" : ""}`}
            onClick={toggleTodayUsage}
            role="switch"
            aria-checked={hasTodayUsage}
            tabIndex={0}
          />
        </div>
      </div>

      {/* Items list (drag to reorder) */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("tray.items", "展示项 (拖拽排序)")}</div>
        {config.items.length === 0 && (
          <div className="text-tertiary" style={{ fontSize: 12 }}>
            {t("tray.noItems", "暂无展示项，托盘将显示图标")}
          </div>
        )}
        {config.items.map((item, i) => {
          const isDragging = dragIndex === i;
          const isDragOver = dragOverIndex === i && dragIndex !== null && dragIndex !== i;
          const isPlatform = item.item_type === "platform";
          const riskyHex = item.color.mode === "custom" && isRiskyHex(item.color.value);
          return (
            <div
              key={`${item.item_type}-${item.platform_id ?? "x"}-${i}`}
              draggable
              onDragStart={(e) => { setDragIndex(i); e.dataTransfer.effectAllowed = "move"; }}
              onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = "move"; if (dragOverIndex !== i) setDragOverIndex(i); }}
              onDragLeave={() => { if (dragOverIndex === i) setDragOverIndex(null); }}
              onDrop={(e) => { e.preventDefault(); if (dragIndex !== null) reorder(dragIndex, i); setDragIndex(null); setDragOverIndex(null); }}
              onDragEnd={() => { setDragIndex(null); setDragOverIndex(null); }}
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 10,
                padding: "12px 14px",
                borderRadius: 10,
                border: isDragOver ? "1px dashed var(--accent)" : "1px solid var(--border)",
                opacity: isDragging ? 0.5 : 1,
                cursor: "grab",
              }}
            >
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 10 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{ color: "var(--text-tertiary)", fontSize: 14 }}>⠿</span>
                  <span style={{ fontSize: 13, fontWeight: 600 }}>
                    {isPlatform ? platformName(item.platform_id) : t("tray.todayUsage", "今日消耗 (Tokens)")}
                  </span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <div
                    className={`toggle ${item.enabled ? "active" : ""}`}
                    onClick={() => updateItem(i, { enabled: !item.enabled })}
                    role="switch"
                    aria-checked={item.enabled}
                    tabIndex={0}
                  />
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 12, color: "var(--danger, #ff453a)" }}
                    onClick={() => removeItem(i)}
                  >
                    {t("common.delete", "删除")}
                  </button>
                </div>
              </div>

              <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap" }}>
                {isPlatform && (
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("tray.display", "展示")}</label>
                    <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 8, overflow: "hidden" }}>
                      {(["balance", "coding"] as const).map((d) => (
                        <button
                          key={d}
                          className="btn btn-ghost"
                          style={{
                            padding: "4px 12px",
                            fontSize: 12,
                            borderRadius: 0,
                            background: item.display === d ? "var(--accent)" : "transparent",
                            color: item.display === d ? "#fff" : "var(--text-secondary)",
                          }}
                          onClick={() => updateItem(i, { display: d })}
                        >
                          {d === "balance" ? t("tray.displayBalance", "余额") : t("tray.displayCoding", "Coding")}
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("tray.fontSize", "字号")}</label>
                  <input
                    className="input"
                    type="number"
                    min={6}
                    max={20}
                    value={item.font_size}
                    onChange={(e) => updateItem(i, { font_size: Math.max(6, Math.min(20, Number(e.target.value))) })}
                    style={{ width: 64 }}
                  />
                </div>

                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("tray.color", "颜色")}</label>
                  <select
                    className="input"
                    value={item.color.mode}
                    onChange={(e) => {
                      const mode = e.target.value as TrayColor["mode"];
                      const value = mode === "preset" ? PRESET_COLORS[0].value : mode === "custom" ? (item.color.value || "#ffffff") : "";
                      updateItem(i, { color: { mode, value } });
                    }}
                    style={{ width: 120, padding: "4px 8px", fontSize: 12 }}
                  >
                    <option value="follow">{t("tray.colorFollow", "跟随系统")}</option>
                    <option value="preset">{t("tray.colorPreset", "预设色")}</option>
                    <option value="custom">{t("tray.colorCustom", "自定义")}</option>
                  </select>

                  {item.color.mode === "preset" && (
                    <select
                      className="input"
                      value={item.color.value}
                      onChange={(e) => updateItem(i, { color: { mode: "preset", value: e.target.value } })}
                      style={{ width: 100, padding: "4px 8px", fontSize: 12 }}
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
                      onChange={(e) => updateItem(i, { color: { mode: "custom", value: e.target.value } })}
                      style={{ width: 36, height: 28, padding: 0, border: "1px solid var(--border)", borderRadius: 6, background: "transparent" }}
                    />
                  )}
                </div>
              </div>

              {riskyHex && (
                <div style={{ fontSize: 11, color: "var(--warning, #ff9f0a)" }}>
                  {t("tray.colorWarning", "该颜色在部分菜单栏主题下可能不清晰")}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {message && <div className="toast">{message}</div>}
    </div>
  );
}
