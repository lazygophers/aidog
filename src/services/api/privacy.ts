// privacy.ts — 隐私脱敏设置与试一试预览（票 04）。
// per_kind 键值与 Rust `PiiKind::as_str` 词表逐字一致。

import { invoke } from "../transport";

export interface PrivacyScrubSettings {
  enabled: boolean;
  secrets: boolean;
  personal: boolean;
  model_enabled: boolean;
  model_endpoint: string;
  per_kind: Record<string, boolean>;
}

export interface PrivacyScrubPreview {
  original: string;
  scrubbed: string;
  hits: number;
}

export const privacyApi = {
  get: () => invoke<PrivacyScrubSettings>("privacy_scrub_get"),
  /** 整体替换保存；后端刷新代理设置缓存即时生效。 */
  set: (settings: PrivacyScrubSettings) => invoke<void>("privacy_scrub_set", { settings }),
  /** 试一试预览：走生产 scrub 路径（规则 + 本地模型）。 */
  preview: (text: string) => invoke<PrivacyScrubPreview>("privacy_scrub_preview", { text }),
};
