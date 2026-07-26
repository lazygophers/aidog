import { Settings } from "./Settings";
import { CodexSettings } from "./CodexSettings";
import { PricingTab } from "./PricingTab";
import { TrayConfigTab } from "./TrayConfigTab";
import { PopoverConfigTab } from "./PopoverConfigTab";
import { MiddlewareSettingsTab } from "../components/settings/MiddlewareRules";
import { SchedulingSettingsTab } from "../components/settings/SchedulingSettings";
import { NotificationSettingsTab } from "../components/settings/NotificationSettings";
import { ImportExportTab } from "../components/settings/ImportExport/ImportExportTab";
import { CodingToolsSettingsTab } from "../components/settings/CodingToolsSettings";
import { MitmConfigTab } from "../components/settings/MitmConfig";
import { useReveal } from "../utils/motion";
import { useSystemSettings } from "./AppSettings/useSystemSettings";
import { ProxyStatusSection, UpstreamProxySection } from "./AppSettings/ProxyStatusSection";
import { StartupSection } from "./AppSettings/StartupSection";
import { LogSettingsSection } from "./AppSettings/LogSettingsSection";
import { SystemMiscSection, DbStatsSection, VersionToastSection, DefaultsSyncSection, ClientTypesSyncSection } from "./AppSettings/SystemMiscSection";

export type Tab = "system" | "claude" | "codex" | "coding_tools" | "middleware" | "scheduling" | "notifications" | "pricing" | "tray" | "popover" | "importexport" | "mitm";

export function AppSettings({ tab, onLogSettingsChanged, onNotifSettingsChanged }: { tab: Tab; onLogSettingsChanged?: (enabled: boolean) => void; onNotifSettingsChanged?: (enabled: boolean) => void }) {
  if (tab === "pricing") return <PricingTab />;
  if (tab === "tray") return <TrayConfigTab />;
  if (tab === "popover") return <PopoverConfigTab />;
  if (tab === "middleware") return <MiddlewareSettingsTab />;
  if (tab === "scheduling") return <SchedulingSettingsTab />;
  if (tab === "notifications") return <NotificationSettingsTab onEnabledChanged={onNotifSettingsChanged} />;
  if (tab === "system") return <SystemTab onLogSettingsChanged={onLogSettingsChanged} />;
  if (tab === "codex") return <CodexSettings />;
  if (tab === "coding_tools") return <CodingToolsSettingsTab />;
  if (tab === "importexport") return <ImportExportTab />;
  if (tab === "mitm") return <MitmConfigTab />;
  return <Settings />;
}

// ponytail: reveal 包装 — 每区块独立 useReveal (stagger 0/80/160)。
// 区块内部数据流 / 视觉顺序零变更, 仅加萤火虫入场动效。
function RevealedSection({ staggerMs, children }: { staggerMs: number; children: React.ReactNode }) {
  const { ref, shown } = useReveal<HTMLDivElement>(staggerMs);
  return (
    <div ref={ref} className={`reveal${shown ? " in" : ""}`}>
      {children}
    </div>
  );
}

/**
 * system tab 编排：useSystemSettings 收 state/actions, section 子组件按原视觉顺序渲染。
 * 顺序与拆前 L258-837 完全一致（零 UI 变更）。
 */
function SystemTab({ onLogSettingsChanged }: { onLogSettingsChanged?: (enabled: boolean) => void }) {
  const s = useSystemSettings(onLogSettingsChanged);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <RevealedSection staggerMs={0}><ProxyStatusSection s={s} /></RevealedSection>
      <RevealedSection staggerMs={80}><StartupSection s={s} /></RevealedSection>
      <RevealedSection staggerMs={80}><UpstreamProxySection s={s} /></RevealedSection>
      <RevealedSection staggerMs={160}><SystemMiscSection s={s} /></RevealedSection>
      <RevealedSection staggerMs={160}><LogSettingsSection s={s} /></RevealedSection>
      <RevealedSection staggerMs={160}><DbStatsSection s={s} /></RevealedSection>
      <RevealedSection staggerMs={160}><DefaultsSyncSection /></RevealedSection>
      <RevealedSection staggerMs={160}><ClientTypesSyncSection /></RevealedSection>
      <RevealedSection staggerMs={160}><VersionToastSection s={s} /></RevealedSection>
    </div>
  );
}
