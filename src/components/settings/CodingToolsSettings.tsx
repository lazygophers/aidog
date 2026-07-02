// ─── CLI 集成（AppSettings「CLI 集成」tab）────────────
// 两联动开关 + 语言选择：开关偏好存 app DB（scope=global, key=coding_tools_settings），
// 变化时后端按 diff 触发写外部文件（~/.claude/config.json 的 primaryApiKey / ~/.claude.json 的 hasCompletedOnboarding）。
// 语言字段写 ~/.claude/settings.json 的 language key（复用 claudeTab 同一 sync 路径：
// settingsApi.set(global, claude_code, …) + configApi.syncGroupSettings()）。
// 即时保存（无 unsaved state），不走 navGuard 离页拦截。

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { codingToolsSettingsApi, settingsApi, configApi, middlewareApi, type CodingToolsSettings, type MiddlewareRule } from "../../services/api";
import { LANGUAGE_OPTIONS } from "../../services/claude-settings-schema";

const CLAUDE_CONFIG_KEY = "claude_code";
// 内置·日期格式改写防检测 规则名（与 schema.rs builtin_rule_specs 一致）。
// 该开关镜像 middleware_rule.enabled，不写 coding_tools_settings。
const DATE_REWRITE_RULE_NAME = "内置·日期格式改写防检测";

export function CodingToolsSettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<CodingToolsSettings>({
    apply_to_claude_plugin: false,
    skip_claude_onboarding: false,
  });
  const [language, setLanguage] = useState<string>("");
  // 日期改写防检测开关：镜像 middleware_rule.enabled（不写 coding_tools_settings）。
  // null = 加载中/规则未找到，true/false = 规则 enabled。
  const [dateRewriteEnabled, setDateRewriteEnabled] = useState<boolean | null>(null);
  const [dateRewriteRuleId, setDateRewriteRuleId] = useState<number | null>(null);
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
    // 日期改写规则开关态：从 middleware_rule 读 enabled（按 name filter 内置规则）
    middlewareApi
      .listRules()
      .then((rules) => {
        if (cancelled || dirtyRef.current) return;
        const r = rules.find((x: MiddlewareRule) => x.name === DATE_REWRITE_RULE_NAME && x.is_builtin);
        if (r) {
          setDateRewriteRuleId(r.id);
          setDateRewriteEnabled(!!r.enabled);
        }
      })
      .catch(() => {
        // 规则读取失败不阻塞开关加载：开关保持 null（不渲染翻转态）
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

  // 切「日期格式改写防检测」开关：镜像 middleware_rule.enabled（非 coding_tools_settings）。
  // UpdateMiddlewareRule 是全量覆盖，须带原规则全部字段，仅翻 enabled。
  const handleDateRewriteToggle = async (next: boolean) => {
    if (busy || dateRewriteRuleId == null) return;
    const prev = dateRewriteEnabled;
    dirtyRef.current = true;
    setMessage("");
    setError("");
    setDateRewriteEnabled(next);
    setBusy(true);
    try {
      // 重新 list 拿最新规则（避免本地态与 DB 漂移），再 update enabled。
      const rules = await middlewareApi.listRules();
      const r = rules.find((x: MiddlewareRule) => x.name === DATE_REWRITE_RULE_NAME && x.is_builtin);
      if (!r) throw new Error("builtin date-rewrite rule not found");
      const updated = await middlewareApi.updateRule({
        id: r.id,
        name: r.name,
        description: r.description,
        rule_type: r.rule_type,
        scope: r.scope,
        scope_ref: r.scope_ref,
        match_type: r.match_type,
        pattern: r.pattern,
        action: r.action,
        config: r.config,
        priority: r.priority,
        enabled: next,
        is_builtin: r.is_builtin,
      });
      setDateRewriteRuleId(updated.id);
      setDateRewriteEnabled(!!updated.enabled);
      setMessage(next ? t("codingTools.applied", "已应用") : t("codingTools.cleared", "已清除"));
    } catch (e: any) {
      setDateRewriteEnabled(prev);
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

      {/* 开关 3：日期格式改写防检测（镜像 middleware_rule.enabled，非 coding_tools_settings） */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div style={{ flex: 1, minWidth: 0, paddingRight: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("codingTools.dateRewrite.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("codingTools.dateRewrite.desc")}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
            middleware · redaction · YYYY/MM/DD → YYYY-MM-DD
          </div>
        </div>
        <div
          className={`toggle ${dateRewriteEnabled ? "active" : ""}`}
          onClick={() => dateRewriteEnabled != null && handleDateRewriteToggle(!dateRewriteEnabled)}
          role="switch"
          aria-checked={dateRewriteEnabled ?? false}
          aria-disabled={dateRewriteEnabled == null}
          tabIndex={dateRewriteEnabled == null ? -1 : 0}
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
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("codingTools.language.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("codingTools.language.desc")}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
            ~/.claude/settings.json · language
          </div>
        </div>
        <select
          className="input"
          style={{ fontSize: 13 }}
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
