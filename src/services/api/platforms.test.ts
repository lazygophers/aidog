import { describe, it, expect } from "vitest";
import {
  DEFAULT_DEVIN_CONFIG,
  parseDevinConfig,
  serializeDevinConfig,
  parseQuotaScriptConfig,
  serializeQuotaScriptConfig,
  hasCustomQuotaScript,
  readRequiresValue,
  parsePlatformPeak,
  serializePlatformPeak,
  parseDisableDuringPeak,
  serializeDisableDuringPeak,
  parseBuiltinToolCompat,
  serializeBuiltinToolCompat,
  parsePlatformTimeWindows,
  serializePlatformTimeWindows,
} from "./platforms";

// platform.extra 是单个 JSON 字符串，多个特性（devin / peak / time_windows /
// disable_during_peak / breaker / quota 脚本 / mock）共享它。每对 parse/serialize 必须：
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
    // org_id 编辑已移交配额脚本 requires 表单（quota-scripts T6），本对不再读写
    expect(parseDevinConfig('{"devin":{"org_id":"o1","devin_timeout":300,"devin_mode":"normal"}}')).toEqual({
      devin_timeout: "300",
      devin_mode: "normal",
    });
    expect(parseDevinConfig('{"devin":{"devin_timeout":"120"}}').devin_timeout).toBe("120");
    expect(parseDevinConfig('{"devin":{"devin_timeout":[],"devin_mode":null}}')).toEqual(
      DEFAULT_DEVIN_CONFIG,
    );
  });
});

describe("serializeDevinConfig", () => {
  it("全空且无存量 org_id → 删 devin 键，保留兄弟键", () => {
    const s = serializeDevinConfig('{"mock":{"a":1},"devin":{"devin_timeout":60}}', {
      devin_timeout: "",
      devin_mode: "",
    });
    const o = JSON.parse(s);
    expect(o.devin).toBeUndefined();
    expect(o.mock).toEqual({ a: 1 });
  });

  it("存量嵌套 org_id 原样保留（本表单不再编辑它）", () => {
    const o = JSON.parse(serializeDevinConfig('{"devin":{"org_id":"o1"}}', {
      devin_timeout: "",
      devin_mode: "",
    }));
    expect(o.devin).toEqual({ org_id: "o1" });
    // 半填值不丢：org_id 保留 + timeout 写入
    expect(JSON.parse(serializeDevinConfig('{"devin":{"org_id":"o1"}}', {
      devin_timeout: "60", devin_mode: "",
    })).devin).toEqual({ org_id: "o1", devin_timeout: 60 });
  });

  it("timeout 取整且 <=0 不写", () => {
    expect(JSON.parse(serializeDevinConfig("", { devin_timeout: "300.9", devin_mode: " fast " })).devin)
      .toEqual({ devin_timeout: 300, devin_mode: "fast" });
    expect(JSON.parse(serializeDevinConfig("", { devin_timeout: "0", devin_mode: "" })).devin)
      .toBeUndefined();
  });

  it("非法/数组 extra 重建", () => {
    for (const extra of ["garbage", "[1]"]) {
      expect(JSON.parse(serializeDevinConfig(extra, { devin_timeout: "60", devin_mode: "" })).devin)
        .toEqual({ devin_timeout: 60 });
    }
  });
});

// ─── 配额查询脚本（quota-scripts T6）─────────────────────────────────────

const VARIANTS = [
  { id: "default", requires: [] as string[] },
  { id: "voapi", requires: ["balance_base_url", "balance_api_key"] },
];
const DEVIN_VARIANTS = [{ id: "default", requires: ["org_id"] }];

describe("parseQuotaScriptConfig / hasCustomQuotaScript", () => {
  it.each(BAD)("非法/缺键 %s 回空", (extra) => {
    expect(parseQuotaScriptConfig(extra)).toEqual({ variantId: "", customScript: "" });
    expect(hasCustomQuotaScript(extra)).toBe(false);
  });

  it("读出 id / 自定义正文；非字符串值忽略", () => {
    expect(parseQuotaScriptConfig('{"quota_script_id":"voapi"}')).toEqual({ variantId: "voapi", customScript: "" });
    expect(parseQuotaScriptConfig('{"quota_custom_script":"return {}"}').customScript).toBe("return {}");
    expect(hasCustomQuotaScript('{"quota_custom_script":"  "}')).toBe(false);
    expect(parseQuotaScriptConfig('{"quota_script_id":5,"quota_custom_script":[]}')).toEqual({ variantId: "", customScript: "" });
  });
});

