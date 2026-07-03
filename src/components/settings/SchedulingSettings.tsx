// ─── Scheduling & Circuit Breaker Global Defaults UI ────────
// 全局调度策略 + 熔断默认设置面板（AppSettings「调度熔断」tab）。
// Platform 的 breaker_* 字段为 0 时继承本面板默认值；Group routing_mode 覆盖 default_routing_mode。
// 消费 GA 冻结的 services/api.ts 契约（schedulingApi / SchedulingBreakerSettings），只读不改。

import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  schedulingApi,
  type SchedulingBreakerSettings,
  type RoutingMode,
} from "../../services/api";
import { ROUTING_MODES, routingModeLabel } from "../../domains/groups/routing";

const DEFAULT_SETTINGS: SchedulingBreakerSettings = {
  default_routing_mode: "health_aware",
  breaker_failure_threshold: 5,
  breaker_open_secs: 60,
  breaker_half_open_max: 2,
  enabled: true,
};

export function SchedulingSettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<SchedulingBreakerSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    (async () => {
      try {
        setSettings(await schedulingApi.getSettings());
      } catch (e) {
        console.error("get scheduling settings failed", e);
        setSettings(DEFAULT_SETTINGS);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const persist = async (next: SchedulingBreakerSettings) => {
    setSettings(next);
    setError("");
    try {
      await schedulingApi.setSettings(next);
    } catch (e) {
      console.error("set scheduling settings failed", e);
      setError(String(e));
    }
  };

  const toggleEnabled = () => persist({ ...settings, enabled: !settings.enabled });

  // 数字字段：钳为 ≥ 0 的整数后持久化（空 → 0）。
  const setNum = (key: "breaker_failure_threshold" | "breaker_open_secs" | "breaker_half_open_max", v: string) =>
    persist({ ...settings, [key]: Math.max(0, Math.floor(Number(v) || 0)) });

  if (loading) {
    return (
      <div className="text-secondary" style={{ padding: 20 }}>
        {t("status.loading", "加载中…")}
      </div>
    );
  }

  const rowStyle = { display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: "10px 12px" } as const;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* 熔断总开关 */}
      <div
        className="glass-surface"
        style={{ padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center" }}
      >
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("scheduling.masterToggle", "熔断总开关")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("scheduling.masterToggleDesc", "关闭后旁路熔断：失败平台不再被临时摘除")}
          </div>
        </div>
        <div
          className={`toggle ${settings.enabled ? "active" : ""}`}
          onClick={toggleEnabled}
          role="switch"
          aria-checked={settings.enabled}
          tabIndex={0}
        />
      </div>

      {/* 默认调度策略 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("scheduling.defaultRoutingMode", "默认调度策略")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("scheduling.defaultRoutingModeDesc", "Group 未单独指定时使用的全局默认策略")}
          </div>
        </div>
        <select
          className="input"
          style={{ maxWidth: 240 }}
          value={settings.default_routing_mode}
          onChange={e => persist({ ...settings, default_routing_mode: e.target.value as RoutingMode })}
        >
          {ROUTING_MODES.map(m => (
            <option key={m} value={m}>{routingModeLabel(t, m)}</option>
          ))}
        </select>
      </div>

      {/* 全局熔断默认阈值 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12, opacity: settings.enabled ? 1 : 0.55 }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("scheduling.breakerDefaults", "全局熔断默认")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("scheduling.breakerDefaultsDesc", "平台未单独覆盖（留空 / 0）时使用的默认熔断阈值")}
          </div>
        </div>
        <div style={rowStyle}>
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerFailureThreshold", "失败阈值")}</span>
          <input
            className="input" type="number" min={0} style={{ width: 140 }}
            value={settings.breaker_failure_threshold}
            onChange={e => setNum("breaker_failure_threshold", e.target.value)}
          />
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerOpenSecs", "熔断时长(秒)")}</span>
          <input
            className="input" type="number" min={0} style={{ width: 140 }}
            value={settings.breaker_open_secs}
            onChange={e => setNum("breaker_open_secs", e.target.value)}
          />
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerHalfOpenMax", "半开探测数")}</span>
          <input
            className="input" type="number" min={0} style={{ width: 140 }}
            value={settings.breaker_half_open_max}
            onChange={e => setNum("breaker_half_open_max", e.target.value)}
          />
        </div>
      </div>

      {error && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>
          {error}
        </div>
      )}
    </div>
  );
}
