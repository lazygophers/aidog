// ─── scope / 时间窗配置（按 type 分支）──
// 自 PopoverConfigTab.tsx 外迁（arch 阶段6 S5）。
import type {
  PopoverItem,
  PopoverTrendScope,
  PopoverTrendWindow,
  Group,
  Platform,
} from "../../services/api";
import { GROUP_TYPES, TREND_WINDOWS } from "./constants";

type TFn = (k: string, d: string, o?: Record<string, unknown>) => string;

export function ScopeConfig({
  item, t, platforms, groups, onUpdate,
}: {
  item: PopoverItem;
  t: TFn;
  platforms: Platform[];
  groups: Group[];
  onUpdate: (patch: Partial<PopoverItem>) => void;
}) {
  if (item.item_type === "cost_trend") {
    return (
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        <select
          className="input"
          style={{ fontSize: 12, width: "auto", minWidth: 90 }}
          value={item.scope ?? "overall"}
          onChange={(e) => {
            const scope = e.target.value as PopoverTrendScope;
            onUpdate({
              scope,
              scope_ref: scope === "overall" ? null
                : scope === "platform" ? (platforms[0] ? String(platforms[0].id) : null)
                : (groups[0]?.group_key ?? null),
            });
          }}
        >
          <option value="overall">{t("popover.trendScopeOverall", "整体")}</option>
          <option value="group">{t("popover.trendScopeGroup", "分组")}</option>
          <option value="platform">{t("popover.trendScopePlatform", "平台")}</option>
        </select>
        {item.scope === "group" && (
          <GroupSelect item={item} groups={groups} t={t} onUpdate={onUpdate} />
        )}
        {item.scope === "platform" && (
          <PlatformSelect item={item} platforms={platforms} t={t} onUpdate={onUpdate} />
        )}
        <WindowSelect item={item} t={t} onUpdate={onUpdate} def="7d" />
      </div>
    );
  }
  if (item.item_type === "platform_metric") {
    return (
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        <PlatformSelect item={item} platforms={platforms} t={t} onUpdate={onUpdate} />
        <WindowSelect item={item} t={t} onUpdate={onUpdate} def="today" />
      </div>
    );
  }
  if (GROUP_TYPES.has(item.item_type)) {
    return (
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        <GroupSelect item={item} groups={groups} t={t} onUpdate={onUpdate} />
        {item.item_type === "group_cost" && <WindowSelect item={item} t={t} onUpdate={onUpdate} def="7d" />}
      </div>
    );
  }
  return null;
}

function GroupSelect({ item, groups, t, onUpdate }: { item: PopoverItem; groups: Group[]; t: (k: string, d: string) => string; onUpdate: (p: Partial<PopoverItem>) => void }) {
  return (
    <select
      className="input"
      style={{ fontSize: 12, width: "auto", minWidth: 110 }}
      value={item.scope_ref ?? ""}
      onChange={(e) => onUpdate({ scope_ref: e.target.value || null })}
    >
      {groups.length === 0 && <option value="">{t("popover.trendNoGroup", "无分组")}</option>}
      {groups.map((g) => (
        <option key={g.group_key} value={g.group_key}>{g.name}</option>
      ))}
    </select>
  );
}

function PlatformSelect({ item, platforms, t, onUpdate }: { item: PopoverItem; platforms: Platform[]; t: (k: string, d: string) => string; onUpdate: (p: Partial<PopoverItem>) => void }) {
  return (
    <select
      className="input"
      style={{ fontSize: 12, width: "auto", minWidth: 110 }}
      value={item.scope_ref ?? ""}
      onChange={(e) => onUpdate({ scope_ref: e.target.value || null })}
    >
      {platforms.length === 0 && <option value="">{t("popover.trendNoPlatform", "无平台")}</option>}
      {platforms.map((p) => (
        <option key={p.id} value={String(p.id)}>{p.name}</option>
      ))}
    </select>
  );
}

function WindowSelect({ item, t, onUpdate, def }: { item: PopoverItem; t: (k: string, d: string) => string; onUpdate: (p: Partial<PopoverItem>) => void; def: PopoverTrendWindow }) {
  return (
    <select
      className="input"
      style={{ fontSize: 12, width: "auto", minWidth: 90 }}
      value={item.time_window ?? def}
      onChange={(e) => onUpdate({ time_window: e.target.value as PopoverTrendWindow })}
    >
      {TREND_WINDOWS.map((w) => (
        <option key={w} value={w}>
          {t(`popover.trendWindow_${w}`, w === "today" ? "今日" : w === "30d" ? "近 30 天" : "近 7 天")}
        </option>
      ))}
    </select>
  );
}
