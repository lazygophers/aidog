// settings.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "../transport";
import type { CodingToolsSettings } from "./types";


// ─── Claude Code Config Export ─────────────────────────────

export const configApi = {
  exportClaudeConfig: (port: number) =>
    invoke<string>("export_claude_config", { port }),
  syncGroupSettings: () =>
    invoke<string[]>("sync_group_settings"),
};

/**
 * Read the aidog-managed leaf dot-path snapshot from the internal DB
 * (`setting` table scope=`claude_default_group`/key=`managed_paths`, written by
 * the default-group sync). Empty/missing → `[]`. Used by the import diff to
 * exclude managed fields. Marker moved out of settings.json (was `_aidog_managed`)
 * to keep the user's file clean.
 */


export const getManagedPaths = (): Promise<string[]> =>
  invoke<string[]>("get_managed_paths");

// ─── Proxy Log Types ───────────────────────────────────────

/** 单次平台尝试快照（proxy_log.attempts JSON 数组元素）。 */


export const NOTIF_SPEAK = "notif-speak";

// ─── Settings API ──────────────────────────────────────────



// ─── Settings API ──────────────────────────────────────────

export const settingsApi = {
  get: (scope: string, key: string) =>
    invoke<Record<string, any> | null>("settings_get", { scope, key }),

  set: (scope: string, key: string, value: Record<string, any>) =>
    invoke<void>("settings_set", { input: { scope, key, value } }),

  delete: (scope: string, key: string) =>
    invoke<void>("settings_delete", { scope, key }),

  list: (scope: string) =>
    invoke<string[]>("settings_list", { scope }),
};

/**
 * 读全量 `claude_code` config → 用 mutator 改字段 → 写回 DB → best-effort syncGroupSettings。
 * sync 失败仅 console.error 不阻断（与原 inline 行为一致）；read/set 失败抛错交 caller。
 * 抽自 CodingToolsSettings（language / compact 双写路径）+ Settings.handleSave 同类模式。
 */
export async function writeClaudeConfigField(
  mutator: (cfg: Record<string, any>) => Record<string, any>,
): Promise<void> {
  const cfg = (await settingsApi.get("global", "claude_code")) ?? {};
  await settingsApi.set("global", "claude_code", mutator({ ...cfg }));
  try {
    await configApi.syncGroupSettings();
  } catch (e) {
    console.error("sync_group_settings:", e);
  }
}

// ─── StatusLine Script Generation ──────────────────────────



// ─── StatusLine Script Generation ──────────────────────────

export const statuslineApi = {
  /**
   * Render the statusline Python script body for the given UI config — preview
   * only, nothing is written to disk. Script materialization lives in Rust
   * (`do_sync_group_settings` rewrites both scripts on every startup / settings
   * change). Returns "" for custom/disabled modes (no builtin script).
   */
  preview: (scriptType: string, stored: unknown) =>
    invoke<string>("preview_statusline_script", { scriptType, stored }),
};

// ─── Script Executor (uv / python3) ────────────────────────

/** 脚本执行器选择。"uv" → uv run --script；"python3" → python3。 */



// ─── Codex Config API ─────────────────────────────────────

export const codexApi = {
  /** Read ~/.codex/config.toml (TOML) → JSON. Missing file → {}. */
  read: () => invoke<Record<string, unknown>>("codex_config_read"),
  /** Write JSON → ~/.codex/config.toml (TOML). Creates ~/.codex/ if missing. */
  write: (value: Record<string, unknown>) =>
    invoke<void>("codex_config_write", { value }),
  /** Absolute path of ~/.codex/config.toml. */
  path: () => invoke<string>("codex_config_path"),
};

// ─── pi Config API ────────────────────────────────────────

export const piApi = {
  /** Read ~/.pi/agent/settings.json. Missing file → {}. */
  read: () => invoke<Record<string, unknown>>("pi_settings_read"),
  /** Write the whole ~/.pi/agent/settings.json back (unknown keys survive the round trip). */
  write: (config: Record<string, unknown>) =>
    invoke<void>("pi_settings_write", { config }),
  /** Absolute path of ~/.pi/agent/models.json. */
  modelsPath: () => invoke<string>("pi_models_path"),
};

// ─── Claude Code Settings Import ──────────────────────────



// ─── Claude Code Settings Import ──────────────────────────

export const claudeSettingsImportApi = {
  /** Read ~/.claude/settings.json and return parsed JSON */
  readDefault: () =>
    invoke<Record<string, any>>("read_claude_code_settings"),
};

// ─── App Log Settings API ─────────────────────────────────


export const codingToolsSettingsApi = {
  get: () => invoke<CodingToolsSettings>("coding_tools_settings_get"),
  set: (partial: Partial<CodingToolsSettings>) =>
    invoke<CodingToolsSettings>("coding_tools_settings_set", {
      applyToClaudePlugin: partial.apply_to_claude_plugin,
      skipClaudeOnboarding: partial.skip_claude_onboarding,
    }),
};

// ─── Statistics Types & API ──────────────────────────────

