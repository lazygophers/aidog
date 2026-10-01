// 假上游：Anthropic POST /v1/messages 与 OpenAI POST /v1/chat/completions，流式 SSE / 非流式 JSON，均带 usage。
// 用法: node mock_upstream.mjs            （env: MOCK_PORT=18901 CHUNKS=20 DELAY_MS=20 CHUNK_TEXT="hello "）
// 计数: GET /stats → {"messages":n,"chat":n,"bytes":n}；POST /stats/reset 清零。
import http from "node:http";

const PORT = +(process.env.MOCK_PORT || 18901);
const CHUNKS = +(process.env.CHUNKS || 20);
const DELAY = +(process.env.DELAY_MS || 20);
const TEXT = process.env.CHUNK_TEXT || "hello ";
const stats = { messages: 0, chat: 0, bytes: 0, other: 0 };
const sleep = (ms) => (ms > 0 ? new Promise((r) => setTimeout(r, ms)) : Promise.resolve());
const isStream = (s) => /"stream"\s*:\s*true/.test(s); // ponytail: 正则代替整体 JSON.parse，省 1 MB 体解析开销
const modelOf = (s) => (s.match(/"model"\s*:\s*"([^"]*)"/) || [, "mock-model"])[1];
const usage = { input: 1000, output: CHUNKS };

async function anthropic(res, body) {
  const model = modelOf(body);
  if (!isStream(body)) {
    res.writeHead(200, { "content-type": "application/json" });
    return res.end(JSON.stringify({
      id: "msg_mock", type: "message", role: "assistant", model,
      content: [{ type: "text", text: TEXT.repeat(CHUNKS) }],
      stop_reason: "end_turn", stop_sequence: null,
      usage: { input_tokens: usage.input, output_tokens: usage.output, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 },
    }));
  }
  res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" });
  const ev = (t, d) => res.write(`event: ${t}\ndata: ${JSON.stringify(d)}\n\n`);
  ev("message_start", { type: "message_start", message: { id: "msg_mock", type: "message", role: "assistant", model, content: [], stop_reason: null, stop_sequence: null, usage: { input_tokens: usage.input, output_tokens: 1, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 } } });
  ev("content_block_start", { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } });
  for (let i = 0; i < CHUNKS; i++) {
    await sleep(DELAY);
    ev("content_block_delta", { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: TEXT } });
  }
  ev("content_block_stop", { type: "content_block_stop", index: 0 });
  ev("message_delta", { type: "message_delta", delta: { stop_reason: "end_turn", stop_sequence: null }, usage: { output_tokens: usage.output } });
  ev("message_stop", { type: "message_stop" });
  res.end();
}

async function openai(res, body) {
  const model = modelOf(body);
  const u = { prompt_tokens: usage.input, completion_tokens: usage.output, total_tokens: usage.input + usage.output };
  if (!isStream(body)) {
    res.writeHead(200, { "content-type": "application/json" });
    return res.end(JSON.stringify({
      id: "chatcmpl-mock", object: "chat.completion", created: 1, model,
      choices: [{ index: 0, message: { role: "assistant", content: TEXT.repeat(CHUNKS) }, finish_reason: "stop" }],
      usage: u,
    }));
  }
  res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" });
  const ch = (delta, finish, extra = {}) => res.write(`data: ${JSON.stringify({ id: "chatcmpl-mock", object: "chat.completion.chunk", created: 1, model, choices: [{ index: 0, delta, finish_reason: finish }], ...extra })}\n\n`);
  ch({ role: "assistant", content: "" }, null);
  for (let i = 0; i < CHUNKS; i++) {
    await sleep(DELAY);
    ch({ content: TEXT }, null);
  }
  ch({}, "stop");
  res.write(`data: ${JSON.stringify({ id: "chatcmpl-mock", object: "chat.completion.chunk", created: 1, model, choices: [], usage: u })}\n\n`);
  res.write("data: [DONE]\n\n");
  res.end();
}

http.createServer((req, res) => {
  const bufs = [];
  req.on("data", (c) => bufs.push(c));
  req.on("end", () => {
    const body = Buffer.concat(bufs).toString();
    const path = req.url.split("?")[0];
    if (path === "/stats") { res.writeHead(200, { "content-type": "application/json" }); return res.end(JSON.stringify(stats)); }
    if (path === "/stats/reset") { Object.keys(stats).forEach((k) => (stats[k] = 0)); res.writeHead(204); return res.end(); }
    stats.bytes += body.length;
    if (req.method === "POST" && path.endsWith("/v1/messages")) { stats.messages++; return anthropic(res, body); }
    if (req.method === "POST" && path.endsWith("/chat/completions")) { stats.chat++; return openai(res, body); }
    stats.other++;
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: `mock: no route ${req.method} ${path}` }));
  });
}).listen(PORT, "127.0.0.1", () => console.log(`mock upstream on 127.0.0.1:${PORT} chunks=${CHUNKS} delay=${DELAY}ms`));
