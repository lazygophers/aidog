// cc-switch provider → aidog Platform 匹配回退链（纯函数）。
//
// **禁在 Rust 重复实现 preset 匹配**：preset 单一事实源是 domains/platforms
// 的 buildProtocolsFromPresets + getDefaultEndpoints，此文件复用之（记忆
// `aidog-add-platform-skill` 反直觉点 1）。后端 ccswitch.rs 只做原始数据
// 读取 + DTO 透传。
//
// 回退链 4 步（prd「base_url → 平台匹配 + 回退链路设计」）：
//   1. preset 关键词匹配（matchPlatform）
//   2. base_url host 匹配（getDefaultEndpoints 各 preset 内置 base_url host）
//   3. codex wire_api / claude 协议回退
//   4. guessProtocol URL 启发式兜底

import type { Protocol, PlatformEndpoint, PlatformModels } from "../services/api";
import type { CcProvider } from "../services/api";
import { getDefaultEndpoints, buildProtocolsFromPresets, type ProtocolOption } from "../domains/platforms";
import {
  normalizeForMatch,
  guessProtocol,
  matchPlatform,
  type PastePresetRef,
} from "./platformPaste";

export type MatchedBy = "preset_keyword" | "base_url_host" | "protocol_fallback";

export interface CcMatchResult {
  /** 命中的 platform_type（Protocol 枚举值）。 */
  protocol: Protocol;
  codingPlan?: boolean;
  matchedBy: MatchedBy;
  /** getDefaultEndpoints 骨架 + 实际 base_url 覆盖同协议 endpoint。 */
  endpoints: PlatformEndpoint[];
  /** 命中的 preset label（展示用）；回退时 undefined。 */
  matchedLabel?: string;
}

/** 把 ProtocolOption 列表转换为 platformPaste.ts 需要的 PastePresetRef。 */
function toPastePresets(protocols: ProtocolOption[]): PastePresetRef[] {
  return protocols.map((p) => ({
    value: p.value,
    label: p.label,
    keywords: p.keywords,
    // hosts 由 buildProtocolsFromPresets 派生自 endpoints 内联写入 ProtocolOption.hosts，
    // 必须透传，否则 matchPlatform 优先级1（host 子串匹配）对所有 preset 失效。
    hosts: p.hosts,
    codingPlan: p.codingPlan,
    // 机制 B 数据源，必须透传，否则纯 token 粘贴的 coding plan 升级（如 tp- 前缀）失效。
    codingKeyPrefixes: p.codingKeyPrefixes,
  }));
}

/** 提取 url 的 host（小写）。无效 url 返回空串。 */
function hostOf(url: string): string {
  try {
    const u = new URL(url);
    return u.hostname.toLowerCase();
  } catch {
    // 容错：手工提取 `https://host/...` 的 host。
    const m = url.match(/^https?:\/\/([^/]+)/i);
    return m ? m[1].toLowerCase() : "";
  }
}

/**
 * 步骤 2：base_url host 匹配。
 * 对每个 preset 取 getDefaultEndpoints(protocol) 的默认 base_url host，
 * 与 cc-switch base_url host 做子串匹配。
 */
async function matchByBaseUrlHost(
  baseUrl: string,
  protocols: ProtocolOption[],
): Promise<{ protocol: Protocol; codingPlan?: boolean; label: string } | null> {
  const target = normalizeForMatch(hostOf(baseUrl));
  if (!target) return null;
  for (const p of protocols) {
    const eps = await getDefaultEndpoints(p.value, p.codingPlan);
    for (const ep of eps) {
      const h = normalizeForMatch(hostOf(ep.base_url));
      // 双向子串匹配（preset host 可能是子域，或反之）。
      if (h && (target.includes(h) || h.includes(target)) && h.length >= 4) {
        return { protocol: p.value, codingPlan: p.codingPlan, label: p.label };
      }
    }
  }
  return null;
}

/** 构造命中结果：取 preset 骨架 endpoints，用实际 base_url 覆盖同协议 endpoint。 */
async function buildMatch(
  protocol: Protocol,
  codingPlan: boolean | undefined,
  matchedBy: MatchedBy,
  baseUrl: string,
  label?: string,
): Promise<CcMatchResult> {
  const skeleton = await getDefaultEndpoints(protocol, codingPlan);
  const endpoints = baseUrl
    ? skeleton.map((ep) =>
        // 用实际 base_url 覆盖同协议 endpoint；不同协议保留骨架默认。
        ep.protocol === protocol ? { ...ep, base_url: baseUrl } : ep,
      )
    : skeleton;
  return {
    protocol,
    codingPlan,
    matchedBy,
    endpoints,
    matchedLabel: label,
  };
}

