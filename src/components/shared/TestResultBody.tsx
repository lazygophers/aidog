// ── TestResultBody ──
// 测试响应正文（耗时已由调用方分离展示）的解析渲染：
//   - 先 JSON.parse，匹配已知结构（error / usage / message / choices）→ 结构化 key-value 视图。
//   - 解析失败或未知结构 → 回退原始文本（pre-wrap，不崩）。
// 供 ModelTestPanel / PlatformCard 最近测试详情统一消费，禁页内重复造解析逻辑。

import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";

/** 解析结果判别：known = 命中已知结构（渲染结构化）；raw = 回退原文。 */
export type ParsedTestBody =
  | { kind: "known"; rows: Array<{ label: string; value: string }> }
  | { kind: "raw"; text: string };

/** 安全取字符串：基础类型转字符串，对象 / 数组 JSON 序列化。 */
function toDisplay(v: unknown): string {
  if (v == null) return "";
  if (typeof v === "string") return v;
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  try {
    return JSON.stringify(v);
  } catch {
    return "";
  }
}

/**
 * 解析测试响应正文。t 通过参数注入（仿函数纯函数模式，禁直接 import 全局实例）。
 * 已知结构识别（按出现取并集，命中任一即 known）：
 *   - error 体（{ error: {...} } 或 { error: "..." }）：拆出 message / type / code。
 *   - usage（input/output tokens）。
 *   - message / choices 文本（Anthropic content / OpenAI choices）。
 */
export function parseTestBody(raw: string, t: TFunction): ParsedTestBody {
  const text = raw.trim();
  if (!text) return { kind: "raw", text: "" };

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return { kind: "raw", text };
  }
  if (parsed == null || typeof parsed !== "object") {
    return { kind: "raw", text };
  }

  const obj = parsed as Record<string, unknown>;
  const rows: Array<{ label: string; value: string }> = [];

  // error 体
  const err = obj.error;
  if (err != null) {
    if (typeof err === "object") {
      const e = err as Record<string, unknown>;
      const msg = toDisplay(e.message);
      if (msg) rows.push({ label: t("testBody.errorMessage", "错误信息"), value: msg });
      const type = toDisplay(e.type);
      if (type) rows.push({ label: t("testBody.errorType", "错误类型"), value: type });
      const code = toDisplay(e.code);
      if (code) rows.push({ label: t("testBody.errorCode", "错误码"), value: code });
      // error 对象存在但无可识别子字段 → 整体序列化兜底
      if (!msg && !type && !code) {
        rows.push({ label: t("testBody.error", "错误"), value: toDisplay(err) });
      }
    } else {
      rows.push({ label: t("testBody.error", "错误"), value: toDisplay(err) });
    }
  }

  // usage（Anthropic: input_tokens/output_tokens；OpenAI: prompt_tokens/completion_tokens）
  const usage = obj.usage;
  if (usage != null && typeof usage === "object") {
    const u = usage as Record<string, unknown>;
    const input = toDisplay(u.input_tokens ?? u.prompt_tokens);
    if (input) rows.push({ label: t("testBody.inputTokens", "输入 tokens"), value: input });
    const output = toDisplay(u.output_tokens ?? u.completion_tokens);
    if (output) rows.push({ label: t("testBody.outputTokens", "输出 tokens"), value: output });
  }

  // model
  const model = toDisplay(obj.model);
  if (model) rows.push({ label: t("testBody.model", "模型"), value: model });

  // 文本内容：Anthropic content[].text / OpenAI choices[].message.content
  const content = extractContentText(obj);
  if (content) rows.push({ label: t("testBody.content", "响应内容"), value: content });

  if (rows.length > 0) return { kind: "known", rows };
  return { kind: "raw", text };
}

/** 从 Anthropic content 数组或 OpenAI choices 数组抽取文本内容（取第一段非空）。 */
function extractContentText(obj: Record<string, unknown>): string {
  // Anthropic: content: [{ type: "text", text: "..." }]
  const content = obj.content;
  if (Array.isArray(content)) {
    for (const block of content) {
      if (block != null && typeof block === "object") {
        const txt = toDisplay((block as Record<string, unknown>).text);
        if (txt) return txt;
      }
    }
  }
  // OpenAI: choices: [{ message: { content: "..." } }]
  const choices = obj.choices;
  if (Array.isArray(choices)) {
    for (const choice of choices) {
      if (choice != null && typeof choice === "object") {
        const message = (choice as Record<string, unknown>).message;
        if (message != null && typeof message === "object") {
          const txt = toDisplay((message as Record<string, unknown>).content);
          if (txt) return txt;
        }
      }
    }
  }
  return "";
}

export interface TestResultBodyProps {
  /** 测试响应正文原文（耗时由调用方在外部分离展示）。 */
  body: string;
}

export function TestResultBody({ body }: TestResultBodyProps) {
  const { t } = useTranslation();
  const parsed = parseTestBody(body, t);

  if (parsed.kind === "raw") {
    if (!parsed.text) return null;
    return (
      <div
        style={{
          fontSize: 11,
          color: "var(--text-secondary)",
          marginTop: 4,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
          fontFamily: "var(--font-mono, monospace)",
        }}
      >
        {parsed.text}
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2, marginTop: 4 }}>
      {parsed.rows.map((r, i) => (
        <div key={i} style={{ display: "flex", gap: 6, fontSize: 11, lineHeight: 1.45 }}>
          <span style={{ color: "var(--text-tertiary)", fontWeight: 600, flexShrink: 0 }}>{r.label}</span>
          <span style={{ color: "var(--text-secondary)", wordBreak: "break-word", whiteSpace: "pre-wrap" }}>
            {r.value}
          </span>
        </div>
      ))}
    </div>
  );
}
