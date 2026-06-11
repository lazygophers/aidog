/**
 * Platform logo resolver: maps Protocol enum values to SVG assets.
 *
 * SVG filenames match Protocol values (e.g. anthropic.svg → "anthropic").
 * Aliases allow multiple Protocol values to share one SVG.
 */

import type { Protocol } from "../../services/api";

// ── Aliases: secondary protocol → primary SVG filename ──────
const ALIASES: Partial<Record<Protocol, string>> = {
  openai_responses: "openai",
  openai_completions: "openai",
  glm_en: "glm",
  doubao_seed: "doubao",
  bailian_coding: "bailian",
};

// ── Build Protocol → URL lookup ──────────────────────────────
const svgModules = import.meta.glob("./*.svg", { eager: true, as: "url" });

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
