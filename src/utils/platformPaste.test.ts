// @vitest-environment node
import { describe, it, expect } from "vitest";
import {
  normalizeForMatch,
  guessProtocol,
  matchPlatform,
  parsePlatformPaste,
  extractExpiryAt,
  type PastePresetRef,
} from "./platformPaste";
// 真值源（platform-presets.json，68 协议）——数据驱动回归矩阵用。
import presetsJson from "../../src-tauri/defaults/platform-presets.json";

const PRESETS: PastePresetRef[] = [
  // anthropic + openai 真值 fixture（对齐 platform-presets.json:18/34）——
  // 历版 fixture 缺这俩致 anthropic/openai 抢匹配回归从未被覆盖（platform-detect-minimax 根因之一）。
  {
    value: "anthropic",
    label: "Anthropic",
    keywords: ["claude", "克劳德", "官方"],
    hosts: ["api.anthropic.com"],
  },
  {
    value: "openai",
    label: "OpenAI",
    keywords: ["gpt", "chatgpt", "官方"],
    hosts: ["api.openai.com/v1"],
  },
  { value: "deepseek", label: "DeepSeek", keywords: ["deepseek"], hosts: ["api.deepseek.com"] },
  {
    value: "glm",
    label: "GLM",
    keywords: ["glm", "智谱"],
    hosts: ["open.bigmodel.cn/api/paas/v4"],
  },
  {
    value: "glm_coding",
    label: "GLM Coding",
    keywords: ["glm coding"],
    hosts: ["open.bigmodel.cn/api/coding"],
    codingPlan: true,
  },
  {
    value: "xiaomi_mimo",
    label: "Xiaomi MiMo",
    keywords: ["xiaomi", "mimo"],
    hosts: ["api.xiaomimimo.com"],
  },
  // 真实 buildProtocolsFromPresets 输出中普通/coding 变体共用同 value（xiaomi_mimo），靠 hosts/codingPlan/
  // codingKeyPrefixes 区分。机制 B 升级依赖同 value 匹配，fixture 须对齐此结构。
  {
    value: "xiaomi_mimo",
    label: "Xiaomi MiMo Coding",
    keywords: [],
    hosts: ["token-plan-cn.xiaomimimo.com"],
    codingPlan: true,
    codingKeyPrefixes: ["tp-"],
  },
  {
    value: "doubao",
    label: "火山引擎",
    keywords: ["火山", "doubao", "volces", "agentplan"],
    // 单平台多端点派生：coding plan（/api/coding + /api/coding/v3）+
    // agent plan（/api/plan + /api/plan/v3）。hosts Set 去重后四条。
    hosts: [
      "ark.cn-beijing.volces.com/api/coding",
      "ark.cn-beijing.volces.com/api/coding/v3",
      "ark.cn-beijing.volces.com/api/plan",
      "ark.cn-beijing.volces.com/api/plan/v3",
    ],
  },
  { value: "mock", label: "Mock", keywords: ["测试", "mock"] },
];

describe("normalizeForMatch", () => {
  it("lowercases, replaces non-alnum/CJK with space, folds whitespace", () => {
    expect(normalizeForMatch("Hello,  WORLD!")).toBe("hello world");
    expect(normalizeForMatch("  GLM-4.5  ")).toBe("glm 4 5");
  });
  it("keeps CJK chars", () => {
    expect(normalizeForMatch("智谱AI")).toBe("智谱ai");
  });
});

describe("guessProtocol", () => {
  it("detects anthropic (with truncation tolerance)", () => {
    expect(guessProtocol("https://api.anthropic.com")).toBe("anthropic");
    expect(guessProtocol("https://x.com/anthropi")).toBe("anthropic");
  });
  it("detects gemini", () => {
    expect(guessProtocol("https://generativelanguage.googleapis.com")).toBe("gemini");
    expect(guessProtocol("https://x/gemini")).toBe("gemini");
  });
  it("detects openai (incl. /v1 path)", () => {
    expect(guessProtocol("https://api.openai.com")).toBe("openai");
    expect(guessProtocol("https://host/v1")).toBe("openai");
  });
  it("returns unknown for unrecognized", () => {
    expect(guessProtocol("https://example.com/foo")).toBe("unknown");
  });
});

