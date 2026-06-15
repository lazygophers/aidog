// ─── 系统通知设置面板（AppSettings「通知」tab；N3）────────────
// 消费 N1/N2 冻结的 services/api.ts 契约（notificationApi + 类型），只读不改。
// 提供：总开关 + TTS 总开关 + TTS 后端选择 + 按类型 {tts,popup,form,template} 编辑 +
//       变量提示 + 测试通知 + 「默认为所有分组注入通知 Hook」总开关（_aidog_hooks.enabled）。
// 单 group 注入按钮已删（API 仍保留: notificationApi.injectHooks/removeHooks），统一走总开关。

import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  notificationApi,
  scriptExecutorApi,
  type NotificationSettings as NotifSettings,
  type TypeSetting,
  type NotifType,
  type NotifForm,
  type TtsBackend,
} from "../../services/api";

// 与 api.ts 契约对齐（禁裸 string）。
const NOTIF_TYPES: NotifType[] = ["task_complete", "waiting_input", "error", "custom"];
const NOTIF_FORMS: NotifForm[] = ["full", "popup_only", "inbox_only", "sound_only"];
const TTS_BACKENDS: TtsBackend[] = ["cross_platform", "mac_say", "web_speech"];
const TEMPLATE_VARS = ["{project}", "{status}", "{time}", "{session}", "{group}"];

const DEFAULT_TYPE_SETTING: TypeSetting = { tts: true, popup: true, form: "full", template: "" };

// macOS 检测：webview UA 含 "Macintosh"。不引 @tauri-apps/plugin-os 依赖，纯前端判定。
// 仅 macOS 显示「打开系统通知设置」引导（Windows/Linux 通知一般默认可用，避免误导）。
const IS_MACOS = typeof navigator !== "undefined" && /Macintosh|Mac OS X/.test(navigator.userAgent);
// Ventura/13+ 新「系统设置」通知面板。
const MACOS_NOTIF_SETTINGS_URL = "x-apple.systempreferences:com.apple.Notifications-Settings.extension";
// 旧「系统偏好设置」通知面板（fallback）。
const MACOS_NOTIF_SETTINGS_URL_LEGACY = "x-apple.systempreferences:com.apple.preference.notifications";

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

