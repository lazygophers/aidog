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

/** Peak and time-window form state. Extra remains the persistence boundary. */
export function usePlatformTimeModels(): PlatformTimeModelsState {
  const [peak, setPeak] = useState<TimeWindow[]>([]);
  const [windowsTz, setWindowsTz] = useState<"local" | "utc">("local");
  const [disableDuringPeak, setDisableDuringPeak] = useState(false);
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