describe("matchPlatform", () => {
  it("matches by base_url host (longest/most specific wins)", () => {
    const hit = matchPlatform("", PRESETS, [
      { url: "https://token-plan-cn.xiaomimimo.com/v1", protocol: "openai" },
    ]);
    expect(hit?.value).toBe("xiaomi_mimo");
    expect(hit?.codingPlan).toBe(true);
  });
  it("distinguishes same-host coding vs normal by path substring", () => {
    const coding = matchPlatform("", PRESETS, [
      { url: "https://open.bigmodel.cn/api/coding/paas/v4", protocol: "openai" },
    ]);
    expect(coding?.value).toBe("glm_coding");
    const normal = matchPlatform("", PRESETS, [
      { url: "https://open.bigmodel.cn/api/paas/v4", protocol: "openai" },
    ]);
    expect(normal?.value).toBe("glm");
  });
  it("doubao: pasting /api/coding/v3 hits doubao (longest substring = Responses endpoint host)", () => {
    const v3 = matchPlatform("", PRESETS, [
      { url: "https://ark.cn-beijing.volces.com/api/coding/v3", protocol: "openai" },
    ]);
    expect(v3?.value).toBe("doubao");
    // /api/coding/v3 (24 chars path) beats /api/coding → most specific host wins.
    const plain = matchPlatform("", PRESETS, [
      { url: "https://ark.cn-beijing.volces.com/api/coding", protocol: "anthropic" },
    ]);
    expect(plain?.value).toBe("doubao");
  });
  it("volces dual base_url: both URLs extracted distinctly + matches doubao (no collapse)", () => {
    // 火山方舟 CodingPlan 分享文案：anthropic /api/coding + openai /api/coding/v3 两条独立 base_url。
    const out = parsePlatformPaste(
      "火山方舟 CodingPlan Lite\n" +
        "Anthropic: https://ark.cn-beijing.volces.com/api/coding\n" +
        "OpenAI: https://ark.cn-beijing.volces.com/api/coding/v3\n" +
        "key: sk-volces-1234567890abcdef",
      PRESETS,
    );
    const urls = out.baseUrls.map((b) => b.url);
    // 两条 base_url 各自保留、不去重塌缩成一个。
    expect(urls).toContain("https://ark.cn-beijing.volces.com/api/coding");
    expect(urls).toContain("https://ark.cn-beijing.volces.com/api/coding/v3");
    expect(out.platform?.value).toBe("doubao");
  });
  it("falls back to keyword scan when no host match", () => {
    const hit = matchPlatform("使用 deepseek 模型", PRESETS);
    expect(hit?.value).toBe("deepseek");
  });
  it("excludes NEVER_AUTO_MATCH presets (mock) even on keyword hit", () => {
    expect(matchPlatform("跑个测试", PRESETS)).toBeNull();
  });
  it("returns null when nothing matches", () => {
    expect(matchPlatform("random gibberish text", PRESETS)).toBeNull();
  });
});

