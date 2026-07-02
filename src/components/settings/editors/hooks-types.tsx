import { useState } from "react";
import { useTranslation } from "react-i18next";
import { notificationApi, type NotifyHooksFragment } from "../../../services/api";
import { F } from "./tokens";

// ─── Hooks Section (friendly editor) ────────────────────────

export const HOOK_EVENTS: { id: string; label: string; desc: string; hasMatcher: boolean; matcherOptions: string[]; matcherFreeform: boolean }[] = [
  { id: "SessionStart", label: "会话启动", desc: "会话启动或恢复时触发", hasMatcher: true, matcherOptions: ["startup", "resume", "clear", "compact"], matcherFreeform: false },
  { id: "UserPromptSubmit", label: "提交提示", desc: "用户提交提示时触发", hasMatcher: false, matcherOptions: [], matcherFreeform: false },
  { id: "PreToolUse", label: "工具调用前", desc: "工具调用前触发，可阻止", hasMatcher: true, matcherOptions: ["Bash", "Edit", "Write", "Read", "Glob", "Grep", "WebFetch", "Agent"], matcherFreeform: true },
  { id: "PostToolUse", label: "工具调用后", desc: "工具调用成功后触发", hasMatcher: true, matcherOptions: ["Bash", "Edit", "Write", "Read", "Glob", "Grep", "WebFetch", "Agent"], matcherFreeform: true },
  { id: "Notification", label: "通知", desc: "发送通知时触发", hasMatcher: true, matcherOptions: ["permission_prompt", "idle_prompt", "auth_success", "elicitation_dialog"], matcherFreeform: false },
  { id: "Stop", label: "停止", desc: "Claude 完成响应时触发", hasMatcher: false, matcherOptions: [], matcherFreeform: false },
  { id: "SubagentStop", label: "子代理停止", desc: "子代理完成时触发", hasMatcher: true, matcherOptions: ["general-purpose", "Explore", "Plan"], matcherFreeform: true },
  { id: "ConfigChange", label: "配置变更", desc: "配置文件变更时触发", hasMatcher: true, matcherOptions: ["user_settings", "project_settings", "local_settings", "policy_settings", "skills"], matcherFreeform: false },
  { id: "FileChanged", label: "文件变更", desc: "监视文件变更时触发", hasMatcher: true, matcherOptions: [], matcherFreeform: true },
  { id: "CwdChanged", label: "目录切换", desc: "工作目录切换时触发", hasMatcher: false, matcherOptions: [], matcherFreeform: false },
  { id: "PreCompact", label: "压缩前", desc: "上下文压缩前触发", hasMatcher: true, matcherOptions: ["manual", "auto"], matcherFreeform: false },
  { id: "SessionEnd", label: "会话结束", desc: "会话结束时触发", hasMatcher: true, matcherOptions: ["clear", "resume", "logout", "prompt_input_exit", "other"], matcherFreeform: false },
];

export const HANDLER_TYPES = ["command", "http", "mcp_tool", "prompt", "agent"] as const;
export type HandlerType = (typeof HANDLER_TYPES)[number];

export const HANDLER_LABELS: Record<HandlerType, string> = {
  command: "命令",
  http: "HTTP",
  mcp_tool: "MCP 工具",
  prompt: "LLM 提示",
  agent: "Agent 验证",
};

export type HookHandler = {
  type: HandlerType;
  command?: string;
  args?: string[];
  url?: string;
  headers?: Record<string, string>;
  allowedEnvVars?: string[];
  server?: string;
  tool?: string;
  input?: Record<string, any>;
  prompt?: string;
  model?: string;
  timeout?: number;
  async?: boolean;
  "if"?: string;
  statusMessage?: string;
  shell?: string;
};

export type MatcherGroup = {
  matcher: string;
  hooks: HookHandler[];
};

export type HooksConfig = Record<string, MatcherGroup[]>;

// ─── notify hook 快捷注入/移除（Hooks 区按钮共享逻辑）──────────────
// 与后端 gateway::hooks 识别约定一致：按命令串含脚本文件名识别 aidog notify 项。
const AIDOG_NOTIFY_MARKERS = ["aidog-notify-complete", "aidog-notify-waiting"];

/** 判断一个 handler 是否为 aidog notify 命令（按 command 串含脚本文件名识别）。 */
function isAidogNotifyHandler(h: HookHandler): boolean {
  const cmd = h.command ?? "";
  return AIDOG_NOTIFY_MARKERS.some((m) => cmd.includes(m));
}