export function NotificationSettingsTab({ onEnabledChanged }: { onEnabledChanged?: (enabled: boolean) => void }) {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<NotifSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [defaultHooks, setDefaultHooks] = useState(false);
  // 通知总开关关闭时，默认注入 Hook 开关强制 off 且禁用（通知未开则 hook 无意义）。
  const hooksDisabled = !settings.enabled;
  const [defaultHooksBusy, setDefaultHooksBusy] = useState(false);
  // uv 询问 modal：通知 hook 脚本为 Python（uv run --script / python3 执行）。注入前若 uv
  // 缺失且未持久化选择，弹此 modal 让用户「自动装 uv」或「用 python3」。resolver 在用户
  // 选择后兑现，门控 gate 继续注入。
  const [uvModal, setUvModal] = useState<{ resolve: (ok: boolean) => void } | null>(null);
  const [uvInstalling, setUvInstalling] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const s = await notificationApi.getSettings();
        setSettings(s);
      } catch (e) {
        console.error("load notification settings failed", e);
      }
      try {
        const enabled = await notificationApi.getDefaultHooksEnabled();
        setDefaultHooks(enabled);
      } catch (e) {
        console.error("load default hooks state failed", e);
      }
      setLoading(false);
    })();
  }, []);

  // settingsRef 始终持最新值，消除闭包 stale（async persist 未完成时新交互拿旧 settings 覆盖）。
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  // DB 写防抖定时器（template 连续输入合并为一次持久化）。
  const persistTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // persist 接收 updater（functional），基于 ref 最新值计算 next，避免闭包竞态覆盖。
  // 乐观 setSettings 立即刷 UI；DB 写防抖 200ms 合并；失败回滚到写前快照。
  const persist = async (updater: (prev: NotifSettings) => NotifSettings) => {
    const prev = settingsRef.current;
    const next = updater(prev);
    const prevEnabled = prev.enabled;
    setSettings(next);
    settingsRef.current = next;
    setError("");
    if (next.enabled !== prevEnabled) onEnabledChanged?.(next.enabled);
    if (persistTimer.current) clearTimeout(persistTimer.current);
    persistTimer.current = setTimeout(async () => {
      try {
        await notificationApi.setSettings(settingsRef.current);
      } catch (e) {
        console.error("set notification settings failed", e);
        // 回滚到写前快照，UI 与 DB 一致（防 UI 显示新值但 DB 存旧的"未生效"现象）。
        setSettings(prev);
        settingsRef.current = prev;
        setError(String(e));
      }
    }, 200);
  };

  const typeSetting = (type: NotifType): TypeSetting =>
    settings.per_type[type] ?? DEFAULT_TYPE_SETTING;

  const updateType = (type: NotifType, partial: Partial<TypeSetting>) =>
    persist(prev => ({
      ...prev,
      per_type: {
        ...prev.per_type,
        [type]: { ...(prev.per_type[type] ?? DEFAULT_TYPE_SETTING), ...partial },
      },
    }));

  const handleTest = async (type: NotifType) => {
    try {
      await notificationApi.testNotify(type);
      setMessage(t("notif.testSent", "测试通知已发送"));
    } catch (e) {
      console.error("test notify failed", e);
      setMessage(String(e));
    }
  };

  // 独立通道测试：绕过 dispatch 直接触发某通道，便于诊断（语音后端 / 弹窗权限 / 系统提示音）。
  const handleTestTts = async (type: NotifType) => {
    try {
      const text = `${notifTypeLabel(t, type)} ${t("notif.testTtsContent", "测试播报")}`;
      await notificationApi.testTts(text);
    } catch (e) {
      console.error("test tts failed", e);
      setMessage(String(e));
    }
  };
  const handleTestPopup = async (type: NotifType) => {
    try {
      await notificationApi.testPopup(notifTypeLabel(t, type), t("notif.testPopupBody", "测试弹窗"));
    } catch (e) {
      console.error("test popup failed", e);
      setMessage(String(e));
    }
  };
  const handleTestBeep = async () => {
    try {
      await notificationApi.testBeep();
    } catch (e) {
      console.error("test beep failed", e);
      setMessage(String(e));
    }
  };

  // 注入前确保脚本执行器就绪：uv 可用 → 直接放行；否则弹 modal。返回 false 表示用户取消注入。
  const ensureExecutorReady = async (): Promise<boolean> => {
    try {
      const ok = await scriptExecutorApi.checkUv();
      if (ok) return true;
    } catch (e) {
      console.error("check uv failed", e);
    }
    // uv 缺失 → 弹 modal 等用户选择。
    return new Promise<boolean>((resolve) => setUvModal({ resolve }));
  };

  const handleUvInstall = async () => {
    if (!uvModal) return;
    setUvInstalling(true);
    try {
      await scriptExecutorApi.installUv();
      setMessage(t("notif.uvInstalled", "uv 安装完成"));
      uvModal.resolve(true);
    } catch (e) {
      console.error("install uv failed", e);
      setMessage(t("notif.uvInstallFailed", "uv 安装失败，将使用 python3"));
      // 安装失败 → 退回 python3 仍可生成脚本。
      try { await scriptExecutorApi.setExecutor("python3"); } catch { /* best-effort */ }
      uvModal.resolve(true);
    }
    setUvInstalling(false);
    setUvModal(null);
  };

  const handleUvUsePython = async () => {
    if (!uvModal) return;
    try {
      await scriptExecutorApi.setExecutor("python3");
    } catch (e) {
      console.error("set python3 executor failed", e);
    }
    uvModal.resolve(true);
    setUvModal(null);
  };

  const handleUvCancel = () => {
    if (!uvModal) return;
    uvModal.resolve(false);
    setUvModal(null);
  };

  // 打开 macOS 系统通知设置面板。新 scheme 失败时回退旧 scheme。
  const handleOpenNotifSettings = async () => {
    try {
      await openUrl(MACOS_NOTIF_SETTINGS_URL);
    } catch (e) {
      console.error("open notification settings (new scheme) failed", e);
      try {
        await openUrl(MACOS_NOTIF_SETTINGS_URL_LEGACY);
      } catch (e2) {
        console.error("open notification settings (legacy scheme) failed", e2);
        setMessage(String(e2));
      }
    }
  };

  const handleToggleDefaultHooks = async () => {
    const next = !defaultHooks;
    // 开启会为全分组生成 Python hook 脚本 → 先确保执行器就绪。
    if (next && !(await ensureExecutorReady())) return;
    setDefaultHooksBusy(true);
    setDefaultHooks(next);
    try {
      await notificationApi.setDefaultHooksEnabled(next);
      setMessage(next
        ? t("notif.defaultHooksOn", "已为所有分组注入通知 hook")
        : t("notif.defaultHooksOff", "已移除所有分组的通知 hook"));
    } catch (e) {
      console.error("set default hooks failed", e);
      setDefaultHooks(!next);
      setMessage(String(e));
    }
    setDefaultHooksBusy(false);
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
          onClick={() => persist(prev => ({ ...prev, enabled: !prev.enabled }))}
          role="switch"
          aria-checked={settings.enabled}
          tabIndex={0}
        />
      </div>

      {/* macOS 授权引导：通知静默不出现时，一键打开系统通知设置允许 aidog。仅 macOS 显示。 */}
      {IS_MACOS && (
        <div
          className="glass-surface"
          style={{ padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap" }}
        >
          <div>
            <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.permGuideTitle", "没收到系统通知？")}</div>
            <div className="text-secondary" style={{ fontSize: 12, marginTop: 2, lineHeight: 1.5 }}>
              {t("notif.permGuideDesc", "可能需在系统设置中允许 aidog 发送通知。")}
            </div>
          </div>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12, padding: "6px 12px", whiteSpace: "nowrap" }}
            onClick={handleOpenNotifSettings}
          >
            {t("notif.permGuideButton", "打开系统通知设置")}
          </button>
        </div>
      )}

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
            onClick={() => persist(prev => ({ ...prev, tts_enabled: !prev.tts_enabled }))}
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
              onChange={(e) => persist(prev => ({ ...prev, tts_backend: e.target.value as TtsBackend }))}
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
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <div style={{ fontSize: 13, fontWeight: 600 }}>{notifTypeLabel(t, type)}</div>
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 12, padding: "4px 8px" }}
                    onClick={() => handleTestTts(type)}
                    disabled={!settings.enabled}
                    title={t("notif.testTtsTip", "仅测语音播报")}
                    aria-label={t("notif.testTtsTip", "仅测语音播报")}
                  >
                    🔊
                  </button>
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 12, padding: "4px 8px" }}
                    onClick={() => handleTestPopup(type)}
                    disabled={!settings.enabled}
                    title={t("notif.testPopupTip", "仅测系统弹窗")}
                    aria-label={t("notif.testPopupTip", "仅测系统弹窗")}
                  >
                    🪟
                  </button>
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 12, padding: "4px 8px" }}
                    onClick={handleTestBeep}
                    disabled={!settings.enabled}
                    title={t("notif.testBeepTip", "仅测系统提示音")}
                    aria-label={t("notif.testBeepTip", "仅测系统提示音")}
                  >
                    🔔
                  </button>
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 12, padding: "4px 10px" }}
                    onClick={() => handleTest(type)}
                    disabled={!settings.enabled}
                  >
                    {t("notif.test", "测试")}
                  </button>
                </div>
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

      {/* 默认注入总开关：控制基线 _aidog_hooks.enabled，全分组生效 */}
      <div
        className="glass-surface"
        style={{ padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center" }}
      >
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.defaultHooksTitle", "默认为所有分组注入通知 Hook")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("notif.defaultHooksDesc", "开启后所有分组自动带 Claude Code hooks 与 Codex notify，无需逐个手动注入")}
            {hooksDisabled && <span style={{ marginLeft: 4 }}>· {t("notif.defaultHooksDisabledHint", "需先开启通知")}</span>}
          </div>
        </div>
        <div
          className={`toggle ${defaultHooks && !hooksDisabled ? "active" : ""}`}
          style={hooksDisabled ? { opacity: 0.5, cursor: "not-allowed" } : undefined}
          onClick={() => { if (!hooksDisabled && !defaultHooksBusy) handleToggleDefaultHooks(); }}
          role="switch"
          aria-checked={defaultHooks && !hooksDisabled}
          aria-disabled={hooksDisabled}
          aria-label={t("notif.defaultHooksTitle", "默认为所有分组注入通知 Hook")}
          tabIndex={hooksDisabled ? -1 : 0}
        />
      </div>

      {error && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>{error}</div>
      )}
      {message && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>{message}</div>
      )}

      {/* uv 询问 modal：通知 hook 脚本为 Python，uv 缺失时让用户选自动安装或回退 python3 */}
      {uvModal && (
        <div
          style={{
            position: "fixed", inset: 0, zIndex: 1000,
            display: "flex", alignItems: "center", justifyContent: "center",
            background: "rgba(0,0,0,0.45)",
          }}
          role="dialog"
          aria-modal="true"
        >
          <div className="glass-elevated" style={{ maxWidth: 420, padding: "20px 24px", display: "flex", flexDirection: "column", gap: 14 }}>
            <div style={{ fontSize: 15, fontWeight: 600 }}>{t("notif.uvModalTitle", "未检测到 uv")}</div>
            <div className="text-secondary" style={{ fontSize: 13, lineHeight: 1.5 }}>
              {t("notif.uvModalDesc", "通知 hook 脚本为 Python（PEP723），推荐用 uv 运行以隔离依赖。是否自动安装 uv？否则将使用系统 python3。")}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", flexWrap: "wrap" }}>
              <button
                className="btn btn-ghost"
                style={{ fontSize: 12, padding: "6px 12px" }}
                onClick={handleUvCancel}
                disabled={uvInstalling}
              >
                {t("notif.uvModalCancel", "取消")}
              </button>
              <button
                className="btn btn-ghost"
                style={{ fontSize: 12, padding: "6px 12px" }}
                onClick={handleUvUsePython}
                disabled={uvInstalling}
              >
                {t("notif.uvModalUsePython", "用 python3")}
              </button>
              <button
                className="btn btn-primary"
                style={{ fontSize: 12, padding: "6px 12px" }}
                onClick={handleUvInstall}
                disabled={uvInstalling}
              >
                {uvInstalling ? t("notif.uvModalInstalling", "安装中…") : t("notif.uvModalInstall", "自动安装 uv")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
