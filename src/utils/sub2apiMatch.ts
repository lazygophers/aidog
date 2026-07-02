// sub2api account → aidog Platform 映射（纯函数）。
//
// 比 ccswitchMatch.ts 简单：sub2api 的 `platform` 值（anthropic/openai/gemini）
// 天然与 aidog Protocol serde 值对齐（均小写，见 models.rs:5-16），直映射即可，
// 未识别兜底 openai + 前端预览行下拉手改（prd §4.1 评审决策 ②）。
//
// endpoints 构造同 ccswitchMatch.ts buildMatch：取 getDefaultEndpoints(protocol)
// 骨架，若提供 base_url 则覆盖同协议 endpoint，否则保留 preset 默认（prd §4.2）。
// preset 单一事实源住前端（记忆 aidog-add-platform-skill 反直觉点 1）。

import type { Protocol, PlatformEndpoint } from "../services/api";
import { getDefaultEndpoints } from "../domains/platforms";
import type { Sub2ApiAccount } from "../services/api";

/** sub2api platform 值 → aidog Protocol 直映射表（小写键）。 */
const PLATFORM_MAP: Record<string, Protocol> = {
  anthropic: "anthropic",
  openai: "openai",
  gemini: "gemini",
};

export interface Sub2ApiMatch {
  /** 映射出的 platform_type（Protocol 枚举值）。 */
  protocol: Protocol;
  /** false = 未识别（已兜底 openai），前端标记徽标 + 允许下拉手改。 */
  recognized: boolean;
}

/**
 * sub2api `platform` 字段 → aidog Protocol。
 * 归一化 lowercase + trim 后直映射；未识别返回 openai + recognized:false。
 */
export function mapPlatformToProtocol(platform: string): Sub2ApiMatch {
  const key = platform.trim().toLowerCase();
  const protocol = PLATFORM_MAP[key];
  if (protocol) {
    return { protocol, recognized: true };
  }
  return { protocol: "openai", recognized: false };
}

/** 构造 endpoints：preset 骨架 + 实际 base_url 覆盖同协议 endpoint。 */
function buildEndpoints(protocol: Protocol, baseUrl: string): PlatformEndpoint[] {
  const skeleton = getDefaultEndpoints(protocol);
  if (!baseUrl) return skeleton;
  return skeleton.map((ep) =>
    ep.protocol === protocol ? { ...ep, base_url: baseUrl } : ep,
  );
}

/**
 * 把 sub2api account + 用户选定 protocol 转换为 aidog Platform JSON
 * （同 ccProviderToPlatformJson 形态），喂给 sub2api_import → apply::apply。
 *
 * @param account 后端解析的账号 DTO
 * @param protocolOverride 用户在预览行下拉手改的 protocol（缺省走 platform 映射）
 */
export function sub2apiAccountToPlatformJson(
  account: Sub2ApiAccount,
  protocolOverride?: Protocol,
): Record<string, unknown> {
  const protocol =
    protocolOverride ?? mapPlatformToProtocol(account.platform).protocol;
  // base_url 缺失 → 取 preset 默认（buildEndpoints 内的 getDefaultEndpoints 骨架默认）。
  const providedBaseUrl = account.baseUrl ?? "";
  const endpoints = buildEndpoints(protocol, providedBaseUrl);
  // platform.base_url 顶层字段：缺失时取同协议 endpoint 的默认 base_url（预设回退）。
  const baseUrl =
    providedBaseUrl ||
    endpoints.find((ep) => ep.protocol === protocol)?.base_url ||
    endpoints[0]?.base_url ||
    "";

  return {
    name: account.name,
    platform_type: protocol,
    base_url: baseUrl,
    api_key: account.apiKey ?? "",
    extra: "",
    models: {},
    available_models: [],
    endpoints,
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
