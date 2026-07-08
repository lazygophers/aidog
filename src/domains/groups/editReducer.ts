import type { GroupDetail, ModelMapping, EnvVar, RoutingMode, Platform } from "../../services/api";

/** 分组编辑表单态（原 8 个 useState 合并为单 reducer，减少分散 setState） */
export interface EditState {
  target: GroupDetail | null;
  name: string;
  mode: RoutingMode;
  platformIds: number[];
  mappings: ModelMapping[];
  envVars: EnvVar[];
  reqTimeout: number;
  connTimeout: number;
  maxRetries: number;
}

/**
 * 新建 group 默认预填隐私 env（禁遥测/增长/反馈）。
 * 仅「新建」分支使用此初值；编辑已有 group 走 "open" action 覆盖为 group.env_vars，不影响用户已存配置。
 * per-group 可改可删（不强注入）—— sync_settings.rs 注入逻辑不变。
 */
const PRIVACY_DEFAULT_ENV_VARS: EnvVar[] = [
  { key: "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", value: "1" },
  { key: "CLAUDE_CODE_ENABLE_TELEMETRY", value: "0" },
  { key: "CLAUDE_CODE_ENHANCED_TELEMETRY_BETA", value: "0" },
  { key: "CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY", value: "1" },
  { key: "CLAUDE_CODE_BYOC_ENABLE_DATADOG", value: "0" },
  { key: "CLAUDE_CODE_PROPAGATE_TRACEPARENT", value: "0" },
  { key: "DISABLE_GROWTHBOOK", value: "1" },
  { key: "CLAUDE_CODE_ATTRIBUTION_HEADER", value: "0" },
  { key: "DISABLE_INSTALLATION_CHECKS", value: "1" },
];

export const EMPTY_EDIT: EditState = {
  target: null,
  name: "",
  mode: "failover",
  platformIds: [],
  mappings: [],
  envVars: PRIVACY_DEFAULT_ENV_VARS,
  reqTimeout: 0,
  connTimeout: 0,
  maxRetries: 10,
};

export type EditAction =
  | { type: "open"; detail: GroupDetail }
  | { type: "reset" }
  | { type: "patch"; patch: Partial<EditState> };

export function editReducer(state: EditState, action: EditAction): EditState {
  switch (action.type) {
    case "open":
      return {
        target: action.detail,
        name: action.detail.group.name,
        mode: action.detail.group.routing_mode,
        platformIds: action.detail.platforms.map(gp => gp.platform.id),
        mappings: action.detail.model_mappings.map(m => ({
          source_model: m.source_model,
          target_platform_id: m.target_platform_id,
          target_model: m.target_model,
          request_timeout_secs: m.request_timeout_secs,
          connect_timeout_secs: m.connect_timeout_secs,
        })),
        envVars: action.detail.group.env_vars.map(ev => ({ key: ev.key, value: ev.value })),
        reqTimeout: action.detail.group.request_timeout_secs,
        connTimeout: action.detail.group.connect_timeout_secs,
        maxRetries: action.detail.group.max_retries,
      };
    case "reset":
      return EMPTY_EDIT;
    case "patch":
      return { ...state, ...action.patch };
  }
}

/** 不可变 upsert：按 id 替换或追加平台（保引用稳定，命中则只换该项）。 */
export function upsertPlatformInto(prev: Platform[], plat: Platform): Platform[] {
  const idx = prev.findIndex(p => p.id === plat.id);
  if (idx === -1) return [...prev, plat];
  const next = prev.slice();
  next[idx] = plat;
  return next;
}
