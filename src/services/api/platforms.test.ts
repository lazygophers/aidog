import { describe, it, expect } from "vitest";
import {
  DEFAULT_DEVIN_CONFIG,
  parseDevinConfig,
  serializeDevinConfig,
  parsePlatformPeak,
  serializePlatformPeak,
  parseDisableDuringPeak,
  serializeDisableDuringPeak,
  parseBuiltinToolCompat,
  serializeBuiltinToolCompat,
  parsePlatformTimeWindows,
  serializePlatformTimeWindows,
} from "./platforms";

// platform.extra 是单个 JSON 字符串，多个特性（devin / peak_hours / time_windows /
// disable_during_peak / breaker / newapi / mock）共享它。每对 parse/serialize 必须：
// 空串、非法 JSON、数组、缺键 → 回默认；serialize 保留兄弟键；空值 → 删键而非写空。

const BAD = ["", "   ", "{not json", "[1,2]", "null", '{"other":1}'];

describe("parseDevinConfig", () => {
  it.each(BAD)("非法/缺键 %s 回默认", (extra) => {
    expect(parseDevinConfig(extra)).toEqual(DEFAULT_DEVIN_CONFIG);
  });

  it("devin 非对象回默认", () => {
    expect(parseDevinConfig('{"devin":5}')).toEqual(DEFAULT_DEVIN_CONFIG);
  });

  it("devin_timeout 数字转字符串，非法字段回空串", () => {
    expect(parseDevinConfig('{"devin":{"org_id":"o1","devin_timeout":300,"devin_mode":"normal"}}')).toEqual({
      org_id: "o1",
      devin_timeout: "300",
      devin_mode: "normal",
    });
    expect(parseDevinConfig('{"devin":{"devin_timeout":"120"}}').devin_timeout).toBe("120");
    expect(parseDevinConfig('{"devin":{"org_id":7,"devin_timeout":[],"devin_mode":null}}')).toEqual(
      DEFAULT_DEVIN_CONFIG,
    );
  });
});

describe("serializeDevinConfig", () => {
  it("三字段全空 → 删 devin 键，保留兄弟键", () => {
    const s = serializeDevinConfig('{"mock":{"a":1},"devin":{"org_id":"old"}}', {
      org_id: "  ",
      devin_timeout: "",
      devin_mode: "",
    });
    const o = JSON.parse(s);
    expect(o.devin).toBeUndefined();
    expect(o.mock).toEqual({ a: 1 });
  });

  it("org_id trim 后写入，timeout 取整且 <=0 不写", () => {
    expect(
      JSON.parse(serializeDevinConfig("", { org_id: " o1 ", devin_timeout: "300.9", devin_mode: " fast " })).devin,
    ).toEqual({ org_id: "o1", devin_timeout: 300, devin_mode: "fast" });
    const noTimeout = JSON.parse(
      serializeDevinConfig("", { org_id: "o1", devin_timeout: "0", devin_mode: "" }),
    ).devin;
    expect(noTimeout).toEqual({ org_id: "o1" });
    expect(
      JSON.parse(serializeDevinConfig("", { org_id: "o1", devin_timeout: "abc", devin_mode: "" })).devin,
    ).toEqual({ org_id: "o1" });
  });

  it("org_id 空但 timeout 有值时仍写入（半填值不丢）", () => {
    expect(
      JSON.parse(serializeDevinConfig("", { org_id: "", devin_timeout: "60", devin_mode: "" })).devin,
    ).toEqual({ org_id: "", devin_timeout: 60 });
  });

  it("非法/数组 extra 重建", () => {
    for (const extra of ["garbage", "[1]"]) {
      expect(JSON.parse(serializeDevinConfig(extra, { org_id: "x", devin_timeout: "", devin_mode: "" })).devin)
        .toEqual({ org_id: "x" });
    }
  });
});

describe("parsePlatformPeak", () => {
  it.each(BAD)("非法/缺键 %s 回空数组", (extra) => {
    expect(parsePlatformPeak(extra)).toEqual([]);
  });

  it("peak_hours 非数组回空", () => {
    expect(parsePlatformPeak('{"peak_hours":{"start_hour":1}}')).toEqual([]);
  });

  it("窗口经 normalizeWindow 归一", () => {
    const out = parsePlatformPeak('{"peak_hours":[{"start_hour":6,"end_hour":10,"multiplier":3}]}');
    expect(out).toHaveLength(1);
    expect(out[0].start_hour).toBe(6);
    expect(out[0].end_hour).toBe(10);
    expect(out[0].multiplier).toBe(3);
  });
});

describe("serializePlatformPeak", () => {
  const win = { start_hour: 6, end_hour: 10, multiplier: 3 };

  it("空数组 → 删键，保留兄弟键", () => {
    const o = JSON.parse(serializePlatformPeak('{"mock":{},"peak_hours":[{}]}', []));
    expect(o.peak_hours).toBeUndefined();
    expect(o.mock).toEqual({});
  });

  it("非空数组写入；非法/数组 extra 重建", () => {
    expect(JSON.parse(serializePlatformPeak("", [win])).peak_hours).toEqual([win]);
    expect(JSON.parse(serializePlatformPeak("bad", [win])).peak_hours).toEqual([win]);
    expect(JSON.parse(serializePlatformPeak("[]", [win])).peak_hours).toEqual([win]);
  });
});