describe("parsePlatformPaste", () => {
  it("returns empty result for blank text", () => {
    expect(parsePlatformPaste("", PRESETS)).toEqual({
      apiKeys: [],
      baseUrls: [],
      platform: null,
      models: [],
      expiresAt: null,
    });
    expect(parsePlatformPaste("   ", PRESETS).platform).toBeNull();
  });

  it("extracts prefix-anchored api keys", () => {
    const out = parsePlatformPaste(
      "key: sk-ant-abcdefghijklmnop1234 base https://api.anthropic.com",
      PRESETS,
    );
    expect(out.apiKeys.some((k) => k.startsWith("sk-ant-"))).toBe(true);
    expect(out.baseUrls.some((b) => b.url.includes("anthropic"))).toBe(true);
  });

  it("strips anti-crawl CJK chars injected into a key", () => {
    const out = parsePlatformPaste("apikey: sk-请删除这些字proj1234567890abcd", PRESETS);
    expect(out.apiKeys.length).toBeGreaterThan(0);
    expect(out.apiKeys[0]).not.toMatch(/[一-鿿]/);
  });

  it("decodes base64-encoded keys via assignment anchor", () => {
    // base64("sk-decoded-key-1234567890") → assign value, no known prefix
    const b64 = Buffer.from("sk-decoded-key-1234567890").toString("base64");
    const out = parsePlatformPaste(`API_KEY（base64编码）: ${b64}`, PRESETS);
    expect(out.apiKeys).toContain("sk-decoded-key-1234567890");
  });

  it("extracts base_url, dedups, skips image assets", () => {
    const out = parsePlatformPaste(
      "url https://api.deepseek.com/v1 logo https://cdn.x/logo.png again https://api.deepseek.com/v1",
      PRESETS,
    );
    const urls = out.baseUrls.map((b) => b.url);
    expect(urls).toContain("https://api.deepseek.com/v1");
    expect(urls.some((u) => u.endsWith(".png"))).toBe(false);
    // dedup
    expect(urls.filter((u) => u === "https://api.deepseek.com/v1").length).toBe(1);
  });

  it("matches platform from extracted base_url", () => {
    const out = parsePlatformPaste("接口地址 https://api.deepseek.com/v1", PRESETS);
    expect(out.platform?.value).toBe("deepseek");
  });

  it("extracts bare base64 key (no label) when decoded shape is key-like", () => {
    // base64 of "tt-bareKeyAbcdefghij1234567890" (matches DECODED_KEY_SHAPE tt-...)
    const b64 = Buffer.from("tt-barekeyabcdefghij1234567890").toString("base64");
    const out = parsePlatformPaste(`随便一段文字 ${b64} 结尾`, PRESETS);
    expect(out.apiKeys).toContain("tt-barekeyabcdefghij1234567890");
  });

  it("rejoins anti-crawl CJK split bare base64 then decodes", () => {
    const full = Buffer.from("sk-joinedkeyabcdefghij1234567890").toString("base64");
    const cut = Math.floor(full.length / 2);
    // 在 base64 串中间插入 CJK 噪声指令
    const injected = `${full.slice(0, cut)}删掉我再base64解码${full.slice(cut)}`;
    const out = parsePlatformPaste(injected, PRESETS);
    expect(out.apiKeys.some((k) => k.startsWith("sk-joinedkey"))).toBe(true);
  });

  it("assignment-anchored plaintext long key without known prefix is kept", () => {
    const out = parsePlatformPaste("token: ABCDEFGHIJKLMNOPQRSTUVWX1234", PRESETS);
    expect(out.apiKeys).toContain("ABCDEFGHIJKLMNOPQRSTUVWX1234");
  });

  it("parses base64-decoded chinese compound label string", () => {
    const compound = "令牌sk-cmp-abcdefghijklmnop地址https://api.deepseek.com/v1模型deepseek-chat";
    const b64 = Buffer.from(compound, "utf-8").toString("base64");
    const out = parsePlatformPaste(b64, PRESETS);
    expect(out.apiKeys.some((k) => k.startsWith("sk-cmp-"))).toBe(true);
    expect(out.baseUrls.some((b) => b.url.includes("deepseek"))).toBe(true);
    expect(out.models.length).toBeGreaterThan(0);
  });

  it("mimo token plan (tp- prefix via anti-crawl base64) upgrades to coding plan", () => {
    // 反爬中文插 base64 中间: 剔中文拼接后解码 = tp-cd0mfe829...token
    const full = Buffer.from("tp-cd0mfe829kk20chvj4n92ujw8synkxw5vqv5z67qx2k569qv").toString("base64");
    const cut = Math.floor(full.length / 2);
    const injected = `分享MIMO 即将过期 ${full.slice(0, cut)}使劲蹬啊${full.slice(cut)} 自己蹬不动了 lark_024`;
    const out = parsePlatformPaste(injected, PRESETS);
    expect(out.apiKeys.some(k => k.startsWith("tp-"))).toBe(true);
    expect(out.platform?.value).toBe("xiaomi_mimo");
    expect(out.platform?.codingPlan).toBe(true);
  });

  it("整段 base64 分享文本：裸 key（无标签）解码后补提 + 识别 MiMo coding + /v1 base_url", () => {
    // 论坛分享帖：整段配置 base64 编码，解码后 key 裸在末尾无「令牌/密钥/key」标签，
    // parseCompoundLabeled 按「接口」标签切分时把裸 key 归入接口段被 URL 正则忽略致漏提。
    // 解码得：兼容 OpenAI 接口协议：\nhttps://token-plan-cn.xiaomimimo.com/v1\n
    //         兼容 Anthropic 接口协议：\nhttps://token-plan-cn.xiaomimimo.com/anthropic\n
    //         tp-ctzbh681u6dgc5axrzs7rrnfajch92w06q80yr68075wh647
    const b64 =
      "5YW85a65IE9wZW5BSSDmjqXlj6PljY/orq7vvJoKaHR0cHM6Ly90b2tlbi1wbGFuLWNuLnhpYW9taW1pbW8uY29tL3YxCuWFvOWuuSBBbnRocm9waWMg5o6l5Y+j5Y2P6K6u77yaCmh0dHBzOi8vdG9rZW4tcGxhbi1jbi54aWFvbWltaW1vLmNvbS9hbnRocm9waWMKdHAtY3R6Ymg2ODF1NmRnYzVheHJ6czdycm5mYWpjaDkydzA2cTgweXI2ODA3NXdoNjQ3";
    const out = parsePlatformPaste(b64, PRESETS);
    // 裸 key 经 PREFIX_TOKEN_RE 兜底补提
    expect(out.apiKeys.some(k => k.startsWith("tp-ctzbh"))).toBe(true);
    // platform 命中 MiMo coding（token-plan-cn host → coding 变体）
    expect(out.platform?.value).toBe("xiaomi_mimo");
    expect(out.platform?.codingPlan).toBe(true);
    // base_url 含 /v1（首个 OpenAI 兼容端点）
    expect(out.baseUrls.some(b => b.url === "https://token-plan-cn.xiaomimimo.com/v1")).toBe(true);
  });

  it("机制 B：纯 token 粘贴（无 base_url）命中 codingKeyPrefixes → 升级 coding plan", () => {
    // 无 base_url，host 匹配（机制 A）触不到 coding host；靠 keyword 命中普通 xiaomi_mimo
    // 后由 tp- 前缀（codingKeyPrefixes 数据驱动）升级到 coding 变体。
    const out = parsePlatformPaste(
      "小米 MiMo 套餐 key: tp-abc1234567890defghijklmnop",
      PRESETS,
    );
    expect(out.apiKeys.some(k => k.startsWith("tp-"))).toBe(true);
    expect(out.platform?.value).toBe("xiaomi_mimo");
    expect(out.platform?.codingPlan).toBe(true);
  });

  it("机制 B 守卫：普通 mimo key（无 codingKeyPrefixes 前缀）不误升级 coding plan", () => {
    // 命中普通 xiaomi_mimo（keyword），key 非 tp- 前缀 → 保持普通版，codingPlan 不置真。
    const out = parsePlatformPaste(
      "小米 MiMo 普通版 key: sk-abc1234567890defghijklmnop",
      PRESETS,
    );
    expect(out.platform?.value).toBe("xiaomi_mimo");
    expect(out.platform?.codingPlan).toBeFalsy();
  });

  it("MiMo PRO 分享文案：coding plan + expiresAt 联合识别", () => {
    // 社区分享帖典型形态：token-plan host（机制 A）+ tp- key + 「6.27 到期」。
    const out = parsePlatformPaste(
      "MiMo PRO 分享 https://token-plan-cn.xiaomimimo.com/v1 key tp-abc1234567890defghij 6.27 到期",
      PRESETS,
    );
    expect(out.platform?.value).toBe("xiaomi_mimo");
    expect(out.platform?.codingPlan).toBe(true);
    expect(out.expiresAt).not.toBeNull();
    expect(out.expiresAt).toBeGreaterThan(0);
  });

  it("lark substring does not false-match doubao (ark keyword too short)", () => {
    // 文案含 lark_024 (含 ark 子串) 但无火山语义 → 不应命中 doubao
    const out = parsePlatformPaste("由 lin2101 发布 lark_024 文化宣导员 sgp吗", PRESETS);
    expect(out.platform?.value).not.toBe("doubao");
  });

  it("ark- prefix key extracted (火山方舟 apikey 前缀)", () => {
    // 火山方舟 key 前缀 ark-，KEY_PREFIXES 锚定后抽取。
    const out = parsePlatformPaste(
      "火山 key: ark-9a96aed4c0e474c9c0949581a00fef7c3c6",
      PRESETS,
    );
    expect(out.apiKeys.some(k => k.startsWith("ark-"))).toBe(true);
    expect(out.apiKeys.some(k => k.includes("9a96aed"))).toBe(true);
  });

  it("strips circled-numeral anti-crawl chars (①②③ U+2460-247F) from key", () => {
    // 社区分享防爬：圈数字 ②⑤⑨ 替换明文数字穿插在 key 中。
    const out = parsePlatformPaste(
      "key: ark-9a②96aed-4c0e-474c-9c09-49⑤8⑨1a00fef-7c3c6",
      PRESETS,
    );
    expect(out.apiKeys.length).toBeGreaterThan(0);
    // 圈数字须全部剔除，key 不含任何 Enclosed Alphanumerics。
    expect(out.apiKeys[0]).not.toMatch(/[①-⓿]/);
    expect(out.apiKeys.some(k => k.startsWith("ark-9a"))).toBe(true);
  });

  it("volces agent plan 全流程：识别 doubao + /api/plan 端点 + ark- key + 圈数字剔除", () => {
    // 用户报文案典型形态：双 base_url（agent plan 端点）+ ark- key + 圈数字防爬。
    const out = parsePlatformPaste(
      "火山方舟 Agent Plan 分享\n" +
        "Anthropic: https://ark.cn-beijing.volces.com/api/plan\n" +
        "OpenAI: https://ark.cn-beijing.volces.com/api/plan/v3\n" +
        "apikey: ark-9a②96aed-4c0e-474c-9c09-49⑤8⑨1a00fef-7c3c6（圈数字换成1以此类推）",
      PRESETS,
    );
    // 识别为 doubao
    expect(out.platform?.value).toBe("doubao");
    // 双 base_url 均抽出（agent plan 端点，非 coding plan）
    const urls = out.baseUrls.map(b => b.url);
    expect(urls).toContain("https://ark.cn-beijing.volces.com/api/plan");
    expect(urls).toContain("https://ark.cn-beijing.volces.com/api/plan/v3");
    // ark- key 抽出，圈数字剔除
    expect(out.apiKeys.some(k => k.startsWith("ark-9a96aed"))).toBe(true);
    expect(out.apiKeys.some(k => !/[①-⓿]/.test(k))).toBe(true);
  });

  it("coding plan 不回归：/api/coding 文案仍命中 doubao（非 agent plan 端点）", () => {
    // 既有 coding plan 文案（无圈数字、sk- key）端点应仍是 /api/coding，不被新 agent plan 干扰。
    const out = parsePlatformPaste(
      "火山方舟 CodingPlan\n" +
        "Anthropic: https://ark.cn-beijing.volces.com/api/coding\n" +
        "OpenAI: https://ark.cn-beijing.volces.com/api/coding/v3\n" +
        "key: sk-volces-1234567890abcdef",
      PRESETS,
    );
    expect(out.platform?.value).toBe("doubao");
    const urls = out.baseUrls.map(b => b.url);
    expect(urls).toContain("https://ark.cn-beijing.volces.com/api/coding");
    expect(urls).toContain("https://ark.cn-beijing.volces.com/api/coding/v3");
    // 不应误抽 agent plan 端点
    expect(urls.some(u => u.includes("/api/plan"))).toBe(false);
  });

  it("parses '即将过期 MM-DD HH:MM' from community share text", () => {
    // 构造一个未来日期的 MM-DD HH:MM（用当前月日 + 1 月）。
    const now = new Date();
    const future = new Date(now.getFullYear(), now.getMonth() + 1, 28, 23, 59);
    const mo = future.getMonth() + 1;
    const d = future.getDate();
    const txt = `分享 MIMO token 即将过期 ${String(mo).padStart(2, "0")}-${String(d).padStart(2, "0")} 23:59`;
    const out = parsePlatformPaste(txt, PRESETS);
    expect(out.expiresAt).not.toBeNull();
    expect(out.expiresAt! > Date.now()).toBe(true);
    // 解出的日期应匹配月份/日
    const parsed = new Date(out.expiresAt!);
    expect(parsed.getMonth() + 1).toBe(mo);
    expect(parsed.getDate()).toBe(d);
    expect(parsed.getHours()).toBe(23);
    expect(parsed.getMinutes()).toBe(59);
  });

  it("skips historical dates (older than 7 days) as expiry", () => {
    // YYYY-MM-DD 形式的去年日期 → 远早于 now - 7d → 不应回填。
    // （注意：MM-DD 形式会被 parseCandidate 推到次年，永远落在未来，不能用于测试历史跳过。）
    const lastYear = new Date().getFullYear() - 1;
    const out = parsePlatformPaste(`老帖 过期 ${lastYear}-01-01 00:00`, PRESETS);
    expect(out.expiresAt).toBeNull();
  });

  it("date-level candidate (no time) → end-of-day 23:59:59.999", () => {
    // 2026-06-25 PRD S3：日期级（无时间分量，如「即将过期 06-28」）→ expiresAt = 该日本地 23:59:59.999。
    // 不应是 00:00（否则当日中午被认作已过期）。
    const now = new Date();
    const future = new Date(now.getFullYear(), now.getMonth() + 1, 28); // 下月 28 日（未来）
    const mo = String(future.getMonth() + 1).padStart(2, "0");
    const d = String(future.getDate()).padStart(2, "0");
    const txt = `即将过期 ${mo}-${d}`;
    const out = parsePlatformPaste(txt, PRESETS);
    expect(out.expiresAt).not.toBeNull();
    const parsed = new Date(out.expiresAt!);
    expect(parsed.getMonth() + 1).toBe(Number(mo));
    expect(parsed.getDate()).toBe(Number(d));
    // 关键：本地时间 23:59:59.999（end-of-day）。
    expect(parsed.getHours()).toBe(23);
    expect(parsed.getMinutes()).toBe(59);
    expect(parsed.getSeconds()).toBe(59);
    expect(parsed.getMilliseconds()).toBe(999);
  });

  it("date-level candidate YYYY-MM-DD → end-of-day 23:59:59.999", () => {
    // 全数字日期 YYYY-MM-DD 无时间分量也走 end-of-day。
    // 取明年同月同日（避免历史无效）。
    const now = new Date();
    const futureY = now.getFullYear() + 1;
    const mo = String(now.getMonth() + 1).padStart(2, "0");
    const d = String(now.getDate()).padStart(2, "0");
    const txt = `过期 ${futureY}-${mo}-${d}`;
    const out = parsePlatformPaste(txt, PRESETS);
    expect(out.expiresAt).not.toBeNull();
    const parsed = new Date(out.expiresAt!);
    expect(parsed.getHours()).toBe(23);
    expect(parsed.getMinutes()).toBe(59);
    expect(parsed.getSeconds()).toBe(59);
    expect(parsed.getMilliseconds()).toBe(999);
  });

  it("date+time candidate keeps original time (not end-of-day)", () => {
    // 带时间分量（HH:MM）→ 保持原时间，不走 end-of-day。
    const now = new Date();
    const future = new Date(now.getFullYear(), now.getMonth() + 1, 15, 18, 30);
    const mo = String(future.getMonth() + 1).padStart(2, "0");
    const d = String(future.getDate()).padStart(2, "0");
    const txt = `过期 ${mo}-${d} 18:30`;
    const out = parsePlatformPaste(txt, PRESETS);
    expect(out.expiresAt).not.toBeNull();
    const parsed = new Date(out.expiresAt!);
    expect(parsed.getHours()).toBe(18);
    expect(parsed.getMinutes()).toBe(30);
    expect(parsed.getSeconds()).toBe(0);
  });

  it("picks date closest to expiry keyword when multiple candidates", () => {
    // 两日期：一个邻近「过期」语义，一个远离。取近语义的。
    const now = new Date();
    // 近过期词的日期：下月 15 日（未来）
    const nearFuture = new Date(now.getFullYear(), now.getMonth() + 1, 15, 23, 59);
    // 远离词的日期：下月 25 日（未来，但更远）
    const farFuture = new Date(now.getFullYear(), now.getMonth() + 1, 25, 23, 59);
    const nearMo = String(nearFuture.getMonth() + 1).padStart(2, "0");
    const nearD = String(nearFuture.getDate()).padStart(2, "0");
    const farMo = String(farFuture.getMonth() + 1).padStart(2, "0");
    const farD = String(farFuture.getDate()).padStart(2, "0");
    // 「过期」前缀 near，far 出现在文末且无语义词邻近。
    const txt = `过期 ${nearMo}-${nearD} 23:59\n另一条信息 ${farMo}-${farD} 23:59`;
    const out = parsePlatformPaste(txt, PRESETS);
    expect(out.expiresAt).not.toBeNull();
    const parsed = new Date(out.expiresAt!);
    expect(parsed.getDate()).toBe(15, "should pick near-keyword date (15), got " + parsed.getDate());
  });

  it("returns null when text has no expiry keyword (tightened mode)", () => {
    // 收紧（2026-06-25 bug 修复）：无任何「过期/到期/exp/有效期」语义词的文案，
    // 即便含未来日期也不识别（防「更新于 2026-07-15」「版本计划 08-20」类帖误识别灌表单）。
    const now = new Date();
    const future = new Date(now.getFullYear(), now.getMonth() + 1, 10, 12, 0);
    const mo = String(future.getMonth() + 1).padStart(2, "0");
    const d = String(future.getDate()).padStart(2, "0");
    const txt = `活动 ${mo}-${d} 12:00 开始`;
    const out = parsePlatformPaste(txt, PRESETS);
    expect(out.expiresAt).toBeNull();
  });

  it("returns null when keyword present but date too far (> 60 chars)", () => {
    // 语义词存在但所有日期候选距语义词均 > 60 字符 → 视为无关日期。
    const now = new Date();
    const future = new Date(now.getFullYear(), now.getMonth() + 1, 15, 23, 59);
    const mo = String(future.getMonth() + 1).padStart(2, "0");
    const d = String(future.getDate()).padStart(2, "0");
    // 70 字 filler 隔开「过期」与日期 → 距离 > 60 → null。
    const filler = "x".repeat(70);
    const txt = `过期${filler}${mo}-${d} 23:59`;
    const out = parsePlatformPaste(txt, PRESETS);
    expect(out.expiresAt).toBeNull();
  });

  it("returns null for plain '更新于 YYYY-MM-DD' without keyword", () => {
    // 论坛帖更新日期类文案：无过期语义词 → 不识别。
    const txt = "更新于 2026-07-15 by lin2101";
    const out = parsePlatformPaste(txt, PRESETS);
    expect(out.expiresAt).toBeNull();
  });
});

