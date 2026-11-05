/** Local API 与转发接口：静态条目（鉴权均为 Authorization: Bearer <分组名>）。 */
export interface StaticEndpoint { id: string; method: "GET" | "POST"; path: string; body?: string }

export const LOCAL_API: StaticEndpoint[] = [
  { id: "group-info", method: "POST", path: "/api/group-info", body: "{}" },
  { id: "notify", method: "POST", path: "/api/notify", body: '{"title":"hello","body":"world"}' },
];

export const FORWARD_API: StaticEndpoint[] = [
  { id: "messages", method: "POST", path: "/v1/messages", body: '{"model":"<model>","max_tokens":256,"messages":[{"role":"user","content":"hi"}]}' },
  { id: "chat", method: "POST", path: "/v1/chat/completions", body: '{"model":"<model>","messages":[{"role":"user","content":"hi"}]}' },
  { id: "responses", method: "POST", path: "/v1/responses", body: '{"model":"<model>","input":"hi"}' },
  { id: "embeddings", method: "POST", path: "/v1/embeddings", body: '{"model":"<model>","input":"hi"}' },
  { id: "rerank", method: "POST", path: "/v1/rerank", body: '{"model":"<model>","query":"q","documents":["a","b"]}' },
  { id: "models", method: "GET", path: "/v1/models" },
];

/** Rust 类型 → 占位值（仅用于 curl 示例的请求体）。 */
function placeholder(type: string): unknown {
  if (type.startsWith("Option<")) return null;
  if (type.startsWith("Vec<")) return [];
  if (type === "bool") return false;
  if (/^[iuf]\d+$|^[iu]size$/.test(type)) return 0;
  if (type === "String" || type === "&str") return "";
  return {};
}

export interface CurlOpts { port: number | null; hasToken: boolean }

function head({ port, hasToken }: CurlOpts, method: string, path: string, authPlaceholder: string) {
  const url = `http://127.0.0.1:${port ?? "<port>"}${path}`;
  const auth = hasToken ? ` -H 'Authorization: Bearer ${authPlaceholder}'` : "";
  return `curl -X ${method} '${url}'${auth}`;
}

export function curlForCommand(name: string, params: { name: string; type: string }[], o: CurlOpts): string {
  const body = JSON.stringify(Object.fromEntries(params.map((p) => [p.name, placeholder(p.type)])));
  return `${head(o, "POST", `/api/rpc/${name}`, "<api-credential>")} -H 'Content-Type: application/json' -d '${body}'`;
}

/** Local API / 转发接口：鉴权恒为分组名 Bearer，与管理接口凭据无关。 */
export function curlForStatic(e: StaticEndpoint, port: number | null): string {
  const base = head({ port, hasToken: true }, e.method, e.path, "<group-name>");
  return e.body ? `${base} -H 'Content-Type: application/json' -d '${e.body}'` : base;
}
