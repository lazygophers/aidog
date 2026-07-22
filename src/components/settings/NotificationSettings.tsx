// ─── 系统通知设置面板（AppSettings「通知」tab；N3）────────────
// 消费 N1/N2 冻结的 services/api.ts 契约（notificationApi + 类型），只读不改。
// 提供：总开关 + TTS 总开关 + TTS 后端选择 + 通道独立测试按钮 +
//       「默认为所有分组注入通知 Hook」总开关（_aidog_hooks.enabled）+ 逐 Hook 事件触发（NotificationEventList）。
// 「按类型配置」已移除（仅保留逐 Hook 事件触发）；单 group 注入按钮已删（API 仍保留: injectHooks/removeHooks）。

import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  notificationApi,
  scriptExecutorApi,
  type NotificationSettings as NotifSettings,
  type EventSetting,
  type TtsBackend,
} from "../../services/api";
import { NotificationEventList } from "./NotificationEventList";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";

// 与 api.ts 契约对齐（禁裸 string）。
const TTS_BACKENDS: TtsBackend[] = ["cross_platform", "mac_say", "web_speech"];

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
  per_event: {},
  inbox_retention_days: 7,
};

function ttsBackendLabel(t: ReturnType<typeof useTranslation>["t"], b: TtsBackend): string {
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

  // N2：逐事件配置更新（写 settings.per_event[event]）。组件传完整 EventSetting（含展示态兜底）。
  const updateEvent = (event: string, setting: EventSetting) =>
    persist(prev => ({
      ...prev,
      per_event: { ...(prev.per_event ?? {}), [event]: setting },
    }));

  const handleTest = async () => {
    try {
      await notificationApi.testNotify("task_complete");
      setMessage(t("notif.testSent", "测试通知已发送"));
    } catch (e) {
      console.error("test notify failed", e);
      setMessage(String(e));
    }
  };

  // 独立通道测试：绕过 dispatch 直接触发某通道，便于诊断（语音后端 / 弹窗权限 / 系统提示音）。
  const handleTestTts = async () => {
    try {
      await notificationApi.testTts(t("notif.testTtsContent", "测试播报"));
    } catch (e) {
      console.error("test tts failed", e);
      setMessage(String(e));
    }
  };
  const handleTestPopup = async () => {
    try {
      await notificationApi.testPopup(t("notif.testPopupTitle", "测试通知"), t("notif.testPopupBody", "测试弹窗"));
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
        <Switch
          checked={settings.enabled}
          onCheckedChange={() => persist(prev => ({ ...prev, enabled: !prev.enabled }))}
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
          <Button variant="ghost"
            
            style={{ fontSize: 12, padding: "6px 12px", whiteSpace: "nowrap" }}
            onClick={handleOpenNotifSettings}
          >
            {t("notif.permGuideButton", "打开系统通知设置")}
          </Button>
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
          <Switch
            checked={settings.tts_enabled}
            onCheckedChange={() => persist(prev => ({ ...prev, tts_enabled: !prev.tts_enabled }))}
          />
        </div>
        {settings.tts_enabled && (
          <div style={{ display: "flex", gap: 8, alignItems: "center", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("notif.ttsBackendLabel", "播报后端")}
            </label>
            <Select
              
              
              value={settings.tts_backend}
              onValueChange={(v) => persist(prev => ({ ...prev, tts_backend: v as TtsBackend }))}
            >
<SelectTrigger style={{ maxWidth: 220, padding: "4px 8px", fontSize: 12 }}><SelectValue/></SelectTrigger>
<SelectContent>
              {TTS_BACKENDS.map((b) => (
                <SelectItem key={b} value={b}>{ttsBackendLabel(t, b)}</SelectItem>
              ))}
            </SelectContent>
</Select>
          </div>
        )}
      </div>

      {/* 通道独立测试：绕过 dispatch 直接触发某通道，便于诊断（语音后端 / 弹窗权限 / 系统提示音 / 端到端） */}
      <div
        className="glass-surface"
        style={{ padding: "12px 20px", display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap", opacity: settings.enabled ? 1 : 0.55 }}
      >
        <span style={{ fontSize: 12, fontWeight: 600 }}>{t("notif.testChannels", "通道测试")}</span>
        <Button variant="ghost"
          
          style={{ fontSize: 12, padding: "4px 10px" }}
          onClick={handleTestTts}
          disabled={!settings.enabled}
          title={t("notif.testTtsTip", "仅测语音播报")}
        >
          🔊 {t("notif.testTtsLabel", "语音")}
        </Button>
        <Button variant="ghost"
          
          style={{ fontSize: 12, padding: "4px 10px" }}
          onClick={handleTestPopup}
          disabled={!settings.enabled}
          title={t("notif.testPopupTip", "仅测系统弹窗")}
        >
          🪟 {t("notif.testPopupLabel", "弹窗")}
        </Button>
        <Button variant="ghost"
          
          style={{ fontSize: 12, padding: "4px 10px" }}
          onClick={handleTestBeep}
          disabled={!settings.enabled}
          title={t("notif.testBeepTip", "仅测系统提示音")}
        >
          🔔 {t("notif.testBeepLabel", "提示音")}
        </Button>
        <Button variant="ghost"
          
          style={{ fontSize: 12, padding: "4px 10px" }}
          onClick={handleTest}
          disabled={!settings.enabled}
        >
          {t("notif.test", "测试")}
        </Button>
      </div>


      {/* 收件箱历史自动清理：开关（不清理↔保留 N 天）+ 天数输入。后端硬删过期行。 */}
      {(() => {
        const retention = settings.inbox_retention_days ?? 7;
        const cleanupOn = retention > 0;
        return (
          <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.retentionTitle", "通知历史自动清理")}</div>
                <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
                  {t("notif.retentionDesc", "超过保留天数的通知历史将被永久删除；关闭则永久保留")}
                </div>
              </div>
              {/* 关 → 0（不清理）；开 → 回 7 天默认。 */}
              <Switch
                checked={cleanupOn}
                onCheckedChange={() => persist(prev => ({ ...prev, inbox_retention_days: cleanupOn ? 0 : 7 }))}
                aria-label={t("notif.retentionTitle", "通知历史自动清理")}
              />
            </div>
            {cleanupOn && (
              <div style={{ display: "flex", gap: 8, alignItems: "center", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
                <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                  {t("notif.retentionDaysLabel", "保留天数")}
                </label>
                <Input
                  type="number"
                  
                  min={1}
                  max={3650}
                  style={{ maxWidth: 120, padding: "4px 8px", fontSize: 12 }}
                  value={retention}
                  onChange={(e) => {
                    // 限 [1,3650]；非法输入回退 1（0 仅由开关切「不清理」）。
                    const n = Math.min(3650, Math.max(1, Math.floor(Number(e.target.value) || 1)));
                    persist(prev => ({ ...prev, inbox_retention_days: n }));
                  }}
                />
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("notif.retentionDaysUnit", "天")}</span>
              </div>
            )}
          </div>
        );
      })()}

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
        <Switch
          checked={defaultHooks && !hooksDisabled}
          disabled={hooksDisabled || defaultHooksBusy}
          onCheckedChange={() => handleToggleDefaultHooks()}
          aria-label={t("notif.defaultHooksTitle", "默认为所有分组注入通知 Hook")}
        />
      </div>

      {/* N2：逐 hook 事件触发配置（仅 claude_code；通知总开关关时禁用整区） */}
      <NotificationEventList
        perEvent={settings.per_event}
        disabled={hooksDisabled}
        onUpdate={updateEvent}
      />

      {error && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>{error}</div>
      )}
      {message && (
        <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>{message}</div>
      )}

      {/* uv 询问 modal：通知 hook 脚本为 Python，uv 缺失时让用户选自动安装或回退 python3。
          shadcn Dialog (Radix Portal) 满足 createPortal(document.body) 居中规则。 */}
      <Dialog open={uvModal !== null} onOpenChange={(o) => { if (!o) handleUvCancel(); }}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 420, padding: "20px 24px" }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 15, fontWeight: 600 }}>{t("notif.uvModalTitle", "未检测到 uv")}</DialogTitle>
            <DialogDescription className="text-secondary" style={{ fontSize: 13, lineHeight: 1.5 }}>
              {t("notif.uvModalDesc", "通知 hook 脚本为 Python（PEP723），推荐用 uv 运行以隔离依赖。是否自动安装 uv？否则将使用系统 python3。")}
            </DialogDescription>
          </DialogHeader>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", flexWrap: "wrap" }}>
            <Button variant="ghost"
              style={{ fontSize: 12, padding: "6px 12px" }}
              onClick={handleUvCancel}
              disabled={uvInstalling}
            >
              {t("notif.uvModalCancel", "取消")}
            </Button>
            <Button variant="ghost"
              style={{ fontSize: 12, padding: "6px 12px" }}
              onClick={handleUvUsePython}
              disabled={uvInstalling}
            >
              {t("notif.uvModalUsePython", "用 python3")}
            </Button>
            <Button variant="default"
              style={{ fontSize: 12, padding: "6px 12px" }}
              onClick={handleUvInstall}
              disabled={uvInstalling}
            >
              {uvInstalling ? t("notif.uvModalInstalling", "安装中…") : t("notif.uvModalInstall", "自动安装 uv")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
