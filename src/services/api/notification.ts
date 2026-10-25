// notification.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "../transport";
import type { NotifType, NotificationSettings, Notification, NotificationDelivery, NotifyHooksFragment, NotifyDispatchResult, HookClient } from "./types";

export const notificationApi = {
  /** 读取通知设置（无配置 → 默认全开 cross_platform）。 */
  getSettings: () => invoke<NotificationSettings>("notification_settings_get"),
  /** 保存通知设置。 */
  setSettings: (settings: NotificationSettings) =>
    invoke<void>("notification_settings_set", { settings }),
  /** 列收件箱（倒序；limit 默认 100）。 */
  listInbox: (limit?: number, filter?: { source?: string; notifType?: string; severity?: string; sinceMs?: number }) =>
    invoke<Notification[]>("notification_inbox_list", {
      limit,
      source: filter?.source,
      notifType: filter?.notifType,
      severity: filter?.severity,
      sinceMs: filter?.sinceMs,
    }),
  /** 清空收件箱。 */
  clearInbox: () => invoke<void>("notification_clear"),
  /** 测试通知（走分发逻辑，含弹窗/TTS）。 */
  testNotify: (notifType: NotifType | string, content?: string) =>
    invoke<NotifyDispatchResult>("notification_test", { notifType, content }),
  /** 仅测 TTS 通道：按当前 settings.tts_backend 播报 text，不走 dispatch。 */
  testTts: (text: string) =>
    invoke<void>("notification_test_tts", { text }),
  /** 仅测系统弹窗通道：直接调 tauri-plugin-notification，不走 dispatch。 */
  testPopup: (title: string, body: string) =>
    invoke<void>("notification_test_popup", { title, body }),
  /** 仅测系统提示音通道：跨平台 spawn beep（macOS afplay / Windows powershell / Linux paplay）。 */
  testBeep: () =>
    invoke<void>("notification_test_beep"),
  /** 外部渠道测试发送（票 03/04：真实投递 + 落投递记录；未配置渠道报错）。 */
  testChannel: (channel: string) =>
    invoke<{ channel: string; ok: boolean; error: string; duration_ms: number; attempts: number }>(
      "notification_test_channel", { channel }),
  /** 投递记录查询（票 04：收件箱展开；actionKey 过滤可选）。 */
  listDeliveries: (actionKey?: string, limit?: number) =>
    invoke<NotificationDelivery[]>("notification_delivery_list", { limit, actionKey }),
  /** 规则表达式编译校验（票 04：Err 文案带「第 N 字符附近」定位）。 */
  validateRuleExpr: (expr: string) =>
    invoke<void>("notification_rule_validate", { expr }),
  /**
   * 一键注入通知 hook（N2）。
   * - client="claude_code"：把 hooks.Stop/Notification 注入基线配置并 re-sync 到所有 settings.{group}.json。
   * - client="codex"：把 notify=[complete 脚本] 注入 ~/.codex/config.toml。
   * 同时物化内置默认模板（task_complete/waiting_input）。group 用于 API 对称。
   */
  injectHooks: (group: string, client: HookClient) =>
    invoke<void>("inject_hooks", { group, client }),
  /** 一键移除通知 hook（strip）。client 同 injectHooks。 */
  removeHooks: (group: string, client: HookClient) =>
    invoke<void>("remove_hooks", { group, client }),
  /** 读取「默认为所有分组注入通知 hook」总开关（基线 _aidog_hooks.enabled）。 */
  getDefaultHooksEnabled: () =>
    invoke<boolean>("get_default_hooks_enabled"),
  /**
   * 构造通知 hook 片段供 Hooks 编辑器并入草稿（只读式：确保 notify 脚本落盘，
   * 但不写 DB、不 sync）。返回 `{Stop:[...], Notification:[...]}` 形状的 CC hooks 子对象。
   * 前端把它并入草稿 config.hooks，由用户正常保存触发既有 sync 物化。
   */
  buildNotifyHooksFragment: () =>
    invoke<NotifyHooksFragment>("build_notify_hooks_fragment"),
  /**
   * 设置「默认为所有分组注入通知 hook」总开关：开=全分组注入 CC hooks + Codex notify，
   * 关=全移除。写基线 _aidog_hooks.enabled 并 re-sync 物化。
   */
  setDefaultHooksEnabled: (enabled: boolean) =>
    invoke<void>("set_default_hooks_enabled", { enabled }),
};

/** hook 注入客户端类型（N2）。 */

