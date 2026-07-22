// ─── 逐 hook 事件触发配置（N2 hook 事件通知 — 逐事件自含）──────────
// 列 Claude Code 官方 hook 事件全量目录，每事件一行：启用开关 + TTS 开关 + 弹窗开关 +
// 自定义模板（textarea，placeholder = 该事件专属默认模板）+ 该事件专属可用入参提示（每事件不同）。
// 默认仅精选 DEFAULT_ON_EVENTS（2 个）展示为 on；旧配置无 per_event → 按默认目录初始态展示，
// 用户改动才落 settings.per_event（经父组件 persist）。仅 claude_code（Codex 不逐事件）。
//
// 跨层镜像：CC_HOOK_EVENTS / DEFAULT_ON_EVENTS / EVENT_CATALOG 逐字镜像后端
// src-tauri/src/gateway/models.rs（CC_HOOK_EVENTS / DEFAULT_ON_EVENTS / default_template_for_event
// + 事件目录表的专属入参），改一侧务必同步另一侧。事件名为 CC 官方英文原样，不翻译。

import { useTranslation } from "react-i18next";
import type { EventSetting } from "../../services/api";
import { Textarea } from "@/components/ui/textarea";

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

/** 通用入参（所有事件都有）：项目名 + 会话 id。事件专属入参见 EVENT_CATALOG.vars。 */
const COMMON_VARS = ["{project}", "{session}"];

/**
 * per-event 目录：每事件**专属默认模板 + 专属可用入参**（每事件不同）。
 * **跨层镜像**后端 `src-tauri/src/gateway/models.rs` 的 `default_template_for_event` + 事件目录表
 * 的「专属入参」列；改一侧务必逐字同步另一侧。defaultTemplate 为 zh 硬编码（非 i18n）。
 * vars 为该事件**额外**专属占位（不含 COMMON_VARS）。
 */
export const EVENT_CATALOG: Record<string, { defaultTemplate: string; vars: string[] }> = {
  SessionStart: { defaultTemplate: "{project} 会话开始", vars: ["{source}", "{model}", "{agent_type}", "{session_title}"] },
  Setup: { defaultTemplate: "{project} 初始化（{trigger}）", vars: ["{trigger}"] },
  InstructionsLoaded: { defaultTemplate: "{project} 已加载 {memory_type}", vars: ["{file_path}", "{memory_type}", "{load_reason}"] },
  UserPromptSubmit: { defaultTemplate: "{project} 收到新指令", vars: ["{prompt}"] },
  UserPromptExpansion: { defaultTemplate: "{project} 展开命令 {command_name}", vars: ["{command_name}", "{command_args}"] },
  MessageDisplay: { defaultTemplate: "{project} 消息更新", vars: ["{turn_id}", "{final}"] },
  PreToolUse: { defaultTemplate: "{project} 即将执行 {tool_name}", vars: ["{tool_name}"] },
  PermissionRequest: { defaultTemplate: "{project} 请求授权：{tool_name}", vars: ["{tool_name}"] },
  PermissionDenied: { defaultTemplate: "{project} 拒绝 {tool_name}：{reason}", vars: ["{tool_name}", "{reason}"] },
  PostToolUse: { defaultTemplate: "{project} {tool_name} 完成（{duration_ms}ms）", vars: ["{tool_name}", "{duration_ms}"] },
  PostToolUseFailure: { defaultTemplate: "{project} {tool_name} 失败：{error}", vars: ["{tool_name}", "{error}"] },
  PostToolBatch: { defaultTemplate: "{project} 批量工具完成", vars: [] },
  Notification: { defaultTemplate: "{project}：{message}", vars: ["{message}", "{type}"] },
  SubagentStart: { defaultTemplate: "{project} 子代理 {agent_type} 启动", vars: ["{agent_type}"] },
  SubagentStop: { defaultTemplate: "{project} 子代理 {agent_type} 完成", vars: ["{agent_type}"] },
  Stop: { defaultTemplate: "{project} 任务完成", vars: [] },
  StopFailure: { defaultTemplate: "{project} 中断：{error_message}", vars: ["{error_code}", "{error_message}"] },
  TeammateIdle: { defaultTemplate: "{project} 队友 {teammate_id} 空闲", vars: ["{teammate_id}", "{status}"] },
  TaskCreated: { defaultTemplate: "{project} 新建任务：{task_name}", vars: ["{task_id}", "{task_name}"] },
  TaskCompleted: { defaultTemplate: "{project} 任务完成：{task_name}", vars: ["{task_id}", "{task_name}"] },
  ConfigChange: { defaultTemplate: "{project} 配置变更（{config_source}）", vars: ["{config_source}"] },
  CwdChanged: { defaultTemplate: "{project} 切换目录：{new_cwd}", vars: ["{old_cwd}", "{new_cwd}"] },
  FileChanged: { defaultTemplate: "{project} 文件变更：{file_path}", vars: ["{file_path}", "{change_type}"] },
  WorktreeCreate: { defaultTemplate: "{project} 创建 worktree", vars: ["{worktree_path}"] },
  WorktreeRemove: { defaultTemplate: "{project} 移除 worktree", vars: ["{worktree_path}"] },
  PreCompact: { defaultTemplate: "{project} 即将压缩上下文（{compact_reason}）", vars: ["{compact_reason}", "{context_size}"] },
  PostCompact: { defaultTemplate: "{project} 压缩完成", vars: ["{context_reduction_ratio}"] },
  Elicitation: { defaultTemplate: "{project} {server_name} 请求输入", vars: ["{server_name}", "{tool_name}"] },
  ElicitationResult: { defaultTemplate: "{project} {server_name} 已响应", vars: ["{server_name}"] },
  SessionEnd: { defaultTemplate: "{project} 会话结束（{end_reason}）", vars: ["{end_reason}", "{duration_ms}"] },
};

