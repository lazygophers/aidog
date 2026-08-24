// mwDsl.ts — 中间件条件树 DSL（票 05）。
// 树 JSON 是唯一存储真值；DSL 只是前端视图。语法（S 表达式风格，括号内换行随意）：
//
//   ALL( 叶子 叶子 ... )      ANY( 叶子 叶子 ... )      叶子
//   叶子 := target[.field] OP "pattern"
//   OP := contains | regex | exact
//
// 往返保证：tree → dsl → tree 与原树深度相等（叶子顺序保持）。

import type { ConditionNode, ConditionLeaf } from "../services/api";

const TARGETS = ["request_body", "request_headers", "response_body", "response_headers", "status", "model"] as const;
const OPS = ["contains", "regex", "exact"] as const;

/** 叶子 → DSL（pattern 用 JSON 字符串转义，可含任意字符）。 */
export function leafToDsl(l: ConditionLeaf): string {
  const field = l.field ? `.${l.field}` : "";
  return `${l.target}${field} ${l.match_type} ${JSON.stringify(l.pattern)}`;
}

/** 树 → DSL（递归；叶子直接输出）。 */
export function treeToDsl(node: ConditionNode): string {
  if (node.kind === "leaf") return leafToDsl(node);
  const inner = node.children.map(treeToDsl);
  if (inner.length === 1) return inner[0];
  return `${node.kind.toUpperCase()}(\n  ${inner.join("\n  ")}\n)`;
}

/** 词法单元。 */
type Tok =
  | { t: "word"; v: string }
  | { t: "str"; v: string }
  | { t: "(" }
  | { t: ")" }
  | { t: "eof" };

class Lexer {
  private i = 0;
  constructor(private readonly s: string) {}
  private pos(): number { return this.i; }
  next(): { tok: Tok; pos: number } {
    while (this.i < this.s.length && /\s/.test(this.s[this.i])) this.i++;
    const pos = this.pos();
    if (this.i >= this.s.length) return { tok: { t: "eof" }, pos };
    const c = this.s[this.i];
    if (c === "(") { this.i++; return { tok: { t: "(" }, pos }; }
    if (c === ")") { this.i++; return { tok: { t: ")" }, pos }; }
    if (c === '"') {
      // JSON 字符串
      let j = ++this.i;
      while (j < this.s.length && this.s[j] !== '"') {
        if (this.s[j] === "\\") j++;
        j++;
      }
      if (j >= this.s.length) throw err(pos, "未闭合的字符串");
      let v: string;
      try {
        v = JSON.parse(this.s.slice(pos, j + 1));
      } catch {
        throw err(pos, "非法字符串字面量");
      }
      this.i = j + 1;
      return { tok: { t: "str", v }, pos };
    }
    const m = /^[A-Za-z0-9_.\-]+/.exec(this.s.slice(this.i));
    if (!m) throw err(pos, `非法字符 '${c}'`);
    this.i += m[0].length;
    return { tok: { t: "word", v: m[0] }, pos };
  }
}

function err(pos: number, msg: string): Error {
  const e = new Error(`位置 ${pos + 1}: ${msg}`) as Error & { dslPos?: number };
  e.dslPos = pos;
  return e;
}

class Parser {
  private la: { tok: Tok; pos: number } | null = null;
  constructor(private readonly lx: Lexer) {}
  peek(): { tok: Tok; pos: number } {
    if (!this.la) this.la = this.lx.next();
    return this.la;
  }
  private take(): Tok {
    const { tok } = this.peek();
    this.la = null;
    return tok;
  }
  parseExpr(): ConditionNode {
    const { tok, pos } = this.peek();
    if (tok.t === "word" && (tok.v === "ALL" || tok.v === "ANY")) {
      this.take();
      if (this.take().t !== "(") throw err(pos, `${tok.v} 后缺 '('`);
      const children: ConditionNode[] = [];
      while (true) {
        const p = this.peek();
        if (p.tok.t === "eof") throw err(p.pos, "缺 ')'");
        if (p.tok.t === ")") { this.take(); break; }
        children.push(this.parseExpr());
      }
      if (children.length === 0) throw err(pos, `${tok.v}() 至少需要一个子条件`);
      return { kind: tok.v.toLowerCase() as "all" | "any", children };
    }
    return this.parseLeaf();
  }
  parseLeaf(): ConditionNode {
    const { tok, pos } = this.peek();
    if (tok.t !== "word") throw err(pos, "期望条件（target op \"pattern\"）");
    this.take();
    // target[.field]
    const segs = tok.v.split(".");
    const target = segs[0];
    const field = segs.slice(1).join(".");
    if (!(TARGETS as readonly string[]).includes(target)) {
      throw err(pos, `未知 target '${target}'（可选: ${TARGETS.join(" / ")}）`);
    }
    const opTok = this.take();
    if (opTok.t !== "word" || !(OPS as readonly string[]).includes(opTok.v)) {
      throw err(pos, `期望算子（${OPS.join(" / ")}）`);
    }
    const patTok = this.take();
    if (patTok.t !== "str") throw err(pos, "期望双引号 pattern");
    const leaf = {
      target: target as ConditionLeaf["target"],
      field,
      match_type: opTok.v as ConditionLeaf["match_type"],
      pattern: patTok.v,
    };
    return { kind: "leaf" as const, ...leaf };
  }
}

/** DSL → 树。解析失败抛带 dslPos 的 Error（前端定位提示）。 */
export function parseDsl(src: string): ConditionNode {
  const parser = new Parser(new Lexer(src));
  const tree = parser.parseExpr();
  const { tok, pos } = parser.peek();
  if (tok.t !== "eof") throw err(pos, "条件后有多余内容");
  return tree;
}

/** DSL 解析错误的位置（无则 -1）。供编辑器错误定位。 */
export function dslErrorPos(e: unknown): number {
  return (e as { dslPos?: number })?.dslPos ?? -1;
}
