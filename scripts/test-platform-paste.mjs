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
  { value: "deepseek", label: "DeepSeek", hosts: ["api.deepseek.com"] },
  { value: "anthropic", label: "Anthropic", hosts: ["api.anthropic.com"] },
];

let failed = 0;
function check(name, text, { base, key, model } = {}) {
  const r = parsePlatformPaste(text, presets);
  const okBase = base === undefined || r.baseUrls.some((b) => b.url === base);
  const okKey = key === undefined || r.apiKeys.includes(key);
  const okModel = model === undefined || (r.models ?? []).includes(model);
  const ok = okBase && okKey && okModel;
  if (!ok) failed++;
  console.log(`[${ok ? "PASS" : "FAIL"}] ${name}`);
  if (!ok) {
    console.log("  baseUrls:", JSON.stringify(r.baseUrls));
    console.log("  apiKeys :", JSON.stringify(r.apiKeys));
    console.log("  models  :", JSON.stringify(r.models));
    if (base !== undefined) console.log("  expBase :", base);
    if (key !== undefined) console.log("  expKey  :", key);
    if (model !== undefined) console.log("  expModel:", model);
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

if (failed > 0) {
  console.error(`\n${failed} 个用例失败`);
  process.exit(1);
}
console.log("\n全部通过");
