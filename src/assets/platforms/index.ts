/**
 * Platform logo resolver: maps Protocol enum values to SVG assets.
 *
 * SVG filenames match Protocol values (e.g. anthropic.svg → "anthropic").
 * Aliases allow multiple Protocol values to share one SVG.
 */

import type { Protocol, Platform } from "../../services/api";

// ── Aliases: secondary protocol → primary SVG filename ──────
const ALIASES: Partial<Record<Protocol, string>> = {
  openai_responses: "openai",
  openai_completions: "openai",
  glm_en: "glm",
  glm_coding_en: "glm_coding",
  bailian_en: "bailian",
  bailian_coding_en: "bailian_coding",
  kimi_en: "kimi",
  xiaomi_mimo_coding_en: "xiaomi_mimo_coding",
  sensenova_en: "sensenova",
  bailian_coding: "bailian",
};

// ── Build Protocol → URL lookup ──────────────────────────────
// 显式静态 import（取代 Vite 专属的 import.meta.glob，票 I14）：
// 默认导入在 Vite 返回 URL 字符串；Next/Turbopack 由 next.config.ts 的
// `turbopack.rules["*.svg"] = { type: "asset" }` 提供同一行为。
import anthropicSvg from "./anthropic.svg";
import bailianSvg from "./bailian.svg";
import claudeCodeSvg from "./claude_code.svg";
import clineSvg from "./cline.svg";
import doubaoSvg from "./doubao.svg";
import exaSvg from "./exa.svg";
import geminiSvg from "./gemini.svg";
import githubCopilotSvg from "./github-copilot.svg";
import glmSvg from "./glm.svg";
import huggingfaceSvg from "./huggingface.svg";
import kimiSvg from "./kimi.svg";
import metaSvg from "./meta.svg";
import ollamaSvg from "./ollama.svg";
import openaiSvg from "./openai.svg";
import piSvg from "./pi.svg";
import sensenovaSvg from "./sensenova.svg";

const SVG_URLS: Record<string, string> = {
  anthropic: anthropicSvg,
  bailian: bailianSvg,
  claude_code: claudeCodeSvg,
  cline: clineSvg,
  doubao: doubaoSvg,
  exa: exaSvg,
  gemini: geminiSvg,
  "github-copilot": githubCopilotSvg,
  glm: glmSvg,
  huggingface: huggingfaceSvg,
  kimi: kimiSvg,
  meta: metaSvg,
  ollama: ollamaSvg,
  openai: openaiSvg,
  pi: piSvg,
  sensenova: sensenovaSvg,
};

/**
 * Get the logo URL for a platform type.
 * Returns undefined if no logo is available.
 */
export function getPlatformLogo(protocol: Protocol): string | undefined {
  // 1. Direct match (filename === protocol)
  const direct = SVG_URLS[protocol];
  if (direct) return direct;
  // 2. Alias match
  const alias = ALIASES[protocol];
  if (alias) return SVG_URLS[alias];
  return undefined;
}

/** Check whether a logo exists for the given protocol */
export function hasPlatformLogo(protocol: Protocol): boolean {
  return getPlatformLogo(protocol) !== undefined;
}

/** 从 base_url 提取 origin，用于 favicon 回退 */
export function extractOrigin(baseUrl: string): string | null {
  try {
    return new URL(baseUrl).origin;
  } catch { return null; }
}

/** 从 platform 的 endpoints/base_url 推导 favicon URL */
export function getFaviconUrl(p: Platform): string | null {
  const eps = p.endpoints ?? [];
  const baseUrl = eps[0]?.base_url || p.base_url;
  const origin = extractOrigin(baseUrl);
  return origin ? `${origin}/favicon.ico` : null;
}
