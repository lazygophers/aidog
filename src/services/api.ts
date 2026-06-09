import { invoke } from "@tauri-apps/api/core";

// ─── Types ─────────────────────────────────────────────────

export type Protocol = "anthropic" | "openai" | "glm" | "kimi";
export type RoutingMode = "load_balance" | "failover";

export interface Platform {
  id: string;
  name: string;
  protocol: Protocol;
  base_url: string;
  api_key: string;
  extra: string | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface Group {
  id: string;
  name: string;
  path: string;
  routing_mode: RoutingMode;
  created_at: string;
  updated_at: string;
}

export interface GroupPlatformDetail {
  platform: Platform;
  priority: number;
  weight: number;
}

export interface ModelMapping {
  id: string;
  group_id: string;
  source_model: string;
  target_platform_id: string;
  target_model: string;
}

export interface GroupDetail {
  group: Group;
  platforms: GroupPlatformDetail[];
  model_mappings: ModelMapping[];
}

export interface ProxySettings {
  port: number;
  autostart: boolean;
}

// ─── Platform API ──────────────────────────────────────────

export const platformApi = {
  create: (input: {
    name: string;
    protocol: Protocol;
    base_url: string;
    api_key: string;
    extra?: string;
  }) => invoke<Platform>("platform_create", { input }),

  list: () => invoke<Platform[]>("platform_list"),

  get: (id: string) => invoke<Platform | null>("platform_get", { id }),

  update: (input: {
    id: string;
    name?: string;
    protocol?: Protocol;
    base_url?: string;
    api_key?: string;
    extra?: string;
    enabled?: boolean;
  }) => invoke<Platform>("platform_update", { input }),

  delete: (id: string) => invoke<void>("platform_delete", { id }),
};

// ─── Group API ─────────────────────────────────────────────

export const groupApi = {
  create: (input: {
    name: string;
    path: string;
    routing_mode: RoutingMode;
  }) => invoke<Group>("group_create", { input }),

  list: () => invoke<Group[]>("group_list"),

  get: (id: string) => invoke<Group | null>("group_get", { id }),

  update: (input: {
    id: string;
    name?: string;
    path?: string;
    routing_mode?: RoutingMode;
  }) => invoke<Group>("group_update", { input }),

  delete: (id: string) => invoke<void>("group_delete", { id }),

  setPlatforms: (
    groupId: string,
    platforms: { platform_id: string; priority?: number; weight?: number }[]
  ) =>
    invoke<void>("group_set_platforms", {
      input: { group_id: groupId, platforms },
    }),

  getPlatforms: (groupId: string) =>
    invoke<GroupPlatformDetail[]>("group_get_platforms", { groupId }),
};

// ─── Model Mapping API ─────────────────────────────────────

export const mappingApi = {
  create: (input: {
    group_id: string;
    source_model: string;
    target_platform_id: string;
    target_model: string;
  }) => invoke<ModelMapping>("mapping_create", { input }),

  list: (groupId: string) =>
    invoke<ModelMapping[]>("mapping_list", { groupId }),

  update: (input: {
    id: string;
    source_model?: string;
    target_platform_id?: string;
    target_model?: string;
  }) => invoke<ModelMapping>("mapping_update", { input }),

  delete: (id: string) => invoke<void>("mapping_delete", { id }),
};

// ─── Aggregate API ─────────────────────────────────────────

export const groupDetailApi = {
  get: (id: string) =>
    invoke<GroupDetail | null>("group_detail", { id }),

  list: () => invoke<GroupDetail[]>("group_detail_list"),
};

// ─── Proxy API ─────────────────────────────────────────────

export const proxyApi = {
  start: (port: number) => invoke<string>("proxy_start", { port }),
  stop: () => invoke<void>("proxy_stop"),
  status: () => invoke<boolean>("proxy_status"),
  getSettings: () => invoke<ProxySettings>("proxy_get_settings"),
  setAutostart: (enabled: boolean) =>
    invoke<void>("proxy_set_autostart", { enabled }),
};

// ─── Claude Code Config Export ─────────────────────────────

export const configApi = {
  exportClaudeConfig: (port: number) =>
    invoke<string>("export_claude_config", { port }),
};
