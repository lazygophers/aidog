// 生成 Claude Code 形状的 /v1/messages 请求体（system 块 + 40 个 tools + 多轮 text/tool_use/tool_result）。
// 用法: node gen_bodies.mjs [outdir]  → body-280k.json body-1m.json（stream:true）+ *-nostream.json
import fs from "node:fs";
import path from "node:path";

const out = process.argv[2] || path.join(path.dirname(new URL(import.meta.url).pathname), "results", "bodies");
fs.mkdirSync(out, { recursive: true });

// 确定性伪随机，保证每次生成字节一致（基线可复现）。
let seed = 42;
const rnd = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
const WORDS = "the function returns a value when called with arguments from module state config file path error handler async await request response token stream buffer index cache".split(" ");
const text = (n) => { let s = ""; while (s.length < n) s += WORDS[(rnd() * WORDS.length) | 0] + " "; return s.slice(0, n); };

const tools = Array.from({ length: 40 }, (_, i) => ({
  name: `Tool${i}`,
  description: text(1200),
  input_schema: {
    type: "object",
    properties: Object.fromEntries(Array.from({ length: 6 }, (_, j) => [`p${j}`, { type: "string", description: text(120) }])),
    required: ["p0"],
  },
}));

function build(target, stream) {
  seed = 42;
  const body = {
    model: "claude-sonnet-4-5",
    max_tokens: 32000,
    stream,
    system: [
      { type: "text", text: "You are Claude Code, Anthropic's official CLI for Claude." },
      { type: "text", text: text(12000), cache_control: { type: "ephemeral" } },
    ],
    tools,
    metadata: { user_id: "user_perf_session_0000" },
    messages: [],
  };
  let i = 0;
  while (JSON.stringify(body).length < target) {
    const id = `toolu_${String(i).padStart(6, "0")}`;
    body.messages.push({ role: "user", content: [{ type: "text", text: text(800) }] });
    body.messages.push({ role: "assistant", content: [
      { type: "text", text: text(300) },
      { type: "tool_use", id, name: `Tool${i % 40}`, input: { p0: text(200) } },
    ] });
    body.messages.push({ role: "user", content: [{ type: "tool_result", tool_use_id: id, content: text(3000) }] });
    i++;
  }
  body.messages.push({ role: "user", content: [{ type: "text", text: "continue" }] });
  return JSON.stringify(body);
}

for (const [name, size] of [["280k", 280 * 1024], ["1m", 1024 * 1024]]) {
  for (const stream of [true, false]) {
    const f = path.join(out, `body-${name}${stream ? "" : "-nostream"}.json`);
    const s = build(size, stream);
    fs.writeFileSync(f, s);
    console.log(`${f} ${s.length} bytes`);
  }
}
