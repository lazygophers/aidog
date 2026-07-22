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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

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
        <Select
          value={item.scope ?? "overall"}
          onValueChange={(v) => {
            const scope = v as PopoverTrendScope;
            onUpdate({
              scope,
              scope_ref: scope === "overall" ? null
                : scope === "platform" ? (platforms[0] ? String(platforms[0].id) : null)
                : (groups[0]?.group_key ?? null),
            });
          }}
        >
          <SelectTrigger style={{ fontSize: 12, width: "auto", minWidth: 100, height: 26 }}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="overall">{t("popover.trendScopeOverall", "整体")}</SelectItem>
            <SelectItem value="group">{t("popover.trendScopeGroup", "分组")}</SelectItem>
            <SelectItem value="platform">{t("popover.trendScopePlatform", "平台")}</SelectItem>
          </SelectContent>
        </Select>
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
    <Select
      value={item.scope_ref ?? "__none__"}
      onValueChange={(v) => onUpdate({ scope_ref: v === "__none__" ? null : v })}
    >
      <SelectTrigger style={{ fontSize: 12, width: "auto", minWidth: 120, height: 26 }}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {groups.length === 0 && <SelectItem value="__none__">{t("popover.trendNoGroup", "无分组")}</SelectItem>}
        {groups.map((g) => (
          <SelectItem key={g.group_key} value={g.group_key}>{g.name}</SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function PlatformSelect({ item, platforms, t, onUpdate }: { item: PopoverItem; platforms: Platform[]; t: (k: string, d: string) => string; onUpdate: (p: Partial<PopoverItem>) => void }) {
  return (
    <Select
      value={item.scope_ref ?? "__none__"}
      onValueChange={(v) => onUpdate({ scope_ref: v === "__none__" ? null : v })}
    >
      <SelectTrigger style={{ fontSize: 12, width: "auto", minWidth: 120, height: 26 }}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {platforms.length === 0 && <SelectItem value="__none__">{t("popover.trendNoPlatform", "无平台")}</SelectItem>}
        {platforms.map((p) => (
          <SelectItem key={p.id} value={String(p.id)}>{p.name}</SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function WindowSelect({ item, t, onUpdate, def }: { item: PopoverItem; t: (k: string, d: string) => string; onUpdate: (p: Partial<PopoverItem>) => void; def: PopoverTrendWindow }) {
  return (
    <Select
      value={item.time_window ?? def}
      onValueChange={(v) => onUpdate({ time_window: v as PopoverTrendWindow })}
    >
      <SelectTrigger style={{ fontSize: 12, width: "auto", minWidth: 100, height: 26 }}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {TREND_WINDOWS.map((w) => (
          <SelectItem key={w} value={w}>
            {t(`popover.trendWindow_${w}`, w === "today" ? "今日" : w === "30d" ? "近 30 天" : "近 7 天")}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
