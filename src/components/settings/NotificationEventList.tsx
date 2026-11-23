// ─── 逐 hook 事件触发配置（N2 hook 事件通知 — 逐事件自含）──────────
// 列 Claude Code 官方 hook 事件全量目录，每事件一行：启用开关 + TTS 开关 + 弹窗开关 +
// 自定义模板（textarea，placeholder = 该事件专属默认模板）+ 该事件专属可用入参提示（每事件不同）。
// 默认仅精选 DEFAULT_ON_EVENTS（2 个）展示为 on；旧配置无 per_event → 按默认目录初始态展示，
// 用户改动才落 settings.per_event（经父组件 persist）。仅 claude_code（Codex 不逐事件）。
//
// 跨层镜像：CC_HOOK_EVENTS / DEFAULT_ON_EVENTS / EVENT_CATALOG 逐字镜像后端
// src-tauri/crates/aidog_db/src/models/notification.rs（CC_HOOK_EVENTS / DEFAULT_ON_EVENTS / default_template_for_event
// + 事件目录表的专属入参），改一侧务必同步另一侧。事件名为 CC 官方英文原样，不翻译。

import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import type { EventSetting } from "../../services/api";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { useReveal } from "../../components/shared";

/**
 * Claude Code 官方 hook 事件全量目录（约 30 个）。
 * **跨层镜像**后端 `CC_HOOK_EVENTS`。事件名英文原样（CC 官方名，非翻译）。
 */
export const CC_HOOK_EVENTS: string[] = [
  "SessionStart",
  "Setup",
  "InstructionsLoaded",
  "UserPromptSubmit",
  "UserPromptExpansion",
  "MessageDisplay",
  "PreToolUse",
  "PermissionRequest",
  "PermissionDenied",
  "PostToolUse",
  "PostToolUseFailure",
  "PostToolBatch",
  "Notification",
  "SubagentStart",
  "SubagentStop",
  "Stop",
  "StopFailure",
  "TeammateIdle",
  "TaskCreated",
  "TaskCompleted",
  "ConfigChange",
  "CwdChanged",
  "FileChanged",
  "WorktreeCreate",
  "WorktreeRemove",
  "PreCompact",
  "PostCompact",
  "Elicitation",
  "ElicitationResult",
  "SessionEnd",
];

/** 默认 ON 精选集（2 个：任务完成 + 等待授权；其余默认 off 可手动开）。**跨层镜像**后端 `DEFAULT_ON_EVENTS`。 */
export const DEFAULT_ON_EVENTS: string[] = ["Stop", "PermissionRequest"];

/** 通用入参（所有事件都有）：项目名 + 会话 id + 会话名（取不到为空串）。事件专属入参见 EVENT_CATALOG.vars。 */
const COMMON_VARS = ["{project}", "{session}", "{session_name}"];

/**
 * per-event 目录：每事件**专属可用入参**（每事件不同）；专属默认模板在 locale `notif.eventTemplate.<事件>`，
 * 与后端 `src-tauri/crates/aidog_db/src/models/notification.rs` 的 `default_template_for_event` 读同一份 key。
 * 专属入参列跨层镜像后端事件目录表，改一侧务必同步另一侧。
 * vars 为该事件**额外**专属占位（不含 COMMON_VARS）。
 */
