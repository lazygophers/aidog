import { useState } from "react";
import {
  parsePlatformPeak,
  parseDisableDuringPeak,
  parsePlatformTimeWindows,
  serializePlatformPeak,
  serializeDisableDuringPeak,
  serializePlatformTimeWindows,
  type TimeModelRule,
} from "../../services/api";
import type { TimeWindow } from "../../domains/platforms";

export interface PlatformTimeModelsState {
  peak: TimeWindow[];
  setPeak: React.Dispatch<React.SetStateAction<TimeWindow[]>>;
  windowsTz: "local" | "utc";
  setWindowsTz: React.Dispatch<React.SetStateAction<"local" | "utc">>;
  disableDuringPeak: boolean;
  setDisableDuringPeak: React.Dispatch<React.SetStateAction<boolean>>;
  timeModels: TimeModelRule[];
  setTimeModels: React.Dispatch<React.SetStateAction<TimeModelRule[]>>;
  reset: () => void;
  hydrate: (extra: string) => void;
  serialize: (extra: string) => string;
}

/** 高峰与时段模型表单态，统一通过 extra 读写持久化字段。 */
export function usePlatformTimeModels(): PlatformTimeModelsState {
  /** peak：用户覆盖，空数组删除键，空值回落 preset 默认。 */
  const [peak, setPeak] = useState<TimeWindow[]>([]);
  /** 时区只影响表单展示；不写入 platform.extra。 */
  const [windowsTz, setWindowsTz] = useState<"local" | "utc">("local");
  /** disable_during_peak：false 删除键，true 写入用户覆盖。 */
  const [disableDuringPeak, setDisableDuringPeak] = useState(false);
  /** time_windows：空数组删除键，无规则时使用默认模型。 */
  const [timeModels, setTimeModels] = useState<TimeModelRule[]>([]);

  const reset = () => {
    setPeak([]);
    setWindowsTz("local");
    setDisableDuringPeak(false);
    setTimeModels([]);
  };

  const hydrate = (extra: string) => {
    setPeak(parsePlatformPeak(extra));
    setDisableDuringPeak(parseDisableDuringPeak(extra));
    setTimeModels(parsePlatformTimeWindows(extra));
  };

  const serialize = (extra: string) => {
    let next = serializePlatformPeak(extra, peak);
    next = serializeDisableDuringPeak(next, disableDuringPeak);
    return serializePlatformTimeWindows(next, timeModels);
  };

  return {
    peak, setPeak, windowsTz, setWindowsTz,
    disableDuringPeak, setDisableDuringPeak,
    timeModels, setTimeModels,
    reset, hydrate, serialize,
  };
}