/** 步骤 3+4：协议回退（codex wire_api / claude 协议 / URL 启发式）。 */
function buildFallback(
  provider: CcProvider,
  baseUrl: string,
): CcMatchResult {
  if (provider.appType === "codex") {
    const wireApi = provider.codexConfigParsed?.wireApi;
    const endpointProtocol: Protocol =
      wireApi === "responses" ? "openai_responses" : "openai";
    const endpoint: PlatformEndpoint = {
      protocol: endpointProtocol,
      base_url: baseUrl,
      client_type: "codex_tui",
    };
    return {
      protocol: "openai",
      matchedBy: "protocol_fallback",
      endpoints: [endpoint],
    };
  }

  // claude 回退：先看 URL 启发式，再回退到 anthropic。
  const guessed = baseUrl ? guessProtocol(baseUrl) : "unknown";
  if (guessed === "openai") {
    // base_url 明显是 openai 类（如 NewAPI），但 provider 是 claude app_type。
    // 按用户原话「claude 类回退 anthropic 协议」——仍走 anthropic endpoint。
    return buildClaudeFallback(baseUrl);
  }
  return buildClaudeFallback(baseUrl);
}

function buildClaudeFallback(baseUrl: string): CcMatchResult {
  const endpoint: PlatformEndpoint = {
    protocol: "anthropic",
    base_url: baseUrl,
    client_type: "claude_code",
  };
  return {
    protocol: "anthropic",
    matchedBy: "protocol_fallback",
    endpoints: [endpoint],
  };
}

/**
 * 匹配主入口：cc-switch provider → aidog platform_type + endpoints。
 * @param provider 后端 DTO（已含 detected_base_url / codex_config_parsed）
 * @param protocols 可选 ProtocolOption[]（默认 buildProtocolsFromPresets 派生自 platform-presets.json）
 */
export async function matchCcProvider(
  provider: CcProvider,
  protocols?: ProtocolOption[],
): Promise<CcMatchResult> {
  const list = protocols ?? await buildProtocolsFromPresets();
  const baseUrl = provider.detectedBaseUrl ?? "";
  const text = `${provider.name} ${baseUrl}`;

  // 步骤 1：preset 关键词匹配（name + base_url 整体做 hay）。
  const hit = matchPlatform(text, toPastePresets(list));
  if (hit) {
    const preset = list.find(
      (p) => p.value === hit.value && p.label === hit.label,
    );
    return buildMatch(
      hit.value as Protocol,
      preset?.codingPlan,
      "preset_keyword",
      baseUrl,
      hit.label,
    );
  }

  // 步骤 2：base_url host 匹配。
  const hostHit = await matchByBaseUrlHost(baseUrl, list);
  if (hostHit) {
    return buildMatch(
      hostHit.protocol,
      hostHit.codingPlan,
      "base_url_host",
      baseUrl,
      hostHit.label,
    );
  }

  // 步骤 3 + 4：协议回退。
  return buildFallback(provider, baseUrl);
}

// ── 转换为 Platform JSON（喂给后端 apply::apply）──

/** 从 cc-switch provider 提取模型映射（D2）。 */
export function extractModels(provider: CcProvider): PlatformModels {
  if (provider.appType === "claude") {
    const env = (provider.settingsConfig as { env?: Record<string, string> }).env ?? {};
    const main = env.ANTHROPIC_MODEL;
    // 优先把 ANTHROPIC_MODEL 当主模型；若同时有 default slot 语义，归 default。
    return {
      default: main || undefined,
      haiku: env.ANTHROPIC_DEFAULT_HAIKU_MODEL || undefined,
      sonnet: env.ANTHROPIC_DEFAULT_SONNET_MODEL || undefined,
      opus: env.ANTHROPIC_DEFAULT_OPUS_MODEL || undefined,
    };
  }
  if (provider.appType === "codex") {
    const model = provider.codexConfigParsed?.model;
    return { default: model || undefined };
  }
  return {};
}

/** 选择性导入维度。 */
export interface CcImportDims {
  /** 平台类型 + endpoints（D1，总是隐含）。 */
  d1: boolean;
  /** 模型映射（D2）。 */
  d2: boolean;
  /** 密钥（D4）。 */
  d4: boolean;
}

export const DEFAULT_DIMS: CcImportDims = { d1: true, d2: true, d4: true };

/**
 * 把 cc-switch provider + 匹配结果转换为 aidog Platform JSON（同
 * collect.rs `serde_json::to_value(Platform)` 形态），喂给 ccswitch_import。
 */
export function ccProviderToPlatformJson(
  provider: CcProvider,
  match: CcMatchResult,
  dims: CcImportDims,
): Record<string, unknown> {
  const models = dims.d2 ? extractModels(provider) : {};
  const apiKey = dims.d4 ? provider.detectedApiKey ?? "" : "";
  return {
    name: provider.name,
    platform_type: match.protocol,
    base_url: provider.detectedBaseUrl ?? "",
    api_key: apiKey,
    extra: "",
    models,
    available_models: [],
    endpoints: match.endpoints,
    enabled: true,
    status: "enabled",
    auto_disabled_until: 0,
    auto_disable_strikes: 0,
    est_balance_remaining: 0,
    est_coding_plan: "",
    last_real_query_at: 0,
    estimate_count: 0,
    show_in_tray: false,
    tray_display: "",
    sort_order: 0,
    manual_budgets: "",
  };
}