describe("readRequiresValue", () => {
  it("嵌套优先（newapi/devin）→ 顶层兜底；嵌套存在但空串不回落顶层（t3c 语义）", () => {
    const extra = JSON.stringify({
      newapi: { balance_base_url: "https://n.example" },
      balance_api_key: "top-key",
      devin: { org_id: "" },
      org_id: "top-org",
    });
    expect(readRequiresValue(extra, "balance_base_url")).toBe("https://n.example");
    expect(readRequiresValue(extra, "balance_api_key")).toBe("top-key");
    expect(readRequiresValue(extra, "org_id")).toBe("");           // 嵌套空串不回落
    expect(readRequiresValue(extra, "unknown")).toBe("");
  });

  it.each(BAD)("非法/缺键 %s 回空串", (extra) => {
    expect(readRequiresValue(extra, "org_id")).toBe("");
  });
});

describe("serializeQuotaScriptConfig", () => {
  it("显式 id 命中变体 → 写 quota_script_id；requires 写顶层 + 镜像旧嵌套家", () => {
    const s = serializeQuotaScriptConfig(
      '{"mock":{"a":1}}',
      { variantId: "voapi", customScript: "", requires: { balance_base_url: " https://n.example ", balance_api_key: "k1" } },
      VARIANTS,
      "newapi",
    );
    const o = JSON.parse(s);
    expect(o.quota_script_id).toBe("voapi");
    expect(o.quota_custom_script).toBeUndefined();
    expect(o.balance_base_url).toBe("https://n.example");           // trim 后写顶层
    expect(o.newapi.balance_base_url).toBe("https://n.example");    // 镜像旧嵌套家
    expect(o.newapi.balance_api_key).toBe("k1");
    expect(o.mock).toEqual({ a: 1 });                               // 保留兄弟键
  });

  it("custom 非空 → 写 quota_custom_script 且清 id（互斥，custom 优先）", () => {
    const o = JSON.parse(serializeQuotaScriptConfig(
      '{"quota_script_id":"voapi"}',
      { variantId: "voapi", customScript: "return {}", requires: {} },
      VARIANTS,
      "openrouter",
    ));
    expect(o.quota_custom_script).toBe("return {}");
    expect(o.quota_script_id).toBeUndefined();
  });

  it("id 缺省 / 失效 → 不写 id（后端物化回落首条）；置空 requires 顶层与旧嵌套同删", () => {
    const legacy = '{"newapi":{"balance_api_key":"stale","user_id":"9"}}';
    // 首条变体带 requires：缺省/失效 id 回落首条 → 其 requires 参与（写/清）
    const fallbackFirst = [{ id: "default", requires: ["balance_base_url", "balance_api_key"] }, VARIANTS[1]];
    for (const variantId of ["", "ghost-id"]) {
      const o = JSON.parse(serializeQuotaScriptConfig(
        legacy,
        { variantId, customScript: "", requires: { balance_base_url: "", balance_api_key: "" } },
        fallbackFirst,
        "newapi",
      ));
      expect(o.quota_script_id).toBeUndefined();
      expect(o.balance_base_url).toBeUndefined();
      expect(o.newapi.balance_api_key).toBeUndefined();   // 清掉旧嵌套脏值，防嵌套优先读到陈旧值
      expect(o.newapi.user_id).toBeUndefined();          // newapi 附加键不在 requires map → 同删（表单态始终带该键）
    }
  });

  it("未选中变体的 requires 键不写（custom 无元数据，不动 requires 键）", () => {
    const o = JSON.parse(serializeQuotaScriptConfig(
      "",
      { variantId: "", customScript: "// x", requires: { org_id: "o9" } },
      DEVIN_VARIANTS,
      "devin",
    ));
    expect(o.org_id).toBeUndefined();
    expect(o.devin).toBeUndefined();
  });

  it("devin org_id 顶层 + 嵌套双写（T4 基线 proxy 只读嵌套 extra.devin.org_id）", () => {
    const o = JSON.parse(serializeQuotaScriptConfig(
      "",
      { variantId: "default", customScript: "", requires: { org_id: "org-1" } },
      DEVIN_VARIANTS,
      "devin",
    ));
    expect(o.org_id).toBe("org-1");
    expect(o.devin).toEqual({ org_id: "org-1" });
  });

  it("newapi user_id（非 requires 附加键）写 extra.newapi.user_id，置空删", () => {
    const base = { variantId: "voapi", customScript: "", requires: { balance_base_url: "u", balance_api_key: "k", user_id: "42" } };
    expect(JSON.parse(serializeQuotaScriptConfig("", base, VARIANTS, "newapi")).newapi.user_id).toBe("42");
    const cleared = { ...base, requires: { ...base.requires, user_id: "" } };
    expect(JSON.parse(serializeQuotaScriptConfig('{"newapi":{"user_id":"9"}}', cleared, VARIANTS, "newapi")).newapi.user_id)
      .toBeUndefined();
  });

  it("无变体协议 → quota 键清理（删 stale），requires 不写", () => {
    const o = JSON.parse(serializeQuotaScriptConfig(
      '{"quota_script_id":"old","quota_custom_script":"x"}',
      { variantId: "old", customScript: "", requires: {} },
      [],
      "openai",
    ));
    expect(o.quota_script_id).toBeUndefined();
    expect(o.quota_custom_script).toBeUndefined();
  });

  it("非法/数组 extra 重建不丢写入", () => {
    for (const extra of ["garbage", "[1]"]) {
      const o = JSON.parse(serializeQuotaScriptConfig(
        extra,
        { variantId: "default", customScript: "", requires: { org_id: "o1" } },
        DEVIN_VARIANTS,
        "devin",
      ));
      expect(o.org_id).toBe("o1");
      expect(o.devin).toEqual({ org_id: "o1" });
    }
  });
});

