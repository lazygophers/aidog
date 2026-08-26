import { lazy, Suspense } from "react";
import { useReveal } from "../utils/motion";
import { useSystemSettings } from "./AppSettings/useSystemSettings";
import { ProxyStatusSection, UpstreamProxySection } from "./AppSettings/ProxyStatusSection";
import { StartupSection } from "./AppSettings/StartupSection";
import { LogSettingsSection } from "./AppSettings/LogSettingsSection";
import { SystemMiscSection, DbStatsSection, VersionToastSection } from "./AppSettings/SystemMiscSection";

export type Tab = "system" | "claude" | "codex" | "pi" | "coding_tools" | "middleware" | "scheduling" | "notifications" | "pricing" | "tray" | "popover" | "importexport" | "mitm";

// ponytail: 每个 settings 子 tab 单独 chunk。AppSettings 本身已由 App.tsx 懒加载，
// 这里再拆一层子 tab —— 进 system tab 时不该把 modelInfo/tray/codex 等其余 tab 的代码一并拖下来。
// 外层 Suspense fallback=null：settings 内部 tab 切换同样经 App.tsx handleNavigate 的
// startTransition（Sidebar 二级菜单走同一 handleNavigate），旧 tab 树留屏不闪烁。
const Settings = lazy(() => import("./Settings").then(m => ({ default: m.Settings })));
const CodexSettings = lazy(() => import("./CodexSettings").then(m => ({ default: m.CodexSettings })));
const PiSettings = lazy(() => import("./PiSettings").then(m => ({ default: m.PiSettings })));
const ModelInfoTab = lazy(() => import("./ModelInfo/ModelInfoTab").then(m => ({ default: m.ModelInfoTab })));
const TrayConfigTab = lazy(() => import("./TrayConfigTab").then(m => ({ default: m.TrayConfigTab })));
const PopoverConfigTab = lazy(() => import("./PopoverConfigTab").then(m => ({ default: m.PopoverConfigTab })));
const MiddlewareSettingsTab = lazy(() => import("../components/settings/MiddlewareRules").then(m => ({ default: m.MiddlewareSettingsTab })));
const SchedulingSettingsTab = lazy(() => import("../components/settings/SchedulingSettings").then(m => ({ default: m.SchedulingSettingsTab })));
const NotificationSettingsTab = lazy(() => import("../components/settings/NotificationSettings").then(m => ({ default: m.NotificationSettingsTab })));
const ImportExportTab = lazy(() => import("../components/settings/ImportExport/ImportExportTab").then(m => ({ default: m.ImportExportTab })));
const CodingToolsSettingsTab = lazy(() => import("../components/settings/CodingToolsSettings").then(m => ({ default: m.CodingToolsSettingsTab })));
const MitmConfigTab = lazy(() => import("../components/settings/MitmConfig").then(m => ({ default: m.MitmConfigTab })));

export function AppSettings({ tab, onLogSettingsChanged, onNotifSettingsChanged }: { tab: Tab; onLogSettingsChanged?: (enabled: boolean) => void; onNotifSettingsChanged?: (enabled: boolean) => void }) {
  return (
    <Suspense fallback={null}>
      <AppSettingsTabContent tab={tab} onLogSettingsChanged={onLogSettingsChanged} onNotifSettingsChanged={onNotifSettingsChanged} />
    </Suspense>
  );
}

function AppSettingsTabContent({ tab, onLogSettingsChanged, onNotifSettingsChanged }: { tab: Tab; onLogSettingsChanged?: (enabled: boolean) => void; onNotifSettingsChanged?: (enabled: boolean) => void }) {
  if (tab === "pricing") return <ModelInfoTab />;
  if (tab === "tray") return <TrayConfigTab />;
  if (tab === "popover") return <PopoverConfigTab />;
  if (tab === "middleware") return <MiddlewareSettingsTab />;
  if (tab === "scheduling") return <SchedulingSettingsTab />;
  if (tab === "notifications") return <NotificationSettingsTab onEnabledChanged={onNotifSettingsChanged} />;
  if (tab === "system") return <SystemTab onLogSettingsChanged={onLogSettingsChanged} />;
  if (tab === "codex") return <CodexSettings />;
  if (tab === "pi") return <PiSettings />;
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
      <RevealedSection staggerMs={160}><VersionToastSection s={s} /></RevealedSection>
    </div>
  );
}
