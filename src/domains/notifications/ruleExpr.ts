// 规则表达式可视化 ⇄ 文本双向转换（通知体系票 05）。
// 文本语法真值源：Rust aidog_db::models::notification_rules 的受限 DSL
// （字段比较 + ==/!=/contains + &&/|| + 括号）。本模块的解析器是该 DSL 的 TS 镜像，
// 只额外负责「能否安全转成可视化」的判定。
//
// 可视化模型 = 单层同构联结：一组比较项以统一 &&（或 ||）连接。
// 不可转换：括号分组 / && 与 || 混用（优先级语义无法用单层表达）/ 语法错。
// 转换失败时保留原文不丢条件（UI 停留文本模式 + 降级提示）。

export type CmpOp = "==" | "!=" | "contains";
export interface Term {
  field: string;
  op: CmpOp;
  value: string;
}
export interface Visual {
  join: "&&" | "||";
  terms: Term[];
}

/** 可视化 → 规范式文本。值一律双引号包裹（无转义语义，与 Rust lexer 对齐）。 */
export function toText(v: Visual): string {
  return v.terms
    .map(t => `${t.field} ${t.op} ${JSON.stringify(t.value)}`)
    .join(` ${v.join} `);
}

export type ParseResult =
  | { ok: true; visual: Visual }
  | { ok: false; reason: string };

type Tok = { t: "ident" | "str"; s: string } | { t: "op"; s: string };

function lex(src: string): Tok[] | string {
  const toks: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) { i++; continue; }
    if (c === '"') {
      const end = src.indexOf('"', i + 1);
      if (end < 0) return "字符串缺少收尾双引号";
      toks.push({ t: "str", s: src.slice(i + 1, end) });
      i = end + 1;
      continue;
    }
    if (src.startsWith("==", i) || src.startsWith("!=", i) || src.startsWith("&&", i) || src.startsWith("||", i)) {
      toks.push({ t: "op", s: src.slice(i, i + 2) });
      i += 2;
      continue;
    }
    if (c === "(" || c === ")") return "含括号分组——可视化不支持，请留在文本模式";
    if (src.startsWith("contains", i) && !/[\w.\-]/.test(src[i + 8] ?? "")) {
      toks.push({ t: "op", s: "contains" });
      i += 8;
      continue;
    }
    // 裸词（字母/数字/_/./-/中文）
    const m = /^[\w.\-一-鿿]+/.exec(src.slice(i));
    if (!m) return `第 ${i} 字符附近无法识别`;
    toks.push({ t: "ident", s: m[0] });
    i += m[0].length;
  }
  return toks;
}

/** 文本 → 可视化。语法错 / 括号 / &&与|| 混用 → ok:false（reason 供降级提示）。 */
export function parseToVisual(src: string): ParseResult {
  const text = src.trim();
  if (!text) return { ok: false, reason: "表达式为空" };
  const toks = lex(text);
  if (typeof toks === "string") return { ok: false, reason: toks };

  // 结构：term (joinOp term)*，term = ident cmpOp value
  const terms: Term[] = [];
  let join: "&&" | "||" | null = null;
  let i = 0;
  const next = () => toks[i++];
  while (i < toks.length) {
    const f = next();
    if (!f || f.t !== "ident") return { ok: false, reason: `期望字段名，收到 ${JSON.stringify(f?.s ?? "结尾")}` };
    const op = next();
    if (!op || op.t !== "op" || op.s === "&&" || op.s === "||") {
      return { ok: false, reason: "期望比较符（== != contains）" };
    }
    const val = next();
    if (!val || val.t === "op") return { ok: false, reason: "期望值（字符串/数字/裸词）" };
    terms.push({ field: f.s, op: op.s as CmpOp, value: val.s });
    if (i >= toks.length) break;
    const j = next();
    if (!j || j.t !== "op" || (j.s !== "&&" && j.s !== "||")) {
      return { ok: false, reason: "期望 && 或 ||" };
    }
    if (join === null) join = j.s as "&&" | "||";
    else if (join !== j.s) {
      return { ok: false, reason: "&& 与 || 混用无法转可视化（优先级语义丢失）——请留在文本模式" };
    }
  }
  if (terms.length === 0) return { ok: false, reason: "无有效条件" };
  return { ok: true, visual: { join: join ?? "&&", terms } };
}

/** 往返等价：toText → parseToVisual 结构不变（测试与 UI 切换自检用）。 */
export function roundTrips(v: Visual): boolean {
  const r = parseToVisual(toText(v));
  return r.ok && JSON.stringify(r.visual) === JSON.stringify(v);
}