export const EVENT_CATALOG: Record<string, { vars: string[] }> = {
  SessionStart: { vars: ["{source}", "{model}", "{agent_type}", "{session_title}"] },
  Setup: { vars: ["{trigger}"] },
  InstructionsLoaded: { vars: ["{file_path}", "{memory_type}", "{load_reason}"] },
  UserPromptSubmit: { vars: ["{prompt}"] },
  UserPromptExpansion: { vars: ["{command_name}", "{command_args}"] },
  MessageDisplay: { vars: ["{turn_id}", "{final}"] },
  PreToolUse: { vars: ["{tool_name}"] },
  PermissionRequest: { vars: ["{action}", "{tool_name}"] },
  PermissionDenied: { vars: ["{tool_name}", "{reason}"] },
  PostToolUse: { vars: ["{tool_name}", "{duration_ms}"] },
  PostToolUseFailure: { vars: ["{tool_name}", "{error}"] },
  PostToolBatch: { vars: [] },
  Notification: { vars: ["{action}", "{message}", "{notification_type}"] },
  SubagentStart: { vars: ["{agent_type}"] },
  SubagentStop: { vars: ["{agent_type}"] },
  Stop: { vars: [] },
  StopFailure: { vars: ["{error_code}", "{error_message}"] },
  TeammateIdle: { vars: ["{teammate_id}", "{status}"] },
  TaskCreated: { vars: ["{task_id}", "{task_name}"] },
  TaskCompleted: { vars: ["{task_id}", "{task_name}"] },
  ConfigChange: { vars: ["{config_source}"] },
  CwdChanged: { vars: ["{old_cwd}", "{new_cwd}"] },
  FileChanged: { vars: ["{file_path}", "{change_type}"] },
  WorktreeCreate: { vars: ["{worktree_path}"] },
  WorktreeRemove: { vars: ["{worktree_path}"] },
  PreCompact: { vars: ["{compact_reason}", "{context_size}"] },
  PostCompact: { vars: ["{context_reduction_ratio}"] },
  Elicitation: { vars: ["{server_name}", "{tool_name}"] },
  ElicitationResult: { vars: ["{server_name}"] },
  SessionEnd: { vars: ["{end_reason}", "{duration_ms}"] },
};

/** 取某事件专属默认模板（缺失 → 仅 {project} 兜底，与后端「未命中类型 default_template」呼应）。 */
function defaultTemplateForEvent(t: TFunction, event: string): string {
  return EVENT_CATALOG[event] ? t(`notif.eventTemplate.${event}`) : "{project}";
}

/** 取某事件可用入参列表（通用 + 专属）。 */
function eventVars(event: string): string[] {
  return [...COMMON_VARS, ...(EVENT_CATALOG[event]?.vars ?? [])];
}

/** 取某事件的有效展示态：per_event 命中用存储值，否则按默认目录兜底（精选集 on + tts/popup 默认开）。 */
function effectiveSetting(perEvent: Record<string, EventSetting | undefined>, event: string): EventSetting {
  const stored = perEvent[event];
  if (stored) return stored;
  return {
    enabled: DEFAULT_ON_EVENTS.includes(event),
    tts: true,
    popup: true,
    sound: true,
    template: "",
  };
}

// ponytail: 单行包装 — 每实例独立 useReveal (React 规则禁 map 内条件 hook),
// reveal 萤火虫错峰入场 (stagger index*60)。
function EventRow({
  index,
  children,
}: {
  index: number;
  children: React.ReactNode;
}) {
  const { ref, shown } = useReveal<HTMLDivElement>(index * 60);
  return (
    <div
      ref={ref}
      className={`reveal${shown ? " in" : ""}`}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: "10px 12px",
        borderRadius: 8,
        background: "var(--secondary)",
      }}
    >
      {children}
    </div>
  );
}

interface Props {
  /** 当前 per_event（undefined → 空对象，按默认目录展示）。 */
  perEvent: Record<string, EventSetting | undefined> | undefined;
  /** 通知总开关关闭时禁用整区（事件配置依赖通知开启）。 */
  disabled: boolean;
  /** 更新某事件配置（父组件经 persist 落 settings.per_event）。 */
  onUpdate: (event: string, setting: EventSetting) => void;
}

/**
 * 逐 hook 事件配置列表。精选默认 ON 集排在前，其余按目录顺序。
 * 改动时把「展示态 + 用户改动」一起写回 per_event（首次改动即物化该事件的当前展示态）。
 * 每事件：启用 + TTS + 弹窗 开关 + 模板 textarea（placeholder = 该事件专属默认模板）+ 专属入参提示。
 */