describe("extractExpiryAt — MM.DD (月.日) format", () => {
  // 固定 now = 2026-06-25 12:00 本地，避免跨日/跨年漂移。
  // 注意：extractExpiryAt 内部 new Date(y, mo-1, d) 走本地时区，测试同样用本地 Date 构造期望值。
  const NOW = new Date(2026, 5, 25, 12, 0).getTime();

  it("识别 'PRO套餐，6.27到期' → 当年 2026-06-27 23:59:59.999", () => {
    // 社区分享帖原样本：M.D 到期格式，当年未过 → end-of-day。
    const ts = extractExpiryAt("分享一个MIMO key，PRO套餐，6.27到期", NOW);
    expect(ts).not.toBeNull();
    const d = new Date(ts!);
    expect(d.getFullYear()).toBe(2026);
    expect(d.getMonth() + 1).toBe(6);
    expect(d.getDate()).toBe(27);
    expect(d.getHours()).toBe(23);
    expect(d.getMinutes()).toBe(59);
    expect(d.getSeconds()).toBe(59);
    expect(d.getMilliseconds()).toBe(999);
  });

  it("语义词在前 ('过期 6.27') 同样识别", () => {
    const ts = extractExpiryAt("过期 6.27", NOW);
    expect(ts).not.toBeNull();
    const d = new Date(ts!);
    expect(d.getMonth() + 1).toBe(6);
    expect(d.getDate()).toBe(27);
    expect(d.getHours()).toBe(23);
  });

  it("无语义词的 '6.27' → null (收紧防护硬门仍生效)", () => {
    // 收紧核心：版本号类语境被语义词硬门挡掉。
    expect(extractExpiryAt("版本 Claude 4.5 发布", NOW)).toBeNull();
    expect(extractExpiryAt("随机文案 6.27 普通文字", NOW)).toBeNull();
  });

  it("'12.31到期' 当年未过 → 当年 12-31 23:59:59", () => {
    // now=2026-06-25, 12-31 仍在未来 → 当年。
    const ts = extractExpiryAt("12.31到期", NOW);
    expect(ts).not.toBeNull();
    const d = new Date(ts!);
    expect(d.getFullYear()).toBe(2026);
    expect(d.getMonth() + 1).toBe(12);
    expect(d.getDate()).toBe(31);
    expect(d.getHours()).toBe(23);
    expect(d.getMinutes()).toBe(59);
  });

  it("'1.15到期' 当年已过 → 推次年 2027-01-15 23:59:59", () => {
    // now=2026-06-25, 1-15 已过且非今天 → parseCandidate 推次年。
    const ts = extractExpiryAt("1.15到期", NOW);
    expect(ts).not.toBeNull();
    const d = new Date(ts!);
    expect(d.getFullYear()).toBe(2027);
    expect(d.getMonth() + 1).toBe(1);
    expect(d.getDate()).toBe(15);
    expect(d.getHours()).toBe(23);
  });
});