/** hooksValue 中是否已存在 aidog notify 注入项。 */
function hasNotifyHooks(hooks: HooksConfig | undefined): boolean {
  if (!hooks) return false;
  return Object.values(hooks).some((groups) =>
    groups.some((g) => g.hooks.some(isAidogNotifyHandler)),
  );
}

/**
 * 把后端 notify hook 片段并入当前 hooksValue。
 * - 先剥除现有 aidog notify 项（避免重复，幂等）。
 * - 再把片段中每个 event 的 aidog handler 追加为独立 matcher 组（matcher 空 = 匹配所有）。
 */
function mergeNotifyHooks(
  current: HooksConfig | undefined,
  fragment: NotifyHooksFragment,
): HooksConfig {
  const merged: HooksConfig = stripNotifyHooks(current) ?? {};
  for (const [event, groups] of Object.entries(fragment)) {
    for (const g of groups) {
      const handlers: HookHandler[] = (g.hooks ?? [])
        .filter((h) => isAidogNotifyHandler(h as HookHandler))
        .map((h) => ({ type: (h.type as HandlerType) ?? "command", command: h.command }));
      if (handlers.length === 0) continue;
      const existing = merged[event] ?? [];
      merged[event] = [...existing, { matcher: "", hooks: handlers }];
    }
  }
  return merged;
}

/**
 * 从 hooksValue 移除所有 aidog notify 项，保留用户其它 hook。
 * 空 handler 组 / 空 event 一并清理；全清后返回 undefined（与 syncHooks 一致）。
 */
function stripNotifyHooks(current: HooksConfig | undefined): HooksConfig | undefined {
  if (!current) return undefined;
  const cleaned: HooksConfig = {};
  for (const [event, groups] of Object.entries(current)) {
    const keptGroups = groups
      .map((g) => ({ ...g, hooks: g.hooks.filter((h) => !isAidogNotifyHandler(h)) }))
      .filter((g) => g.hooks.length > 0);
    if (keptGroups.length > 0) cleaned[event] = keptGroups;
  }
  return Object.keys(cleaned).length > 0 ? cleaned : undefined;
}

/** Hooks 区「注入/移除通知 hook」快捷条（注入态切换 + busy 防并发 + 失败提示）。 */
export function NotifyHookQuickBar(props: {
  hooksValue: HooksConfig | undefined;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const { hooksValue, updateField, t } = props;
  const injected = hasNotifyHooks(hooksValue);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>("");

  const handleInject = async () => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const fragment = await notificationApi.buildNotifyHooksFragment();
      const merged = mergeNotifyHooks(hooksValue, fragment);
      updateField("hooks", merged);
      updateField("_aidog_hooks", { enabled: true });
    } catch (e) {
      console.error("buildNotifyHooksFragment failed", e);
      setError(t("settings.hooksNotifyError", "操作失败，请重试"));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = () => {
    if (busy) return;
    const cleaned = stripNotifyHooks(hooksValue);
    updateField("hooks", cleaned);
    updateField("_aidog_hooks", { enabled: false });
    setError("");
  };

  return (
    <div className="glass-surface" style={{
      borderRadius: "var(--radius-md)", padding: "12px 16px",
      display: "flex", flexDirection: "column", gap: 8,
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 2, flex: 1, minWidth: 200 }}>
          <span style={{ fontSize: F.body, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("settings.hooksQuickTitle", "通知 hook")}
          </span>
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
            {injected
              ? t("settings.hooksNotifyInjected", "已注入 Claude Code 完成/等待通知 hook，保存后对全部分组生效。")
              : t("settings.hooksQuickDesc", "一键填入 Claude Code 完成/等待通知 hook，保存后对全部分组与 Codex 生效。")}
          </span>
        </div>
        {injected ? (
          <button type="button" className="btn btn-ghost" disabled={busy}
            style={{ fontSize: F.body, padding: "6px 14px", flexShrink: 0 }}
            onClick={handleRemove}>
            {t("notif.hookRemove", "移除")}
          </button>
        ) : (
          <button type="button" className="btn btn-primary" disabled={busy}
            style={{ fontSize: F.body, padding: "6px 14px", flexShrink: 0 }}
            onClick={handleInject}>
            {busy ? t("settings.hooksNotifyBusy", "处理中…") : t("settings.hooksNotifyInject", "注入通知 hook")}
          </button>
        )}
      </div>
      {error && (
        <span style={{ fontSize: F.hint, color: "var(--danger)" }}>{error}</span>
      )}
    </div>
  );
}

