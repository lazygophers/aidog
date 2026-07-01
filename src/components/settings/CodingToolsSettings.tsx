// ─── CLI 集成（AppSettings「CLI 集成」tab）────────────
// 两联动开关 + 语言选择：开关偏好存 app DB（scope=global, key=coding_tools_settings），
// 变化时后端按 diff 触发写外部文件（~/.claude/config.json 的 primaryApiKey / ~/.claude.json 的 hasCompletedOnboarding）。
// 语言字段写 ~/.claude/settings.json 的 language key（复用 claudeTab 同一 sync 路径：
// settingsApi.set(global, claude_code, …) + configApi.syncGroupSettings()）。
// 即时保存（无 unsaved state），不走 navGuard 离页拦截。

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { codingToolsSettingsApi, settingsApi, configApi, type CodingToolsSettings } from "../../services/api";
import { LANGUAGE_OPTIONS } from "../../services/claude-settings-schema";

const CLAUDE_CONFIG_KEY = "claude_code";

export function CodingToolsSettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<CodingToolsSettings>({
    apply_to_claude_plugin: false,
    skip_claude_onboarding: false,
  });
  const [language, setLanguage] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  // 写外部文件失败时的常驻错误态（与瞬时成功提示分离）：失败不自动消失，
  // 直到下次操作或修复，确保用户 100% 看到"开关未生效 + 真实原因"。
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  // 用户一旦操作过开关或语言（乐观翻转 + set 确认），mount 的初始 get() 即便晚到也不得覆盖。
  // 根因：React 19 StrictMode 致 mount effect 双跑，慢 get() 的 resolve 可能晚于用户操作，
  // 无条件 setSettings(get结果) 会把已确认的值改回 DB 旧值（几秒后自动变灰、无报错）。
  const dirtyRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    // 开关偏好
    codingToolsSettingsApi
      .get()
      .then((s) => {
        // 组件已卸载，或用户在 get 解析前已操作过 → 丢弃晚到结果，不覆盖。
        if (cancelled || dirtyRef.current) {
          return;
        }
        setSettings(s);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    // 语言字段（与 claudeTab 共享同一 claude_code config 对象的 language key）
    settingsApi
      .get("global", CLAUDE_CONFIG_KEY)
      .then((cfg) => {
        if (cancelled || dirtyRef.current) return;
        const lang = (cfg as Record<string, any> | null)?.language;
        if (typeof lang === "string") setLanguage(lang);
      })
      .catch(() => {
        // 语言读失败不阻塞开关加载：留空 option 占位即可
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleToggle = async (field: keyof CodingToolsSettings, next: boolean) => {
    if (busy) return;
    const prev = settings[field];
    // 标记用户已操作：此后任何晚到的 mount get() resolve 都不得覆盖本地值。
    dirtyRef.current = true;
    // 进入新操作前清掉旧的成功/错误态，避免陈旧提示干扰判断
    setMessage("");
    setError("");
    setSettings((s) => ({ ...s, [field]: next }));
    setBusy(true);
    try {
      const updated = await codingToolsSettingsApi.set({ [field]: next } as Partial<CodingToolsSettings>);
      setSettings(updated);
      setMessage(next ? t("codingTools.applied", "已应用") : t("codingTools.cleared", "已清除"));
    } catch (e: any) {
      // 写失败：回滚开关到失败前状态 + 常驻红色错误（含后端真实原因），不可错过
      setSettings((s) => ({ ...s, [field]: prev }));
      setError(t("codingTools.writeFailed", "写入失败") + ": " + String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleLanguageChange = async (next: string) => {
    if (busy || next === language) return;
    const prev = language;
    dirtyRef.current = true;
    setMessage("");
    setError("");
    setLanguage(next);
    setBusy(true);
    try {
      // 复用 claudeTab 的持久化路径：读全量 claude_code config → 改 language key →
      // 写回 DB → sync_group_settings 触发 ~/.claude/settings.json 与 per-group 文件重写。
      // 严禁自实现 fs 写（PRD D2 硬约束）。
      const cfg = (await settingsApi.get("global", CLAUDE_CONFIG_KEY)) as Record<string, any> | null;
      const updated = { ...(cfg ?? {}), language: next };
      await settingsApi.set("global", CLAUDE_CONFIG_KEY, updated);
      // best-effort sync，失败不阻塞 DB 已存值（下次任意 sync 路径会补偿）
      try {
        await configApi.syncGroupSettings();
      } catch (e) {
        console.error("sync_group_settings (language):", e);
      }
      setMessage(t("codingTools.applied", "已应用"));
    } catch (e: any) {
      setLanguage(prev);
      setError(t("codingTools.writeFailed", "写入失败") + ": " + String(e));
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
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("codingTools.cliIntegrationTitle", "CLI 集成")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 4 }}>
          {t("codingTools.introDesc", "以下开关会写入 Claude Code CLI 的本地配置文件，使扩展与 CLI 走 aidog 代理并跳过首启引导。")}
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
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("codingTools.applyPlugin.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("codingTools.applyPlugin.desc")}
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
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("codingTools.skipOnboarding.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("codingTools.skipOnboarding.desc")}
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

      {/* 语言选择：写 ~/.claude/settings.json 的 language key（复用 claudeTab sync 路径） */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        gap: 16,
      }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("codingTools.language.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("codingTools.language.desc")}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
            ~/.claude/settings.json · language
          </div>
        </div>
        <select
          className="input"
          style={{ fontSize: 13, minWidth: 180 }}
          value={language}
          onChange={(e) => handleLanguageChange(e.target.value)}
          disabled={busy}
        >
          {!language && <option value="">—</option>}
          {LANGUAGE_OPTIONS.map((lc) => (
            <option key={lc} value={lc}>{lc}</option>
          ))}
        </select>
      </div>

      {/* 常驻错误态：写外部文件失败时不可错过，红色 inline、不自动消失 */}
      {error && (
        <div
          role="alert"
          className="glass-surface"
          style={{
            padding: "14px 16px",
            border: "1px solid color-mix(in srgb, var(--color-danger) 50%, var(--border))",
            background: "color-mix(in srgb, var(--color-danger) 10%, var(--bg-floating))",
            color: "var(--color-danger)",
            fontSize: 13,
            lineHeight: 1.6,
            display: "flex",
            alignItems: "flex-start",
            gap: 10,
            wordBreak: "break-word",
          }}
        >
          <span style={{ fontSize: 16, lineHeight: 1.3, flexShrink: 0 }}>⚠</span>
          <span style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontWeight: 600, marginBottom: 4 }}>
              {t("codingTools.errorTitle", "开关未生效")}
            </div>
            <div style={{ fontFamily: "ui-monospace, monospace", fontSize: 12, opacity: 0.92 }}>
              {error}
            </div>
            <div style={{ marginTop: 6, fontSize: 12, opacity: 0.85 }}>
              {t("codingTools.errorHint", "请检查该文件的 JSON 格式与读写权限后重试。")}
            </div>
          </span>
        </div>
      )}

      {/* 瞬时成功提示（仅在无错误时显示） */}
      {message && !error && <div className="toast">{message}</div>}
    </div>
  );
}