// ─── 全协议回归矩阵（platform-presets.json 数据驱动）─────────────────────
// platform-detect-minimax s3：fixture 补 anthropic + 遍历全 presets 协议，
// 每协议造典型分享帖（自身 keywords + anthropic/openai 通用干扰词「官方 API」），
// 断言 matchPlatform 识别为自身（非 anthropic/openai 抢匹配）。根因根治验证：
// s1 打分（命中数 desc > 最长命中 keyword 优先）让多 keyword 协议胜出；
// s2 codingKeyPrefixes 前置判定让 minimax/xiaomi_mimo 等 sk-cp-/tp- key 协议跳过 keyword 路径。

/** presets.json 单协议 entry 的最小子集（仅取解析器所需字段，避免 any）。 */
interface PresetEntry {
  keywords?: string[];
  codingKeyPrefixes?: string[];
  is_coding_plan?: boolean;
  name?: { "en-US"?: string; [locale: string]: string | undefined };
  endpoints?: {
    default?: { base_url: string }[];
    coding_plan?: { base_url: string }[];
  };
}

/**
 * 从 presets.json 派生 PastePresetRef[]——镜像 defaults.ts::buildProtocolsFromPresets +
 * deriveProtocolHosts（hosts = host+path 子串，让同 host 分裂如 glm 普通/coding 可区分）。
 * 单一事实源：presets.json 改动自动反映到矩阵，无需手维护第二份 fixture。
 * ponytail: 内联派生而非 import 真实 loadDoc（避免 RPC + async 测试复杂度），逻辑等同。
 */
