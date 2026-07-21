// ─── CLI 集成（AppSettings「Coding 设置」tab）────────────
// 即时保存（无 unsaved state，不走 navGuard）。5 项开关写不同外部文件：
//   apply_to_claude_plugin  → ~/.claude/config.json primaryApiKey
//   skip_claude_onboarding  → ~/.claude.json hasCompletedOnboarding
//   日期改写防检测          → 镜像 middleware_rule.enabled（非 coding_tools_settings）
//   language                → ~/.claude/settings.json language key（复用 claudeTab sync 路径）
//   努力级别                → 单值双写：claude 顶层 effortLevel + codex model_reasoning_effort；读时 claude 优先
//   自动压缩窗口            → 单值双写：claude env.CLAUDE_CODE_AUTO_COMPACT_WINDOW（数字）+
//                             codex model_auto_compact_token_limit（字符串）；读时 claude 优先
//   代理设置                → 仅 Claude：env.HTTP_PROXY/HTTPS_PROXY/ALL_PROXY/NO_PROXY（4 键）。
//                             Codex config.toml 无原生 proxy 字段（官方 issue #4242/#6060 未实现），
//                             codex 侧由「复制启动命令」注入代理 env（另见 Groups 启动命令复制点）。
// writeClaudeConfigField（services/api/settings.ts）统一「读全量 → 改 → 写回 → sync」路径，
// language / compact-claude 共用。runCommit 统一 5 项「乐观翻转 → persist → 回滚/提示」模板。
// dirtyRef 防 React 19 StrictMode 双 mount 下慢 get() resolve 覆盖用户已操作值（见 runCommit）。

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  codingToolsSettingsApi,
  codexApi,
  middlewareApi,
  settingsApi,
  writeClaudeConfigField,
  type CodingToolsSettings,
  type MiddlewareRule,
} from "../../services/api";
import { LANGUAGE_GROUPS } from "../../services/claude-settings-schema";
import { Toggle } from "./editors";

// 内置·日期格式改写防检测 规则名（与 schema.rs builtin_rule_specs 一致）。
// 该开关镜像 middleware_rule.enabled，不写 coding_tools_settings。
const DATE_REWRITE_RULE_NAME = "内置·日期格式改写防检测";

// 努力级别枚举。claude 顶层 effortLevel 原生档位 low/medium/high/xhigh，
// codex model_reasoning_effort 原生 minimal/low/medium/high/xhigh；max 为 claude env 档位，
// 双写时若任一侧不识别该值由对应 CLI 自行忽略/回落。默认 medium。
const EFFORT_OPTIONS = ["low", "medium", "high", "xhigh", "max"] as const;
const EFFORT_DEFAULT = "medium";

// 代理设置：4 个出站代理 env 键（写 claude settings.json env 段，启动注入 process env）。
// key = 本地 draft 字段名；value = claude env 键名。Codex 无 config 级 proxy（issue #4242/#6060）。
const PROXY_ENV_KEYS = {
  http: "HTTP_PROXY",
  https: "HTTPS_PROXY",
  all: "ALL_PROXY",
  no: "NO_PROXY",
} as const;
type ProxyKey = keyof typeof PROXY_ENV_KEYS;
const PROXY_KEYS: ProxyKey[] = ["http", "https", "all", "no"];
const emptyProxy = (): Record<ProxyKey, string> => ({ http: "", https: "", all: "", no: "" });

// 自动压缩窗口：原始 token 字符串 → K 输入值（180000 → "180"，1500 → "1.5"）。
function tokensToDraft(s: string): string {
  if (!s) return "";
  const n = Number(s);
  if (!Number.isFinite(n)) return "";
  const k = n / 1000;
  return Number.isInteger(k) ? String(k) : String(k);
}
// K 输入 → 原始 token 字符串。空串 = 清除；非法返 null（caller 报错）。
function parseCompactInput(s: string): string | null {
  const m = s.trim();
  if (!m) return "";
  if (!/^\d+(\.\d+)?$/.test(m)) return null;
  return String(Math.round(Number(m) * 1000));
}

