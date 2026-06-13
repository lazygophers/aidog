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
  doubao_seed: "doubao",
  bailian_coding: "bailian",
};

// ── Build Protocol → URL lookup ──────────────────────────────
const svgModules = import.meta.glob("./*.svg", { eager: true, query: "?url", import: "default" });

// filename → URL map (strip "./" prefix and ".svg" suffix)
const fileMap = new Map<string, string>();
for (const [path, url] of Object.entries(svgModules)) {
  if (typeof url === "string") {
    const name = path.replace(/^\.\//, "").replace(/\.svg$/, "");
    fileMap.set(name, url);
  }
}

/**
 * Get the logo URL for a platform type.
 * Returns undefined if no logo is available.
 */
export function getPlatformLogo(protocol: Protocol): string | undefined {
  // 1. Direct match (filename === protocol)
  const direct = fileMap.get(protocol);
  if (direct) return direct;
  // 2. Alias match
  const alias = ALIASES[protocol];
  if (alias) return fileMap.get(alias);
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