function buildAllPresetsFromJson(): PastePresetRef[] {
  const protocols = (presetsJson as { protocols: Record<string, PresetEntry> }).protocols ?? {};
  const out: PastePresetRef[] = [];
  for (const [key, entry] of Object.entries(protocols)) {
    const hosts = new Set<string>();
    const epsAll = [
      ...(entry.endpoints?.default ?? []),
      ...(entry.endpoints?.coding_plan ?? []),
    ];
    for (const ep of epsAll) {
      try {
        const u = new URL(ep.base_url);
        const host = u.host.replace(/^www\./, "").toLowerCase();
        const path = u.pathname.replace(/\/+$/, "").toLowerCase();
        const sub = path && path !== "/" ? host + path : host;
        if (host) hosts.add(sub);
      } catch {
        // 非法 URL 跳过（同 deriveProtocolHosts 容错）
      }
    }
    out.push({
      value: key,
      label: entry.name?.["en-US"] ?? key,
      keywords: entry.keywords ?? [],
      ...(hosts.size ? { hosts: [...hosts] } : {}),
      ...(entry.is_coding_plan ? { codingPlan: true } : {}),
      ...(entry.codingKeyPrefixes?.length ? { codingKeyPrefixes: entry.codingKeyPrefixes } : {}),
    });
  }
  return out;
}

