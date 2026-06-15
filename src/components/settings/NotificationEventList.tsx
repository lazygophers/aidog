// ─── 逐 hook 事件触发配置（N2 hook 事件通知）────────────────
// 列 Claude Code 官方 hook 事件全量目录，每事件一行：开关 + notif_type 下拉 + 可选自定义文案。
// 默认仅精选 DEFAULT_ON_EVENTS（6 个）展示为 on；旧配置无 per_event → 按默认目录初始态展示，
// 用户改动才落 settings.per_event（经父组件 persist）。仅 claude_code（Codex 不逐事件）。
//
// 跨层镜像：CC_HOOK_EVENTS / DEFAULT_ON_EVENTS / defaultNotifTypeForEvent 逐字镜像后端
// src-tauri/src/gateway/models.rs（CC_HOOK_EVENTS / DEFAULT_ON_EVENTS / default_notif_type_for_event），
// 改一侧务必同步另一侧。事件名为 CC 官方英文原样，不翻译。

import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import type { NotifType, EventSetting } from "../../services/api";

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

/** 默认 ON 精选集（6 个；SessionStart 默认 off）。**跨层镜像**后端 `DEFAULT_ON_EVENTS`。 */
export const DEFAULT_ON_EVENTS: string[] = [
  "Stop",
  "SubagentStop",
  "Notification",
  "PermissionRequest",
  "SessionEnd",
  "PreCompact",
];

const NOTIF_TYPES: NotifType[] = ["task_complete", "waiting_input", "error"];

/**
 * 事件名 → 默认 notif_type 映射规则。**跨层镜像**后端 `default_notif_type_for_event`。
 * - 含 Failure/Denied/Error → error
 * - 含 Notification/Permission/Elicitation → waiting_input
 * - 其余（含 Stop/Complete/End）→ task_complete
 */
export function defaultNotifTypeForEvent(event: string): NotifType {
  if (event.includes("Failure") || event.includes("Denied") || event.includes("Error")) {
    return "error";
  }
  if (event.includes("Notification") || event.includes("Permission") || event.includes("Elicitation")) {
    return "waiting_input";
  }
  return "task_complete";
}

/** 取某事件的有效展示态：per_event 命中用存储值，否则按默认目录兜底（精选集 on + 默认 type）。 */
function effectiveSetting(perEvent: Record<string, EventSetting>, event: string): EventSetting {
  const stored = perEvent[event];
  if (stored) return stored;
  return {
    enabled: DEFAULT_ON_EVENTS.includes(event),
    notif_type: defaultNotifTypeForEvent(event),
    template: "",
  };
}

function notifTypeLabel(t: TFunction, type: NotifType): string {
  return t(`notif.type.${type}`, type);
}

interface Props {
  /** 当前 per_event（undefined → 空对象，按默认目录展示）。 */
  perEvent: Record<string, EventSetting> | undefined;
  /** 通知总开关关闭时禁用整区（事件配置依赖通知开启）。 */
  disabled: boolean;
  /** 更新某事件配置（父组件经 persist 落 settings.per_event）。 */
  onUpdate: (event: string, setting: EventSetting) => void;
}

/**
 * 逐 hook 事件配置列表。精选默认 ON 集排在前，其余按目录顺序。
 * 改动时把「展示态 + 用户改动」一起写回 per_event（首次改动即物化该事件的当前展示态）。
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

  return (
    <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 10 }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.eventListTitle", "逐 Hook 事件触发")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("notif.eventListDesc", "为每个 Claude Code hook 事件单独配置：是否触发、走哪种通知类型、可选自定义文案（留空用类型模板）。默认仅精选若干事件启用。")}
          {disabled && <span style={{ marginLeft: 4 }}>· {t("notif.defaultHooksDisabledHint", "需先开启通知")}</span>}
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 6, opacity: disabled ? 0.5 : 1, pointerEvents: disabled ? "none" : undefined }}>
        {ordered.map((event) => {
          const es = effectiveSetting(pe, event);
          return (
            <div
              key={event}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "8px 10px",
                borderRadius: 8,
                background: "var(--bg-subtle, rgba(127,127,127,0.06))",
                flexWrap: "wrap",
              }}
            >
              <div
                className={`toggle ${es.enabled ? "active" : ""}`}
                onClick={() => { if (!disabled) update(event, { enabled: !es.enabled }); }}
                role="switch"
                aria-checked={es.enabled}
                aria-label={event}
                tabIndex={disabled ? -1 : 0}
              />
              {/* 事件名英文原样（CC 官方名，不翻译） */}
              <code style={{ fontSize: 12, fontWeight: 600, minWidth: 150 }}>{event}</code>

              <select
                className="input"
                style={{ fontSize: 12, padding: "4px 6px", width: 130 }}
                value={es.notif_type}
                disabled={!es.enabled}
                onChange={(e) => update(event, { notif_type: e.target.value as NotifType })}
                aria-label={`${event} ${t("notif.eventTypeLabel", "通知类型")}`}
              >
                {NOTIF_TYPES.map((type) => (
                  <option key={type} value={type}>{notifTypeLabel(t, type)}</option>
                ))}
              </select>

              <input
                className="input"
                style={{ fontSize: 12, padding: "4px 6px", flex: 1, minWidth: 160 }}
                value={es.template}
                disabled={!es.enabled}
                placeholder={t("notif.eventTemplatePlaceholder", "留空用所选类型的模板")}
                onChange={(e) => update(event, { template: e.target.value })}
                aria-label={`${event} ${t("notif.fieldTemplate", "模板")}`}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
