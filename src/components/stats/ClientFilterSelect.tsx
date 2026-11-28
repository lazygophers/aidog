// 统计页客户端多选筛选（issue #66）：Popover + checkbox 列表，多选、默认全部（空集 = 全部）。
// 视觉与 FilterDropdown 同族（Popover + trigger 玻璃底）；选项显示名走 clientLabel 词表映射。
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import { clientLabel } from "../../utils/clientLabel";

export interface ClientFilterSelectProps {
  /** 窗口内实际出现的客户端码（后端 available_clients） */
  clients: string[];
  /** 已选客户端码；空数组 = 全部 */
  selected: string[];
  onChange: (next: string[]) => void;
  testId?: string;
}

// 纯逻辑抽出可测：勾选/取消一个客户端码。取消后若集合变空 = 回到「全部」。
export function toggleClient(selected: string[], code: string): string[] {
  return selected.includes(code) ? selected.filter((c) => c !== code) : [...selected, code];
}

export function ClientFilterSelect({ clients, selected, onChange, testId }: ClientFilterSelectProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const label = (code: string) => clientLabel((k, fb) => t(k, fb), code);
  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return clients;
    return clients.filter((c) => label(c).toLowerCase().includes(q) || c.toLowerCase().includes(q));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clients, search, t]);
  const trigger = selected.length === 0
    ? t("stats.allClients", "All Clients")
    : selected.map(label).join(", ");
  return (
    <Popover open={open} onOpenChange={(o) => { setOpen(o); if (!o) setSearch(""); }}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          data-testid={testId ?? "stats-client-filter"}
          style={{ fontSize: 14, height: 30, maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
        >
          {trigger}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" style={{ padding: 8, minWidth: 180 }}>
        <Input
          data-testid="stats-client-search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t("stats.searchClient", "Search client")}
          style={{ fontSize: 14, marginBottom: 6 }}
        />
        {filtered.length === 0 ? (
          <div style={{ fontSize: 14, color: "var(--text-secondary)", padding: "8px 4px" }}>
            {t("stats.noMatch", "No match")}
          </div>
        ) : (
          filtered.map((c) => {
            const checked = selected.includes(c);
            return (
              <label
                key={c}
                data-testid={`stats-client-opt-${c}`}
                style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14, padding: "5px 6px", borderRadius: 6, cursor: "pointer", background: checked ? "var(--accent-subtle)" : "transparent" }}
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => onChange(toggleClient(selected, c))}
                  style={{ accentColor: "var(--accent)" }}
                />
                {label(c)}
              </label>
            );
          })
        )}
      </PopoverContent>
    </Popover>
  );
}
