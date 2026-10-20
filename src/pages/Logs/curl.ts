// 票 passthrough-log-fixes/04：日志详情复制 curl 的纯生成逻辑。
// 鉴权头名单是 Rust `is_sensitive_log_header`（aidog_core/src/gateway/proxy/headers.rs
// 六头：authorization / api-key / x-api-key / x-goog-api-key / cookie / set-cookie）
// 的前端展示层镜像，单一真值在 Rust，改动须两处同步。

/** 鉴权/凭证头名单（小写）；命中后 curl 里值替换为 {key} 占位、scheme 保留。 */
export const SENSITIVE_AUTH_HEADER_NAMES: ReadonlySet<string> = new Set([
  "authorization",
  "api-key",
  "x-api-key",
  "x-goog-api-key",
  "cookie",
  "set-cookie",
]);

/** shell 单引号包裹，值内单引号 → '\'' */
function shellQuote(s: string): string {
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

/** 鉴权头值 → scheme 保留 + {key} 占位："Bearer sk-x" → "Bearer {key}"；裸 key → "{key}"。 */
function maskAuthValue(value: string): string {
  const i = value.indexOf(" ");
  return i > 0 ? `${value.slice(0, i)} {key}` : "{key}";
}

export interface BuildCurlInput {
  url: string;
  headers: unknown;
  /** 请求体；空串 / undefined 不带 --data */
  body?: unknown;
  /** proxy_log 未落 method，取不到默认 POST */
  method?: string;
}

/** 生成 `curl -X <method> '<url>' -H 'k: v' … --data '<body>'`；多行 JSON body 压成合法单行字面量。 */
export function buildCurl({ url, headers, body, method }: BuildCurlInput): string {
  const parts = [`curl -X ${method || "POST"} ${shellQuote(url)}`];
  if (headers && typeof headers === "object" && !Array.isArray(headers)) {
    for (const [k, v] of Object.entries(headers as Record<string, unknown>)) {
      let value = typeof v === "string" ? v : JSON.stringify(v);
      if (SENSITIVE_AUTH_HEADER_NAMES.has(k.toLowerCase())) value = maskAuthValue(value);
      parts.push(`-H ${shellQuote(`${k}: ${value}`)}`);
    }
  }
  if (body !== undefined && body !== null && body !== "") {
    let data = typeof body === "string" ? body : JSON.stringify(body);
    if (typeof body === "string") {
      // 多行 JSON 压成单行；非 JSON 串原样
      try {
        data = JSON.stringify(JSON.parse(body));
      } catch {
        /* keep raw */
      }
    }
    parts.push(`--data ${shellQuote(data)}`);
  }
  return parts.join(" \\\n  ");
}
