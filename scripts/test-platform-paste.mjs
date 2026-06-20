// 平台「智能粘贴」解析器回归测试（无 vitest，直接 node 跑）。
// 用 esbuild 即时转译 src/utils/platformPaste.ts 后断言关键用例。
// 运行: node scripts/test-platform-paste.mjs
import { execSync } from "node:child_process";

const SRC = "src/utils/platformPaste.ts";
const OUT = "/tmp/aidog-platformPaste.test.mjs";
execSync(
  `npx esbuild ${SRC} --format=esm --outfile=${OUT} --log-level=error`,
  { stdio: "inherit" },
);
const { parsePlatformPaste } = await import(OUT);

const presets = [
  { value: "deepseek", label: "DeepSeek", hosts: ["api.deepseek.com"], keywords: ["deepseek", "深度求索"] },
  { value: "anthropic", label: "Anthropic", hosts: ["api.anthropic.com"], keywords: ["claude"] },
  // mock 占位平台：keyword 含通用子串「测试」，验证永不被自动匹配。
  { value: "mock", label: "Mock", keywords: ["mock", "测试", "调试", "假数据"] },
];

let failed = 0;
// platform: 传字符串断言 value 命中；传 null 断言「未匹配」（r.platform === null）。
function check(name, text, { base, key, model, platform } = {}) {
  const r = parsePlatformPaste(text, presets);
  const okBase = base === undefined || r.baseUrls.some((b) => b.url === base);
  const okKey = key === undefined || r.apiKeys.includes(key);
  const okModel = model === undefined || (r.models ?? []).includes(model);
  const okPlatform =
    platform === undefined ||
    (platform === null ? r.platform === null : r.platform?.value === platform);
  const ok = okBase && okKey && okModel && okPlatform;
  if (!ok) failed++;
  console.log(`[${ok ? "PASS" : "FAIL"}] ${name}`);
  if (!ok) {
    console.log("  baseUrls:", JSON.stringify(r.baseUrls));
    console.log("  apiKeys :", JSON.stringify(r.apiKeys));
    console.log("  models  :", JSON.stringify(r.models));
    console.log("  platform:", JSON.stringify(r.platform));
    if (base !== undefined) console.log("  expBase :", base);
    if (key !== undefined) console.log("  expKey  :", key);
    if (model !== undefined) console.log("  expModel:", model);
    if (platform !== undefined) console.log("  expPlat :", platform);
  }
}

// 明文 base_url / apikey 中间插全角括号 CJK 噪声（本次修复目标用例）。
check(
  "plaintext-cjk-noise (sk_ 前缀 + 全角括号噪声)",
  `https://srapi.se（删除我）nran.net/v1
sk_8fed78f353bb_4db3c7142426b201e2c6600236de69b2（删除我）ce9dc2541f2da7b055fb60ec882f64b5`,
  {
    base: "https://srapi.senran.net/v1",
    key: "sk_8fed78f353bb_4db3c7142426b201e2c6600236de69b2ce9dc2541f2da7b055fb60ec882f64b5",
  },
);

// 回归：干净的 base_url + sk- key。
check("clean sk- key", "base_url: https://api.deepseek.com/v1  key: sk-abc123def456ghi789jkl", {
  base: "https://api.deepseek.com/v1",
  key: "sk-abc123def456ghi789jkl",
});

// 回归：sk-ant- 前缀不被 sk-/sk_ 抢吃。
check("sk-ant- prefix", "https://api.anthropic.com  sk-ant-api03-abcdefghijklmnop1234567890", {
  base: "https://api.anthropic.com",
  key: "sk-ant-api03-abcdefghijklmnop1234567890",
});

// 回归：正常中文平台名紧邻 URL 不污染 URL（URL 本不含 CJK）。
check("chinese platform name near url", "深度求索平台 https://api.deepseek.com/v1", {
  base: "https://api.deepseek.com/v1",
});

// 回归：base64 变体（上次 commit 5f87600 修复，path 3.5）。
const tpKey = "tp-abcdefghijklmnopqrstuvwxyz123456";
const b64 = Buffer.from(tpKey).toString("base64");
const mid = Math.floor(b64.length / 2);
const noisy = b64.slice(0, mid) + "删掉我再base64解码" + b64.slice(mid);
check("base64 cjk-noise variant", `KEY: ${noisy}`, { key: tpKey });

// 第三变体：base64 解码后是「中文标签紧贴值」复合串。
// 解码 = 令牌sk-...地址https://www.supertoken.lol/v1/messages模型名MiniMax-M3
const compoundB64 =
  "5Luk54mMc2stV1VsYld3dmdoeFpUWHVaS2k1NmlFcXJRVVk4MDU1VzRaa0N2c3VhQXNrbTNISHc55Zyw5Z2AaHR0cHM6Ly93d3cuc3VwZXJ0b2tlbi5sb2wvdjEvbWVzc2FnZXPmqKHlnovlkI1NaW5pTWF4LU0z";
check("base64 cjk-label compound", `这里分享一个 ${compoundB64} 自己 base64 解码`, {
  key: "sk-WUlbWwvghxZTXuZKi56iEqrQUY8055W4ZkCvsuaAskm3HHw9",
  base: "https://www.supertoken.lol/v1",
  model: "MiniMax-M3",
});

// ── matchPlatform: mock 永不自动匹配 + 未知 host 不误判 ──

// 未知 host（无预设）粘贴：不该匹配任何平台（绝不该是 mock）。
check("unknown host not matched as mock", "https://srapi.senran.net/v1  sk_8fed78f353bb_4db3c7142426b201e2c6600236de69b2ce9dc2541f2da7b055fb60ec882f64b5", {
  base: "https://srapi.senran.net/v1",
  platform: null,
});

// 文案含「测试」噪声（论坛分享高发词）：仍不该被 mock 关键字吃掉。
check("test/debug noise does not trigger mock", "测试一下这个接口 调试用 https://srapi.senran.net/v1", {
  platform: null,
});

// 文本直接含「mock」字样也不自动匹配（用户须手动选）。
check("literal mock keyword not auto-matched", "mock 假数据 本地模拟 https://srapi.senran.net/v1", {
  platform: null,
});

// 回归：已知 host 仍正常匹配（host 优先）。
check("known host still matches (deepseek)", "https://api.deepseek.com/v1  key sk-abc123def456ghi789", {
  platform: "deepseek",
});

// 回归：已知 keyword fallback 仍正常匹配（无 host 命中时）。
check("known keyword fallback still matches", "深度求索 平台分享", {
  platform: "deepseek",
});

if (failed > 0) {
  console.error(`\n${failed} 个用例失败`);
  process.exit(1);
}
console.log("\n全部通过");