export function NotificationEventList({ perEvent, disabled, onUpdate }: Props) {
  const { t } = useTranslation();
  const pe = perEvent ?? {};

  // 排序：默认 ON 精选集在前（按 DEFAULT_ON_EVENTS 顺序），其余按目录顺序。
  const ordered = [
    ...DEFAULT_ON_EVENTS.filter((e) => CC_HOOK_EVENTS.includes(e)),
    ...CC_HOOK_EVENTS.filter((e) => !DEFAULT_ON_EVENTS.includes(e)),
  ];

  const update = (event: string, partial: Partial<EventSetting>) => {
    const cur = effectiveSetting(pe, event);
    onUpdate(event, { ...cur, ...partial });
  };

  // 外层卡片本体 reveal。
  const { ref: cardRef, shown: cardShown } = useReveal<HTMLDivElement>(0);

  return (
    <div
      ref={cardRef}
      className={`glass-surface glass-highlight hover-lift reveal${cardShown ? " in" : ""}`}
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 10 }}
    >
      <div>
        <div style={{ fontSize: 14, fontWeight: 600 }}>{t("notif.eventListTitle", "Per-Hook Event Triggers")}</div>
        <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>
          {t("notif.eventListDesc", "Configure each Claude Code hook event individually: whether to trigger, which notification type, and an optional custom message (leave blank to use the type's template). Only a curated set is enabled by default.")}
          {disabled && <span style={{ marginLeft: 4 }}>· {t("notif.defaultHooksDisabledHint", "Enable notifications first")}</span>}
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 8, opacity: disabled ? 0.5 : 1, pointerEvents: disabled ? "none" : undefined }}>
        {ordered.map((event, idx) => {
          const es = effectiveSetting(pe, event);
          const vars = eventVars(event);
          return (
            <EventRow key={event} index={idx}>
              {/* 第一行：启用开关 + 事件名 + TTS / 弹窗 开关 */}
              <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
                <Switch
                  checked={es.enabled}
                  disabled={disabled}
                  onCheckedChange={(v) => update(event, { enabled: v })}
                  aria-label={event}
                />
                {/* 事件名英文原样（CC 官方名，不翻译） */}
                <code style={{ fontSize: 14, fontWeight: 600, minWidth: 150 }}>{event}</code>

                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>{t("notif.fieldTts", "Voice")}</span>
                  <Switch
                    checked={es.tts}
                    disabled={disabled || !es.enabled}
                    onCheckedChange={(v) => update(event, { tts: v })}
                    aria-label={`${event} ${t("notif.fieldTts", "Voice")}`}
                  />
                </div>
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>{t("notif.fieldPopup", "Popup")}</span>
                  <Switch
                    checked={es.popup}
                    disabled={disabled || !es.enabled}
                    onCheckedChange={(v) => update(event, { popup: v })}
                    aria-label={`${event} ${t("notif.fieldPopup", "Popup")}`}
                  />
                </div>
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>{t("notif.fieldSound", "Sound")}</span>
                  <Switch
                    checked={es.sound !== false}
                    disabled={disabled || !es.enabled}
                    onCheckedChange={(v) => update(event, { sound: v })}
                    aria-label={`${event} ${t("notif.fieldSound", "Sound")}`}
                  />
                </div>
              </div>

              {/* 第二行：模板 textarea（placeholder = 该事件专属默认模板） */}
              <Textarea

                style={{ fontSize: 14, fontFamily: "var(--font-mono, monospace)", minHeight: 40, resize: "vertical", width: "100%", boxSizing: "border-box" }}
                value={es.template}
                disabled={!es.enabled}
                placeholder={defaultTemplateForEvent(t, event)}
                onChange={(e) => update(event, { template: e.target.value })}
                aria-label={`${event} ${t("notif.fieldTemplate", "Template")}`}
              />

              {/* 第三行：该事件专属可用入参提示（每事件不同） */}
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
                <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>{t("notif.eventVarsHint", "Available variables")}</span>
                {vars.map((v) => (
                  <code
                    key={v}
                    style={{
                      fontSize: 14,
                      padding: "2px 6px",
                      borderRadius: "var(--radius-sm)",
                      background: "var(--accent-subtle)",
                      color: "var(--accent)",
                    }}
                  >
                    {v}
                  </code>
                ))}
                <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>
                  {t("notif.optionalPlaceholderHint", "Optional placeholder {prefix|variable}: outputs prefix + value when the variable is non-empty, nothing otherwise, e.g. {project}{ · |session_name}")}
                </span>
              </div>
            </EventRow>
          );
        })}
      </div>
    </div>
  );
}