const ALL_PRESETS = buildAllPresetsFromJson();
const PROTOCOLS_JSON = (presetsJson as { protocols: Record<string, PresetEntry> }).protocols;

/**
 * 已知弱 keyword 协议：brand keyword 含 "claude" 子串 → anthropic keyword "claude" 免费命中，
 * 叠加 "官方" 干扰词 = 2 hits，胜过单 brand keyword 的 1 hit。
 * SPEC: 交 main 判是否补更独特 keyword（如 "claude api 中转"/"claude cn 镜像"），禁擅自改 presets.json。
 */
const KNOWN_WEAK_KEYWORD_PROTOCOLS = new Set(["claudeapi", "claudecn"]);

describe("全协议回归矩阵（platform-presets.json 数据驱动）", () => {
  // 排除 mock（NEVER_AUTO_MATCH）、xiaomi_mimo_coding（coding 变体同族，需 tp- key 区分，见专项测试）、
  // KNOWN_WEAK_KEYWORD_PROTOCOLS（brand 含 "claude" 子串，单独文档化）。
  const matrixProtocols = Object.entries(PROTOCOLS_JSON).filter(
    ([k]) => k !== "mock" && k !== "xiaomi_mimo_coding" && !KNOWN_WEAK_KEYWORD_PROTOCOLS.has(k),
  );

  it.each(matrixProtocols)(
    "keyword 识别: %s 自身 keywords + ' 官方 API' 干扰 → 识别为自身",
    (key, entry) => {
      const e = entry as PresetEntry;
      // 典型分享帖：自身全 keywords（最大化命中数）+ anthropic/openai 通用干扰词。
      // anthropic/openai 各得 "官方" 1 hit；目标协议靠 ≥2 distinctive keywords 或更长 keyword 胜出。
      const text = (e.keywords ?? []).join(" ") + " 官方 API";
      const hit = matchPlatform(text, ALL_PRESETS);
      expect(hit?.value).toBe(key);
    },
  );

  it("anthropic 不回归：纯 anthropic 文案（无其他平台词）→ anthropic", () => {
    // 无更强信号竞争时，anthropic 自身文案仍识别正确。
    expect(matchPlatform("claude 克劳德 官方", ALL_PRESETS)?.value).toBe("anthropic");
  });

  it("openai 自身文案 → openai（非 anthropic 抢匹配，二者共享 '官方' keyword）", () => {
    // anthropic 与 openai 共享 "官方"，但 openai 的 "gpt"+"chatgpt" 命中数胜出。
    expect(matchPlatform("gpt chatgpt 官方", ALL_PRESETS)?.value).toBe("openai");
  });

  it("minimax 根因场景（s1 打分根治）：'minimax 海螺 claude code 官方' → minimax（非 anthropic）", () => {
    // bug 根因：minimax 分享帖含 "claude code 官方" 致旧版 idx0 anthropic 抢匹配。
    // s1 打分：minimax 2 hits（海螺 + minimax，longest=7）vs anthropic 2 hits（claude + 官方，longest=6）→
    // tie 命中数后最长 keyword 胜出 → minimax。literal 文案无 key 纯走 keyword 路径验证打分逻辑。
    const hit = matchPlatform("minimax 海螺 claude code 官方", ALL_PRESETS);
    expect(hit?.value).toBe("minimax");
  });

  it("minimax 根因场景（s2 codingKeyPrefixes 前置）：含 sk-cp- key 的分享帖 → minimax", () => {
    // s2 根治：parsePlatformPaste 优先级 2 命中 sk-cp- 前缀直接返 minimax，跳过 keyword 打分。
    // 含 key 的真实分享帖（key 用脱敏 placeholder，禁用真实 key）。
    const out = parsePlatformPaste(
      "minimax 海螺 claude code 官方 API key sk-cp-abcdefghijklmnop1234567890",
      ALL_PRESETS,
    );
    expect(out.apiKeys.some((k) => k.startsWith("sk-cp-"))).toBe(true);
    expect(out.platform?.value).toBe("minimax");
  });

  it("xiaomi_mimo 同族：tp- key（无文案无 URL）→ xiaomi_mimo（presets.json 中 xiaomi_mimo 与 xiaomi_mimo_coding 共享 tp- 前缀，首个胜出）", () => {
    // SPEC 数据现状：presets.json 中 xiaomi_mimo 与 xiaomi_mimo_coding 均 codingKeyPrefixes=['tp-']，
    // parsePlatformPaste priority-2 取首个匹配 → xiaomi_mimo（普通版，在 JSON 中先于 coding 变体）。
    // coding 变体在真实场景靠 host 区分（token-plan-cn.xiaomimimo.com，机制 A），纯 token 无法区分。
    // ponytail: 测试断言实际行为（非理想行为），让数据歧义可见；交 main 判是否调整 codingKeyPrefixes 归属。
    const out = parsePlatformPaste("tp-abcdefghijklmnop1234567890", ALL_PRESETS);
    expect(out.apiKeys.some((k) => k.startsWith("tp-"))).toBe(true);
    expect(out.platform?.value).toBe("xiaomi_mimo");
  });

  it("SPEC 已知限制: claudeapi/claudecn brand 含 'claude' 子串，'官方 API' 干扰下 anthropic 抢匹配", () => {
    // 文档化（不禁跳过，让限制可见）：keyword 打分下 anthropic "claude"(子串命中 brand) + "官方" = 2 hits，
    // 而 claudeapi/claudecn 单 brand keyword = 1 hit → anthropic 胜。
    // 真实场景影响：用户粘贴 "claudeapi 官方 API key sk-..." 会被识别为 anthropic 而非 claudeapi。
    // SPEC(交 main 判): 是否给 claudeapi/claudecn 补 distinctive keyword（如 "claude api 中转"/"claude cn 镜像"），
    // 或调整 keyword 打分权重（brand 完整匹配 > 子串匹配），禁擅自改 presets.json。
    for (const proto of ["claudeapi", "claudecn"]) {
      const entry = PROTOCOLS_JSON[proto];
      const text = (entry.keywords ?? []).join(" ") + " 官方 API";
      expect(matchPlatform(text, ALL_PRESETS)?.value).toBe("anthropic");
    }
  });
});