// 3 个同构开关卡片：标题 + 描述 + 落点 hint + Toggle。抽出消除 JSX 复制。
function ToggleCard({
  titleKey, descKey, hint, active, disabled, onToggle,
}: {
  titleKey: string;
  descKey: string;
  hint: string;
  active: boolean;
  disabled?: boolean;
  onToggle: (next: boolean) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="glass-surface" style={{
      padding: "16px 20px",
      display: "flex",
      justifyContent: "space-between",
      alignItems: "center",
    }}>
      <div style={{ flex: 1, minWidth: 0, paddingRight: 16 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t(titleKey)}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>{t(descKey)}</div>
        <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
          {hint}
        </div>
      </div>
      <Toggle
        active={active}
        onChange={disabled ? () => {} : () => onToggle(!active)}
      />
    </div>
  );
}

export function CodingToolsSettingsTab() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<CodingToolsSettings>({
    apply_to_claude_plugin: false,
    skip_claude_onboarding: false,
  });
  const [language, setLanguage] = useState<string>("");
  // 努力级别：claude 顶层 effortLevel 优先，codex model_reasoning_effort 回落。
  const [effort, setEffort] = useState<string>("");
  // 自动压缩窗口：draft = 用户编辑中的 K 值；applied = 已落盘原始 token 字符串。
  const [compactDraft, setCompactDraft] = useState<string>("");
  const [compactApplied, setCompactApplied] = useState<string>("");
  // 日期改写开关：null = 加载中/规则未找到（Toggle 不响应）；true/false = 镜像 rule.enabled。
  const [dateRewriteEnabled, setDateRewriteEnabled] = useState<boolean | null>(null);
  const [dateRewriteRuleId, setDateRewriteRuleId] = useState<number | null>(null);
  // 代理设置 draft（用户编辑中）；applied ref 用于 onBlur diff 判定是否需要提交。
  const [proxyDraft, setProxyDraft] = useState<Record<ProxyKey, string>>(emptyProxy);
  const proxyAppliedRef = useRef<Record<ProxyKey, string>>(emptyProxy());
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  // 写外部文件失败的常驻错误态（与瞬时成功提示分离）：失败不自动消失，
  // 直到下次操作或修复，确保用户 100% 看到「开关未生效 + 真实原因」。
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  // 用户一旦操作过任一项，mount 的初始 get() 即便晚到也不得覆盖。
  const dirtyRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    codingToolsSettingsApi
      .get()
      .then((s) => {
        if (cancelled || dirtyRef.current) return;
        setSettings(s);
      })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });

    // language（claude_code.language）+ compact（claude env 优先，codex 回落）。
    Promise.all([
      settingsApi.get("global", "claude_code").catch(() => null),
      codexApi.read().catch(() => null),
    ]).then(([cfg, cx]) => {
      if (cancelled || dirtyRef.current) return;
      const obj = cfg as Record<string, any> | null;
      const lang = obj?.language;
      if (typeof lang === "string") setLanguage(lang);
      const effortVal = (obj as any)?.effortLevel;
      const codexEffort = (cx as Record<string, any> | null)?.model_reasoning_effort;
      const eff = typeof effortVal === "string" && effortVal
        ? effortVal
        : typeof codexEffort === "string" && codexEffort ? codexEffort : "";
      setEffort(eff || EFFORT_DEFAULT);
      const envVal = (obj?.env as Record<string, any> | undefined)?.CLAUDE_CODE_AUTO_COMPACT_WINDOW;
      const codexVal = (cx as Record<string, any> | null)?.model_auto_compact_token_limit;
      const compact = envVal != null && envVal !== ""
        ? String(envVal)
        : codexVal != null && codexVal !== "" ? String(codexVal) : "";
      if (compact) {
        setCompactApplied(compact);
        setCompactDraft(tokensToDraft(compact));
      }
      // 代理设置：从 claude env 读 4 键（空值留空）。
      const env = (obj?.env as Record<string, any> | undefined);
      const loaded = emptyProxy();
      for (const k of PROXY_KEYS) {
        const v = env?.[PROXY_ENV_KEYS[k]];
        if (typeof v === "string") loaded[k] = v;
      }
      setProxyDraft(loaded);
      proxyAppliedRef.current = loaded;
    }).catch(() => {
      // 读失败不阻塞开关加载：language 留空，compact 留空
    });

    // 日期改写规则态：从 middleware_rule 读 enabled。
    middlewareApi.listRules()
      .then((rules) => {
        if (cancelled || dirtyRef.current) return;
        const r = rules.find((x: MiddlewareRule) => x.name === DATE_REWRITE_RULE_NAME && x.is_builtin);
        if (r) {
          setDateRewriteRuleId(r.id);
          setDateRewriteEnabled(!!r.enabled);
        }
      })
      .catch(() => {
        // 规则读取失败不阻塞开关加载：Toggle 保持 null（不响应）
      });

    return () => { cancelled = true; };
  }, []);

  // 统一 commit：乐观翻转 → persist → 失败回滚 + 常驻错误，成功 toast。
  // persist 返回 true=apply / false=clear（决定 toast 文案）；抛错触发 revert。
  // revert lambda 引用闭包内当前值（= 翻转前的 prev），闭包绑定 render 故稳定。
  const runCommit = async (
    optimistic: () => void,
    revert: () => void,
    persist: () => Promise<boolean>,
  ): Promise<void> => {
    if (busy) return;
    dirtyRef.current = true;
    setMessage("");
    setError("");
    optimistic();
    setBusy(true);
    try {
      const isApply = await persist();
      setMessage(isApply ? t("codingTools.applied", "已应用") : t("codingTools.cleared", "已清除"));
    } catch (e: any) {
      revert();
      setError(t("codingTools.writeFailed", "写入失败") + ": " + String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleToggle = (field: keyof CodingToolsSettings, next: boolean) =>
    runCommit(
      () => setSettings((s) => ({ ...s, [field]: next })),
      () => setSettings((s) => ({ ...s, [field]: !next })),
      async () => {
        const updated = await codingToolsSettingsApi.set({ [field]: next } as Partial<CodingToolsSettings>);
        setSettings(updated);
        return next;
      },
    );

  // 日期改写：镜像 middleware_rule.enabled。UpdateMiddlewareRule 全量覆盖，
  // 须带原规则全部字段仅翻 enabled；先重 list 拿最新规则防本地态漂移。
  const handleDateRewriteToggle = (next: boolean) =>
    runCommit(
      () => setDateRewriteEnabled(next),
      () => setDateRewriteEnabled(!next),
      async () => {
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
        return next;
      },
    );

  const handleLanguageChange = (next: string) => {
    if (busy || next === language) return;
    runCommit(
      () => setLanguage(next),
      () => setLanguage(language),  // language 闭包 = 翻转前 prev
      async () => {
        // 复用 claudeTab 持久化路径：read → merge language → write → sync。
        // 严禁自实现 fs 写（PRD D2 硬约束）。
        await writeClaudeConfigField((c) => ({ ...c, language: next }));
        return true;
      },
    );
  };

  // 努力级别：单值双写 claude 顶层 effortLevel + codex model_reasoning_effort。
  const handleEffortChange = (next: string) => {
    if (busy || next === effort) return;
    runCommit(
      () => setEffort(next),
      () => setEffort(effort),  // effort 闭包 = 翻转前 prev
      async () => {
        await writeClaudeConfigField((c) => ({ ...c, effortLevel: next }));
        const cx = (await codexApi.read()) as Record<string, any> | null;
        const updatedCx: Record<string, any> = { ...(cx ?? {}) };
        updatedCx.model_reasoning_effort = next;
        await codexApi.write(updatedCx);
        return true;
      },
    );
  };

  // 自动压缩窗口：单值双写 claude env + codex config。空串 = 清除两侧键。
  // 输入框单位为 K（外侧静态标签），存原始 token = K × 1000，允许 1 位小数（1.5K = 1500）。
  const handleCompactCommit = (raw: string) => {
    if (busy) return;
    const parsed = parseCompactInput(raw);
    if (parsed === null) {
      setError(t("codingTools.compact.invalid", "请输入非负整数"));
      setCompactDraft(tokensToDraft(compactApplied));
      return;
    }
    const next = parsed;
    runCommit(
      () => { /* draft 保持用户输入；applied 等 persist 确认后再写 */ },
      () => {
        setCompactApplied(compactApplied);  // 闭包 = 翻转前 prev
        setCompactDraft(tokensToDraft(compactApplied));
      },
      async () => {
        // claude：env.CLAUDE_CODE_AUTO_COMPACT_WINDOW
        await writeClaudeConfigField((c) => {
          const env: Record<string, any> = { ...((c.env as Record<string, any>) ?? {}) };
          if (next) env.CLAUDE_CODE_AUTO_COMPACT_WINDOW = next;
          else delete env.CLAUDE_CODE_AUTO_COMPACT_WINDOW;
          return { ...c, env };
        });
        // codex：model_auto_compact_token_limit
        const cx = (await codexApi.read()) as Record<string, any> | null;
        const updatedCx: Record<string, any> = { ...(cx ?? {}) };
        if (next) updatedCx.model_auto_compact_token_limit = next;
        else delete updatedCx.model_auto_compact_token_limit;
        await codexApi.write(updatedCx);
        setCompactApplied(next);
        setCompactDraft(tokensToDraft(next));
        return Boolean(next);
      },
    );
  };

  // 代理设置：onBlur 任一输入触发批量提交。trim 后与 applied diff 才写 claude env。
  // 空值 = 删键；任一键变化即整批重写（writeClaudeConfigField 读全量 → 改 env → 写回）。
  const handleProxyCommit = () => {
    if (busy) return;
    const prev = { ...proxyAppliedRef.current };
    const next = { ...proxyDraft };
    const same = PROXY_KEYS.every((k) => prev[k].trim() === next[k].trim());
    if (same) return;
    runCommit(
      () => { /* draft 保持用户输入；applied 等 persist 确认后再写 */ },
      () => {
        setProxyDraft(prev);
        proxyAppliedRef.current = prev;
      },
      async () => {
        await writeClaudeConfigField((c) => {
          const env: Record<string, any> = { ...((c.env as Record<string, any>) ?? {}) };
          for (const k of PROXY_KEYS) {
            const v = next[k].trim();
            if (v) env[PROXY_ENV_KEYS[k]] = v;
            else delete env[PROXY_ENV_KEYS[k]];
          }
          return { ...c, env };
        });
        proxyAppliedRef.current = next;
        return true;
      },
    );
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

      <ToggleCard
        titleKey="codingTools.applyPlugin.title"
        descKey="codingTools.applyPlugin.desc"
        hint={'~/.claude/config.json · primaryApiKey="any"'}
        active={settings.apply_to_claude_plugin}
        disabled={busy}
        onToggle={(next) => handleToggle("apply_to_claude_plugin", next)}
      />

      <ToggleCard
        titleKey="codingTools.skipOnboarding.title"
        descKey="codingTools.skipOnboarding.desc"
        hint={"~/.claude.json · hasCompletedOnboarding=true"}
        active={settings.skip_claude_onboarding}
        disabled={busy}
        onToggle={(next) => handleToggle("skip_claude_onboarding", next)}
      />

      <ToggleCard
        titleKey="codingTools.dateRewrite.title"
        descKey="codingTools.dateRewrite.desc"
        hint={"middleware · redaction · YYYY/MM/DD → YYYY-MM-DD"}
        active={dateRewriteEnabled ?? false}
        disabled={busy || dateRewriteEnabled == null}
        onToggle={(next) => dateRewriteRuleId != null && handleDateRewriteToggle(next)}
      />

      {/* 语言选择：写 ~/.claude/settings.json 的 language key */}
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
        </div>
        <select
          className="input"
          style={{ fontSize: 13, width: "auto", padding: "4px 28px 4px 8px" }}
          value={language}
          onChange={(e) => handleLanguageChange(e.target.value)}
          disabled={busy}
        >
          {!language && <option value="">—</option>}
          {LANGUAGE_GROUPS.map((g) => (
            <optgroup key={g.family} label={g.family}>
              {g.options.map((o) => (
                <option key={o.value} value={o.value}>{o.label}</option>
              ))}
            </optgroup>
          ))}
        </select>
      </div>

      {/* 努力级别：claude 顶层 effortLevel + codex model_reasoning_effort 双写 */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        gap: 16,
      }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("codingTools.effort.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("codingTools.effort.desc")}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
            claude · effortLevel · codex · model_reasoning_effort
          </div>
        </div>
        <select
          className="input"
          style={{ fontSize: 13, width: "auto", padding: "4px 28px 4px 8px" }}
          value={effort}
          onChange={(e) => handleEffortChange(e.target.value)}
          disabled={busy}
        >
          {!effort && <option value="">—</option>}
          {EFFORT_OPTIONS.map((lv) => (
            <option key={lv} value={lv}>{lv}</option>
          ))}
        </select>
      </div>

      {/* 自动压缩窗口：claude env.CLAUDE_CODE_AUTO_COMPACT_WINDOW + codex model_auto_compact_token_limit */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        gap: 16,
      }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{t("codingTools.compact.title")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("codingTools.compact.desc")}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
            claude · env.CLAUDE_CODE_AUTO_COMPACT_WINDOW · codex · model_auto_compact_token_limit
          </div>
          {compactApplied && (
            <div className="text-tertiary" style={{ fontSize: 11, marginTop: 4 }}>
              {t("codingTools.compact.current", "当前")}: {compactApplied} tokens
            </div>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <input
            className="input"
            type="number"
            min={0}
            step="0.1"
            style={{ fontSize: 13, width: 110, padding: "4px 8px" }}
            value={compactDraft}
            placeholder="200"
            onChange={(e) => setCompactDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                (e.target as HTMLInputElement).blur();
              }
            }}
            onBlur={() => {
              if (tokensToDraft(compactApplied) !== compactDraft) void handleCompactCommit(compactDraft);
            }}
            disabled={busy}
          />
          <span style={{ fontSize: 13, color: "var(--text-secondary)", fontWeight: 600 }}>K</span>
        </div>
      </div>

      {/* 代理设置：仅 Claude env.HTTP_PROXY/HTTPS_PROXY/ALL_PROXY/NO_PROXY（4 输入，onBlur 批量提交）。
          Codex 无 config 级 proxy 字段，由「复制启动命令」注入 env（另见 Groups 启动命令复制点）。 */}
      <div className="glass-surface" style={{ padding: "16px 20px" }}>
        <div style={{ fontSize: 14, fontWeight: 600 }}>{t("codingTools.proxy.title")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("codingTools.proxy.desc")}
        </div>
        <div className="text-tertiary" style={{ fontSize: 11, marginTop: 6, fontFamily: "ui-monospace, monospace" }}>
          claude · env.HTTP_PROXY / HTTPS_PROXY / ALL_PROXY / NO_PROXY
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10, marginTop: 12 }}>
          {PROXY_KEYS.map((k) => (
            <div key={k} style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              <label className="text-secondary" style={{ fontSize: 11, fontWeight: 600 }}>
                {PROXY_ENV_KEYS[k]}
              </label>
              <input
                className="input"
                type="text"
                style={{ fontSize: 13, padding: "4px 8px" }}
                value={proxyDraft[k]}
                placeholder={k === "no" ? "localhost,127.0.0.1,*.local" : "http://host:port"}
                onChange={(e) => setProxyDraft((d) => ({ ...d, [k]: e.target.value }))}
                onKeyDown={(e) => {
                  if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                }}
                onBlur={handleProxyCommit}
                disabled={busy}
              />
            </div>
          ))}
        </div>
      </div>

      {/* 常驻错误态：写外部文件失败时不可错过，红色 inline、不自动消失。
          与 Sub2ApiImport/CcSwitchImport 约定对齐（var(--color-danger-bg) + 1px danger border）。 */}
      {error && (
        <div
          role="alert"
          className="toast"
          style={{
            border: "1px solid var(--color-danger)",
            background: "var(--color-danger-bg)",
            color: "var(--color-danger)",
            fontSize: 13,
            lineHeight: 1.6,
            display: "flex",
            flexDirection: "column",
            gap: 4,
            wordBreak: "break-word",
          }}
        >
          <div style={{ fontWeight: 600 }}>{t("codingTools.errorTitle", "开关未生效")}</div>
          <div style={{ fontFamily: "ui-monospace, monospace", fontSize: 12, opacity: 0.92 }}>
            {error}
          </div>
          <div style={{ fontSize: 12, opacity: 0.85 }}>
            {t("codingTools.errorHint", "请检查该文件的 JSON 格式与读写权限后重试。")}
          </div>
        </div>
      )}

      {/* 瞬时成功提示（仅在无错误时显示） */}
      {message && !error && <div className="toast">{message}</div>}
    </div>
  );
}
