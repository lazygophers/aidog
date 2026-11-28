// 单卡「草稿 / 已存快照 / dirty / 保存」样板（票 57-03）：hook 在容器层调用，
// rail 切换不卸载状态（与拆分前行为一致）；卡片组件只收 props。
// 保存前重新 settingsApi.get，用 write(draft) 只覆盖本卡字段，其余键原样保留
// （读-改-写，防两卡互相覆盖）。
import { useCallback, useState } from "react";
import { settingsApi } from "../../services/api/settings";
import type { McpToolsSettings } from "../../services/api/search";

export type ToolsCardId = "search" | "fetch";

export function useMcpToolsCard<T>(
  read: (v: McpToolsSettings | null) => T,
  write: (draft: T) => Record<string, unknown>,
) {
  const [draft, setDraft] = useState<T>(() => read(null));
  const [saved, setSaved] = useState<T>(() => read(null));
  const [snap, setSnap] = useState(() => JSON.stringify(read(null)));
  const init = useCallback(
    (v: McpToolsSettings | null) => {
      const d = read(v);
      setDraft(d);
      setSaved(d);
      setSnap(JSON.stringify(d));
    },
    [read],
  );
  const dirty = JSON.stringify(draft) !== snap;
  const save = useCallback(async () => {
    const cur = ((await settingsApi.get("mcp_tools", "settings")) ?? {}) as Record<string, unknown>;
    const next = { ...cur, ...write(draft) };
    await settingsApi.set("mcp_tools", "settings", next);
    setSnap(JSON.stringify(draft));
    setSaved(draft);
  }, [draft, write]);
  return { draft, setDraft, saved, dirty, init, save };
}

export type ToolsCard<T> = ReturnType<typeof useMcpToolsCard<T>>;