describe("parsePlatformPeak", () => {
  it.each(BAD)("非法/缺键 %s 回空数组", (extra) => {
    expect(parsePlatformPeak(extra)).toEqual([]);
  });

  it("peak 非数组回空", () => {
    expect(parsePlatformPeak('{"peak":{"start_hour":1}}')).toEqual([]);
  });

  it("窗口经 normalizeWindow 归一", () => {
    const out = parsePlatformPeak('{"peak":[{"start_hour":6,"end_hour":10,"multiplier":3}]}');
    expect(out).toHaveLength(1);
    expect(out[0].start_hour).toBe(6);
    expect(out[0].end_hour).toBe(10);
    expect(out[0].multiplier).toBe(3);
  });
});

describe("serializePlatformPeak", () => {
  const win = { start_hour: 6, end_hour: 10, multiplier: 3 };

  it("空数组 → 删键，保留兄弟键", () => {
    const o = JSON.parse(serializePlatformPeak('{"mock":{},"peak":[{}]}', []));
    expect(o.peak).toBeUndefined();
    expect(o.mock).toEqual({});
  });

  it("非空数组写入；非法/数组 extra 重建", () => {
    expect(JSON.parse(serializePlatformPeak("", [win])).peak).toEqual([win]);
    expect(JSON.parse(serializePlatformPeak("bad", [win])).peak).toEqual([win]);
    expect(JSON.parse(serializePlatformPeak("[]", [win])).peak).toEqual([win]);
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
    extra = serializeDevinConfig(extra, { devin_timeout: "60", devin_mode: "" });
    extra = serializePlatformPeak(extra, [{ start_hour: 6, end_hour: 10, multiplier: 2 }]);
    extra = serializeDisableDuringPeak(extra, true);
    extra = serializePlatformTimeWindows(extra, [
      { windows: [{ start_hour: 0, end_hour: 6, multiplier: 1 }], models: { default: "m1" } },
    ]);
    // quota 序列化在 devin 之后（org_id 镜像写 extra.devin，见 serializeQuotaScriptConfig 注释）
    extra = serializeQuotaScriptConfig(
      extra,
      { variantId: "default", customScript: "", requires: { org_id: "o1" } },
      DEVIN_VARIANTS,
      "devin",
    );

    expect(parseDevinConfig(extra).devin_timeout).toBe("60");
    expect(readRequiresValue(extra, "org_id")).toBe("o1");
    expect(parsePlatformPeak(extra)).toHaveLength(1);
    expect(parseDisableDuringPeak(extra)).toBe(true);
    expect(parsePlatformTimeWindows(extra)).toHaveLength(1);
  });
});
