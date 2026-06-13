// ─── 系统通知设置面板（AppSettings「通知」tab；N3）────────────
// 消费 N1/N2 冻结的 services/api.ts 契约（notificationApi + 类型），只读不改。
// 提供：总开关 + TTS 总开关 + TTS 后端选择 + 按类型 {tts,popup,form,template} 编辑 +
//       变量提示 + 测试通知 + Claude Code / Codex 一键注入/移除 hook。

import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  notificationApi,
  groupApi,
  type NotificationSettings as NotifSettings,
  type TypeSetting,
  type NotifType,
  type NotifForm,
  type TtsBackend,
  type HookClient,
  type Group,
} from "../../services/api";

// 与 api.ts 契约对齐（禁裸 string）。
const NOTIF_TYPES: NotifType[] = ["task_complete", "waiting_input", "error", "custom"];
const NOTIF_FORMS: NotifForm[] = ["full", "popup_only", "inbox_only", "sound_only"];
const TTS_BACKENDS: TtsBackend[] = ["cross_platform", "mac_say", "web_speech"];
const HOOK_CLIENTS: HookClient[] = ["claude_code", "codex"];
const TEMPLATE_VARS = ["{project}", "{status}", "{time}", "{session}", "{group}"];

const DEFAULT_TYPE_SETTING: TypeSetting = { tts: true, popup: true, form: "full", template: "" };

const DEFAULT_SETTINGS: NotifSettings = {
  enabled: true,
  tts_enabled: true,
  tts_backend: "cross_platform",
  per_type: {},
};

function notifTypeLabel(t: TFunction, type: NotifType): string {
  return t(`notif.type.${type}`, type);
}

function notifFormLabel(t: TFunction, form: NotifForm): string {
  return t(`notif.form.${form}`, form);
}

function ttsBackendLabel(t: TFunction, b: TtsBackend): string {
  return t(`notif.ttsBackend.${b}`, b);
}

function hookClientLabel(t: TFunction, c: HookClient): string {
  return t(`notif.hookClient.${c}`, c);
}

