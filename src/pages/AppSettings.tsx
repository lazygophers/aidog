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

/**
 * system tab 编排：useSystemSettings 收 state/actions, section 子组件按原视觉顺序渲染。
 * 顺序与拆前 L258-837 完全一致（零 UI 变更）。
 */
function SystemTab({ onLogSettingsChanged }: { onLogSettingsChanged?: (enabled: boolean) => void }) {
  const s = useSystemSettings(onLogSettingsChanged);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <ProxyStatusSection s={s} />
      <StartupSection s={s} />
      <UpstreamProxySection s={s} />
      <SystemMiscSection s={s} />
      <LogSettingsSection s={s} />
      <DbStatsSection s={s} />
      <DefaultsSyncSection />
      <ClientTypesSyncSection />
      <VersionToastSection s={s} />
    </div>
  );
}