describe("parseDisableDuringPeak", () => {
  it.each(BAD)("非法/缺键 %s 回 false", (extra) => {
    expect(parseDisableDuringPeak(extra)).toBe(false);
  });

  it("严格布尔：只有 true 才为 true", () => {
    expect(parseDisableDuringPeak('{"disable_during_peak":true}')).toBe(true);
    for (const v of ["1", "\"true\"", "\"yes\"", "{}"]) {
      expect(parseDisableDuringPeak(`{"disable_during_peak":${v}}`), v).toBe(false);
    }
  });
});

describe("serializeDisableDuringPeak", () => {
  it("false → 删键（默认行为不入库），保留兄弟键", () => {
    const o = JSON.parse(serializeDisableDuringPeak('{"mock":{},"disable_during_peak":true}', false));
    expect(o.disable_during_peak).toBeUndefined();
    expect(o.mock).toEqual({});
  });

  it("true → 写键；非法/数组 extra 重建", () => {
    expect(JSON.parse(serializeDisableDuringPeak("", true)).disable_during_peak).toBe(true);
    expect(JSON.parse(serializeDisableDuringPeak("bad", true)).disable_during_peak).toBe(true);
    expect(JSON.parse(serializeDisableDuringPeak("[]", true)).disable_during_peak).toBe(true);
  });
});

describe("parseBuiltinToolCompat", () => {
  it.each(BAD)("非法/缺键 %s 回默认（不兼容）", (extra) => {
    expect(parseBuiltinToolCompat(extra)).toEqual({ enabled: false, models: [], stripTools: [] });
  });

  it("enabled 严格布尔；models/strip_tools 非字符串项过滤", () => {
    expect(parseBuiltinToolCompat('{"builtin_tool_compat":{"enabled":true,"models":["glm-4.7"],"strip_tools":["ToolSearch"]}}'))
      .toEqual({ enabled: true, models: ["glm-4.7"], stripTools: ["ToolSearch"] });
    expect(parseBuiltinToolCompat('{"builtin_tool_compat":{"enabled":1,"models":[1,"a"]}}'))
      .toEqual({ enabled: false, models: ["a"], stripTools: [] });
  });
});

describe("serializeBuiltinToolCompat", () => {
  it("enabled=false → 删键，保留兄弟键", () => {
    const o = JSON.parse(serializeBuiltinToolCompat(
      '{"mock":{},"builtin_tool_compat":{"enabled":true}}',
      { enabled: false, models: [], stripTools: [] },
    ));
    expect(o.builtin_tool_compat).toBeUndefined();
    expect(o.mock).toEqual({});
  });

  it("enabled=true 写键；models/stripTools 空数组不入键", () => {
    const o = JSON.parse(serializeBuiltinToolCompat(
      "", { enabled: true, models: ["glm-4.7"], stripTools: [] },
    ));
    expect(o.builtin_tool_compat).toEqual({ enabled: true, models: ["glm-4.7"] });
  });

  it("非法 extra 重建不抛", () => {
    expect(() => serializeBuiltinToolCompat("bad", { enabled: true, models: [], stripTools: [] })).not.toThrow();
  });
});

describe("parsePlatformTimeWindows", () => {
  it.each(BAD)("非法/缺键 %s 回空数组", (extra) => {
    expect(parsePlatformTimeWindows(extra)).toEqual([]);
  });

  it("time_windows 非数组回空", () => {
    expect(parsePlatformTimeWindows('{"time_windows":"x"}')).toEqual([]);
  });

  it("每条规则的 windows 逐个归一", () => {
    const out = parsePlatformTimeWindows(
      '{"time_windows":[{"windows":[{"start_hour":0,"end_hour":6,"multiplier":1}],"models":{"default":"m1"}}]}',
    );
    expect(out).toHaveLength(1);
    expect(out[0].windows[0].end_hour).toBe(6);
    expect(out[0].models).toEqual({ default: "m1" });
  });
});

describe("serializePlatformTimeWindows", () => {
  const rule = { windows: [{ start_hour: 0, end_hour: 6, multiplier: 1 }], models: { default: "m1" } };

  it("空数组 → 删键，保留兄弟键", () => {
    const o = JSON.parse(serializePlatformTimeWindows('{"mock":{},"time_windows":[{}]}', []));
    expect(o.time_windows).toBeUndefined();
    expect(o.mock).toEqual({});
  });

  it("非空写入；非法/数组 extra 重建", () => {
    expect(JSON.parse(serializePlatformTimeWindows("", [rule])).time_windows).toEqual([rule]);
    expect(JSON.parse(serializePlatformTimeWindows("bad", [rule])).time_windows).toEqual([rule]);
    expect(JSON.parse(serializePlatformTimeWindows("[]", [rule])).time_windows).toEqual([rule]);
  });
});

// 同一 extra 串上多特性互不覆盖 —— 编辑表单会依次调多个 serialize，
// 任一实现漏掉「保留其余键」都会静默丢用户配置。
describe("多特性共享 extra 串", () => {
  it("串行 serialize 后所有键并存", () => {
    let extra = "";
    extra = serializeDevinConfig(extra, { org_id: "o1", devin_timeout: "", devin_mode: "" });
    extra = serializePlatformPeak(extra, [{ start_hour: 6, end_hour: 10, multiplier: 2 }]);
    extra = serializeDisableDuringPeak(extra, true);
    extra = serializePlatformTimeWindows(extra, [
      { windows: [{ start_hour: 0, end_hour: 6, multiplier: 1 }], models: { default: "m1" } },
    ]);

    expect(parseDevinConfig(extra).org_id).toBe("o1");
    expect(parsePlatformPeak(extra)).toHaveLength(1);
    expect(parseDisableDuringPeak(extra)).toBe(true);
    expect(parsePlatformTimeWindows(extra)).toHaveLength(1);
  });
});
