import { useState } from "react";
import type { TimeWindow } from "../../domains/platforms";
import type { PlatformDraft, DraftAction } from "../../domains/platforms/platformDraft";
import type { TimeModelRule, ClientUaRule } from "../../services/api";

export interface PlatformTimeModelsState {
  peak: TimeWindow[];
  setPeak: React.Dispatch<React.SetStateAction<TimeWindow[]>>;
  windowsTz: "local" | "utc";
  setWindowsTz: React.Dispatch<React.SetStateAction<"local" | "utc">>;
  disableDuringPeak: boolean;
  setDisableDuringPeak: React.Dispatch<React.SetStateAction<boolean>>;
  timeModels: TimeModelRule[];
  setTimeModels: React.Dispatch<React.SetStateAction<TimeModelRule[]>>;
  /** direct_models：直通模型列表，空数组删除键（未声明 = 走默认链）。 */
  directModels: string[];
  setDirectModels: React.Dispatch<React.SetStateAction<string[]>>;
  /** allowed_clients：客户端白名单词表码，空数组删除键（= 全放行）。 */
  allowedClients: string[];
  setAllowedClients: React.Dispatch<React.SetStateAction<string[]>>;
  uaRules: ClientUaRule[];
  setUaRules: React.Dispatch<React.SetStateAction<ClientUaRule[]>>;
}

/** 高峰与时段模型表单态（票 12）：6 个持久化键并入 PlatformDraft，由 usePlatformForm 持
 *  useReducer(draftReducer) 持有；本 hook 只提供 draft 键视图 + patch setter 适配 +
 *  windowsTz（纯展示态，不写 extra）。hydrate/serialize/reset 已删——水合在
 *  draftFromPlatform / hydrateExtra，序列化在 draftToPayload（platformDraft.ts）。 */
export function usePlatformTimeModels(
  draft: PlatformDraft,
  dispatch: React.Dispatch<DraftAction>,
): PlatformTimeModelsState {
  const patchOf = <K extends "peak" | "disableDuringPeak" | "timeModels" | "directModels" | "allowedClients" | "uaRules">(k: K) =>
    (v: React.SetStateAction<PlatformDraft[K]>) =>
      dispatch({ type: "patch", patch: { [k]: typeof v === "function" ? (v as (prev: PlatformDraft[K]) => PlatformDraft[K])(draft[k]) : v } });
  const [windowsTz, setWindowsTz] = useState<"local" | "utc">("local");
  return {
    peak: draft.peak, setPeak: patchOf("peak"),
    windowsTz, setWindowsTz,
    disableDuringPeak: draft.disableDuringPeak, setDisableDuringPeak: patchOf("disableDuringPeak"),
    timeModels: draft.timeModels, setTimeModels: patchOf("timeModels"),
    directModels: draft.directModels, setDirectModels: patchOf("directModels"),
    allowedClients: draft.allowedClients, setAllowedClients: patchOf("allowedClients"),
    uaRules: draft.uaRules, setUaRules: patchOf("uaRules"),
  };
}