export function NotificationSettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<NotifSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [groups, setGroups] = useState<Group[]>([]);
  const [hookGroup, setHookGroup] = useState("");
  const [hookBusy, setHookBusy] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const s = await notificationApi.getSettings();
        setSettings(s);
      } catch (e) {
        console.error("load notification settings failed", e);
      }
      try {
        const gs = await groupApi.list();
        setGroups(gs);
        if (gs.length > 0) setHookGroup(gs[0].name);
      } catch (e) {
        console.error("load groups failed", e);
      }
      setLoading(false);
    })();
  }, []);

  const persist = async (next: NotifSettings) => {
    setSettings(next);
    try {
      await notificationApi.setSettings(next);
      setError("");
    } catch (e) {
      console.error("set notification settings failed", e);
      setError(String(e));
    }
  };

  const typeSetting = (type: NotifType): TypeSetting =>
    settings.per_type[type] ?? DEFAULT_TYPE_SETTING;

  const updateType = (type: NotifType, partial: Partial<TypeSetting>) => {
    const current = typeSetting(type);
    persist({
      ...settings,
      per_type: { ...settings.per_type, [type]: { ...current, ...partial } },
    });
  };

  const handleTest = async (type: NotifType) => {
    try {
      await notificationApi.testNotify(type);
      setMessage(t("notif.testSent", "测试通知已发送"));
    } catch (e) {
      console.error("test notify failed", e);
      setMessage(String(e));
    }
  };

  const handleInject = async (client: HookClient, remove: boolean) => {
    if (!hookGroup) {
      setMessage(t("notif.hookNoGroup", "请先选择分组"));
      return;
    }
    setHookBusy(true);
    try {
      if (remove) {
        await notificationApi.removeHooks(hookGroup, client);
        setMessage(t("notif.hookRemoved", "已移除通知 hook"));
      } else {
        await notificationApi.injectHooks(hookGroup, client);
        setMessage(t("notif.hookInjected", "已注入通知 hook"));
      }
    } catch (e) {
      console.error("hook op failed", e);
      setMessage(String(e));
    }
    setHookBusy(false);
  };

  if (loading) {
    return (
      <div className="text-secondary" style={{ padding: 20 }}>
        {t("status.loading", "加载中…")}
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* 总开关 */}
      <div
        className="glass-surface"
        style={{ padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center" }}
      >
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.masterToggle", "通知总开关")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("notif.masterToggleDesc", "关闭后所有通知分发被旁路")}
          </div>
        </div>
        <div
          className={`toggle ${settings.enabled ? "active" : ""}`}
          onClick={() => persist({ ...settings, enabled: !settings.enabled })}
          role="switch"
          aria-checked={settings.enabled}
          tabIndex={0}
        />
      </div>

      {/* TTS 总开关 + 后端 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12, opacity: settings.enabled ? 1 : 0.55 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.ttsToggle", "语音播报")}</div>
            <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
              {t("notif.ttsToggleDesc", "启用后按类型配置朗读通知内容")}
            </div>
          </div>
          <div
            className={`toggle ${settings.tts_enabled ? "active" : ""}`}
            onClick={() => persist({ ...settings, tts_enabled: !settings.tts_enabled })}
            role="switch"
            aria-checked={settings.tts_enabled}
            tabIndex={0}
          />
        </div>
        {settings.tts_enabled && (
          <div style={{ display: "flex", gap: 8, alignItems: "center", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("notif.ttsBackendLabel", "播报后端")}
            </label>
            <select
              className="input"
              style={{ maxWidth: 220, padding: "4px 8px", fontSize: 12 }}
              value={settings.tts_backend}
              onChange={(e) => persist({ ...settings, tts_backend: e.target.value as TtsBackend })}
            >
              {TTS_BACKENDS.map((b) => (
                <option key={b} value={b}>{ttsBackendLabel(t, b)}</option>
              ))}
            </select>
          </div>
        )}
      </div>

      {/* 变量提示 */}
      <div className="glass-surface" style={{ padding: "12px 20px", display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
        <span style={{ fontSize: 12, fontWeight: 600 }}>{t("notif.varsHint", "可用变量")}</span>
        {TEMPLATE_VARS.map((v) => (
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

      {/* 按类型配置 */}
      <div style={{ display: "flex", flexDirection: "column", gap: 12, opacity: settings.enabled ? 1 : 0.55 }}>
        {NOTIF_TYPES.map((type) => {
          const ts = typeSetting(type);
          return (
            <div key={type} className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div style={{ fontSize: 13, fontWeight: 600 }}>{notifTypeLabel(t, type)}</div>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 12, padding: "4px 10px" }}
                  onClick={() => handleTest(type)}
                  disabled={!settings.enabled}
                >
                  {t("notif.test", "测试")}
                </button>
              </div>

              <div style={{ display: "flex", gap: 20, flexWrap: "wrap", alignItems: "center" }}>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("notif.fieldTts", "语音")}</span>
                  <div
                    className={`toggle ${ts.tts ? "active" : ""}`}
                    onClick={() => updateType(type, { tts: !ts.tts })}
                    role="switch"
                    aria-checked={ts.tts}
                    tabIndex={0}
                  />
                </div>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("notif.fieldPopup", "弹窗")}</span>
                  <div
                    className={`toggle ${ts.popup ? "active" : ""}`}
                    onClick={() => updateType(type, { popup: !ts.popup })}
                    role="switch"
                    aria-checked={ts.popup}
                    tabIndex={0}
                  />
                </div>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("notif.fieldForm", "呈现形式")}</span>
                  <select
                    className="input"
                    style={{ maxWidth: 160, padding: "4px 8px", fontSize: 12 }}
                    value={ts.form}
                    onChange={(e) => updateType(type, { form: e.target.value as NotifForm })}
                  >
                    {NOTIF_FORMS.map((f) => (
                      <option key={f} value={f}>{notifFormLabel(t, f)}</option>
                    ))}
                  </select>
                </div>
              </div>

              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("notif.fieldTemplate", "模板")}</label>
                <textarea
                  className="input"
                  style={{ fontSize: 12, fontFamily: "var(--font-mono, monospace)", minHeight: 48, resize: "vertical" }}
                  value={ts.template}
                  placeholder={t("notif.templatePlaceholder", "留空使用内置默认模板")}
                  onChange={(e) => updateType(type, { template: e.target.value })}
                />
              </div>
            </div>
          );
        })}
      </div>

      {/* 一键注入 hook */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.hookTitle", "一键注入通知 Hook")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("notif.hookDesc", "把通知 hook 注入 Claude Code / Codex 配置；任务完成与等待输入时触发通知")}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
            {t("notif.hookGroup", "分组")}
          </label>
          <select
            className="input"
            style={{ maxWidth: 200, padding: "4px 8px", fontSize: 12 }}
            value={hookGroup}
            onChange={(e) => setHookGroup(e.target.value)}
          >
            {groups.length === 0 && <option value="">{t("notif.hookNoGroupOpt", "无可用分组")}</option>}
            {groups.map((g) => (
              <option key={g.id} value={g.name}>{g.name}</option>
            ))}
          </select>
        </div>
        {HOOK_CLIENTS.map((client) => (
          <div key={client} style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
            <span style={{ fontSize: 12, fontWeight: 600, minWidth: 96 }}>{hookClientLabel(t, client)}</span>
            <button
              className="btn btn-primary"
              style={{ fontSize: 12, padding: "5px 12px" }}
              disabled={hookBusy || !hookGroup}
              onClick={() => handleInject(client, false)}
            >
              {t("notif.hookInject", "注入")}
            </button>
            <button
              className="btn btn-ghost"
              style={{ fontSize: 12, padding: "5px 12px" }}
              disabled={hookBusy || !hookGroup}
              onClick={() => handleInject(client, true)}
            >
              {t("notif.hookRemove", "移除")}
            </button>
          </div>
        ))}
      </div>

      {error && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>{error}</div>
      )}
      {message && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>{message}</div>
      )}
    </div>
  );
}