/** 取某事件专属默认模板（缺失 → 仅 {project} 兜底，与后端「未命中类型 default_template」呼应）。 */
function defaultTemplateForEvent(event: string): string {
  return EVENT_CATALOG[event]?.defaultTemplate ?? "{project}";
}

/** 取某事件可用入参列表（通用 + 专属）。 */
function eventVars(event: string): string[] {
  return [...COMMON_VARS, ...(EVENT_CATALOG[event]?.vars ?? [])];
}

/** 取某事件的有效展示态：per_event 命中用存储值，否则按默认目录兜底（精选集 on + tts/popup 默认开）。 */
function effectiveSetting(perEvent: Record<string, EventSetting>, event: string): EventSetting {
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

  return (
    <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 10 }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.eventListTitle", "逐 Hook 事件触发")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("notif.eventListDesc", "为每个 Claude Code hook 事件单独配置：是否触发、语音、弹窗、可选自定义文案（留空用该事件默认模板）。每事件可用入参不同，见下方提示。默认仅精选若干事件启用。")}
          {disabled && <span style={{ marginLeft: 4 }}>· {t("notif.defaultHooksDisabledHint", "需先开启通知")}</span>}
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 8, opacity: disabled ? 0.5 : 1, pointerEvents: disabled ? "none" : undefined }}>
        {ordered.map((event) => {
          const es = effectiveSetting(pe, event);
          const vars = eventVars(event);
          return (
            <div
              key={event}
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 8,
                padding: "10px 12px",
                borderRadius: 8,
                background: "var(--bg-subtle, rgba(127,127,127,0.06))",
              }}
            >
              {/* 第一行：启用开关 + 事件名 + TTS / 弹窗 开关 */}
              <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
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

                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("notif.fieldTts", "语音")}</span>
                  <div
                    className={`toggle ${es.tts ? "active" : ""}`}
                    onClick={() => { if (!disabled && es.enabled) update(event, { tts: !es.tts }); }}
                    role="switch"
                    aria-checked={es.tts}
                    aria-label={`${event} ${t("notif.fieldTts", "语音")}`}
                    style={es.enabled ? undefined : { opacity: 0.5, cursor: "not-allowed" }}
                    tabIndex={disabled || !es.enabled ? -1 : 0}
                  />
                </div>
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("notif.fieldPopup", "弹窗")}</span>
                  <div
                    className={`toggle ${es.popup ? "active" : ""}`}
                    onClick={() => { if (!disabled && es.enabled) update(event, { popup: !es.popup }); }}
                    role="switch"
                    aria-checked={es.popup}
                    aria-label={`${event} ${t("notif.fieldPopup", "弹窗")}`}
                    style={es.enabled ? undefined : { opacity: 0.5, cursor: "not-allowed" }}
                    tabIndex={disabled || !es.enabled ? -1 : 0}
                  />
                </div>
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("notif.fieldSound", "提示音")}</span>
                  <div
                    className={`toggle ${es.sound !== false ? "active" : ""}`}
                    onClick={() => { if (!disabled && es.enabled) update(event, { sound: es.sound === false }); }}
                    role="switch"
                    aria-checked={es.sound !== false}
                    aria-label={`${event} ${t("notif.fieldSound", "提示音")}`}
                    style={es.enabled ? undefined : { opacity: 0.5, cursor: "not-allowed" }}
                    tabIndex={disabled || !es.enabled ? -1 : 0}
                  />
                </div>
              </div>

              {/* 第二行：模板 textarea（placeholder = 该事件专属默认模板） */}
              <Textarea
                
                style={{ fontSize: 12, fontFamily: "var(--font-mono, monospace)", minHeight: 40, resize: "vertical", width: "100%", boxSizing: "border-box" }}
                value={es.template}
                disabled={!es.enabled}
                placeholder={defaultTemplateForEvent(event)}
                onChange={(e) => update(event, { template: e.target.value })}
                aria-label={`${event} ${t("notif.fieldTemplate", "模板")}`}
              />

              {/* 第三行：该事件专属可用入参提示（每事件不同） */}
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
                <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>{t("notif.eventVarsHint", "可用入参")}</span>
                {vars.map((v) => (
                  <code
                    key={v}
                    style={{
                      fontSize: 11,
                      padding: "2px 6px",
                      borderRadius: "var(--radius-sm)",
                      background: "var(--accent-subtle)",
                      color: "var(--accent)",
                    }}
                  >
                    {v}
                  </code>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
