import type { TFunction } from "i18next";
import { setUiExtra } from "../../services/api/ui_extra";
import { configApi } from "../../services/api/settings";

/**
 * 分组的 pi 线路协议。取值与 pi `models.json` 的 `api` 字段一致，
 * 也与 Rust `gateway::pi::PiApi` 一一对应。
 *
 * 版本后缀由后端按协议推导（anthropic 用裸根地址、openai 带 `/v1`、google 带 `/v1beta`），
 * 用户在 UI 上无处输入 URL。
 */
export const PI_APIS = [
  "anthropic-messages",
  "openai-completions",
  "openai-responses",
  "google-generative-ai",
] as const;

export type PiApi = (typeof PI_APIS)[number];

/** 老分组无此配置时的取值。与 Rust `PiApi::default()` 一致。 */
export const PI_API_DEFAULT: PiApi = "anthropic-messages";

/** 从 group.extra JSON 读协议；缺失 / 非法 JSON / 未知值一律回落默认。 */
export function parseGroupPiApi(extra: string): PiApi {
  try {
    const v = JSON.parse(extra || "{}").pi_api;
    return PI_APIS.includes(v) ? v : PI_API_DEFAULT;
  } catch {
    return PI_API_DEFAULT;
  }
}

export function piApiLabel(t: TFunction, api: PiApi): string {
  const fallback: Record<PiApi, string> = {
    "anthropic-messages": "Anthropic Messages",
    "openai-completions": "OpenAI Chat Completions",
    "openai-responses": "OpenAI Responses",
    "google-generative-ai": "Google Generative AI",
  };
  return t(`group.piApi.${api}`, fallback[api]);
}

/** 写协议到 group.extra，并立刻重生成 pi 配置（否则改动要等下一次同步才落盘）。 */
export async function setGroupPiApi(groupId: number, api: PiApi): Promise<void> {
  await setUiExtra("group", groupId, "pi_api", api);
  await configApi.syncGroupSettings();
}
