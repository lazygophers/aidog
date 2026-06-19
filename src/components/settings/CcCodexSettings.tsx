// ─── Claude Code / Codex 联动开关（AppSettings「CC / Codex」tab）────────────
// 两联动开关：开关偏好存 app DB（scope=global, key=cc_codex_settings），
// 变化时后端按 diff 触发写外部文件（~/.claude/config.json 的 primaryApiKey / ~/.claude.json 的 hasCompletedOnboarding）。
// 即时保存（无 unsaved state），不走 navGuard 离页拦截。

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ccCodexSettingsApi, type CcCodexSettings } from "../../services/api";

export function CcCodexSettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<CcCodexSettings>({
    apply_to_claude_plugin: false,
    skip_claude_onboarding: false,
  });
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    ccCodexSettingsApi
      .get()
      .then((s) => setSettings(s))
      .catch((e) => setMessage(String(e)))
      .finally(() => setLoading(false));
  }, []);

  const handleToggle = async (field: keyof CcCodexSettings, next: boolean) => {
    if (busy) return;
    const prev = settings[field];
    setSettings((s) => ({ ...s, [field]: next }));
    setBusy(true);
    try {
      const updated = await ccCodexSettingsApi.set({ [field]: next } as Partial<CcCodexSettings>);
      setSettings(updated);
      setMessage(next ? t("ccCodex.applied", "已应用") : t("ccCodex.cleared", "已清除"));
    } catch (e: any) {
      // 回滚
      setSettings((s) => ({ ...s, [field]: prev }));
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (loading) {
    return <div style={{ padding: 20, color: "var(--text-secondary)" }}>{t("common.loading", "加载中…")}</div>;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* 说明卡片 */}
      <div className="glass-surface" style={{ padding: "16px 20px" }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("ccCodex.introTitle", "Claude Code / Codex 联动")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 4 }}>
          {t("ccCodex.introDesc", "以下开关会写入 Claude Code CLI 的本地配置文件，使扩展与 CLI 走 aidog 代理并跳过首启引导。")}
        </div>
      </div>

      {/* 开关 1：应用到 Claude Code 插件 */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div style={{ flex: 1, minWidth: 0, paddingRight: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("ccCodex.applyPlugin.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("ccCodex.applyPlugin.desc")}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
            ~/.claude/config.json · primaryApiKey="any"
          </div>
        </div>
        <div
          className={`toggle ${settings.apply_to_claude_plugin ? "active" : ""}`}
          onClick={() => handleToggle("apply_to_claude_plugin", !settings.apply_to_claude_plugin)}
          role="switch"
          aria-checked={settings.apply_to_claude_plugin}
          tabIndex={0}
        />
      </div>

      {/* 开关 2：跳过 Claude Code 安装确认 */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div style={{ flex: 1, minWidth: 0, paddingRight: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("ccCodex.skipOnboarding.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("ccCodex.skipOnboarding.desc")}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
            ~/.claude.json · hasCompletedOnboarding=true
          </div>
        </div>
        <div
          className={`toggle ${settings.skip_claude_onboarding ? "active" : ""}`}
          onClick={() => handleToggle("skip_claude_onboarding", !settings.skip_claude_onboarding)}
          role="switch"
          aria-checked={settings.skip_claude_onboarding}
          tabIndex={0}
        />
      </div>

      {message && <div className="toast">{message}</div>}
    </div>
  );
}
