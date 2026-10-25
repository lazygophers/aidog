// NotificationRules — 规则列表区（通知体系票 04）。
// 预置规则：只可开关/复制；用户规则：增删改。条件表达式编辑期调后端编译校验（报错带字符定位）。
// 渠道并集驱动 dispatch（本地 popup/inbox/sound/tts + 外部 7 渠道）。
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { notificationApi, type NotificationSettings, type Rule, type Channel } from "../../services/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { parseToVisual, toText, type Visual } from "../../domains/notifications/ruleExpr";

type Persist = (f: (prev: NotificationSettings) => NotificationSettings) => void;

const ALL_CHANNELS: Channel[] = ["popup", "inbox", "sound", "tts", "webhook", "telegram", "bark", "discord", "slack", "email", "feishu"];
const SEMANTIC_TYPES = ["task_complete", "waiting_input", "error", "breaker_on", "breaker_off", "quota_exhaust_forecast"];

function blankRule(id: number): Rule {
  return {
    id: `user-${id}-${Date.now()}`,
    name: "",
    enabled: true,
    preset: false,
    match_type: "task_complete",
    condition: null,
    action: { channels: ["inbox"], bypass_dnd: false, dedup_window_secs: 0 },
  };
}

function RuleEditor({ rule, onSave, onCancel }: {
  rule: Rule; onSave: (r: Rule) => void; onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<Rule>(rule);
  const [exprMsg, setExprMsg] = useState("");
  const [exprOk, setExprOk] = useState<"ok" | "err" | null>(null);
  // 票 05：可视化 ⇄ 文本双向。打开时能解析则进可视化；文本→可视化失败 → 降级提示并停留文本（保留原文）。
  const initial = parseToVisual(rule.condition ?? "");
  const [mode, setMode] = useState<"visual" | "text">(initial.ok ? "visual" : "text");
  const [visual, setVisual] = useState<Visual>(initial.ok ? initial.visual : { join: "&&", terms: [{ field: "severity", op: "==", value: "error" }] });
  const [degrade, setDegrade] = useState("");

  const applyVisual = (v: Visual) => {
    setVisual(v);
    setDraft(d => ({ ...d, condition: v.terms.length ? toText(v) : null }));
  };

  const switchToVisual = () => {
    const r = parseToVisual(draft.condition ?? "");
    if (r.ok) {
      setVisual(r.visual);
      setMode("visual");
      setDegrade("");
    } else {
      setDegrade(r.reason);
    }
  };

  const switchToText = () => {
    // 可视化 → 文本：串行化（总可行），空条件保留空。
    setDraft(d => ({ ...d, condition: d.condition }));
    setMode("text");
    setDegrade("");
  };

  const validate = async () => {
    const expr = draft.condition?.trim() ?? "";
    if (!expr) { setExprMsg(""); setExprOk(null); return; }
    try {
      await notificationApi.validateRuleExpr(expr);
      setExprMsg(t("notif.rules.validOk", "✓ 表达式有效"));
      setExprOk("ok");
    } catch (e) {
      setExprMsg(String(e));
      setExprOk("err");
    }
  };

  const toggleChannel = (c: Channel) => setDraft(d => ({
    ...d,
    action: {
      ...d.action,
      channels: d.action.channels.includes(c)
        ? d.action.channels.filter(x => x !== c)
        : [...d.action.channels, c],
    },
  }));

  return (
    <div id={`notif-rule-editor-${draft.id}`} data-state="open"
      style={{ display: "flex", flexDirection: "column", gap: 8, padding: "8px 0", borderTop: "1px solid var(--border)" }}>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
        <label style={{ display: "flex", flexDirection: "column", gap: 3, fontSize: 11, minWidth: 160, flex: 1 }}>
          <span className="text-secondary">{t("notif.rules.name", "名称")}</span>
          <Input className="input" style={{ fontSize: 12 }} value={draft.name}
            onChange={e => setDraft(d => ({ ...d, name: e.target.value }))} />
        </label>
        <label style={{ display: "flex", flexDirection: "column", gap: 3, fontSize: 11, minWidth: 160 }}>
          <span className="text-secondary">{t("notif.rules.matchType", "事件类型")}</span>
          <select className="input" style={{ fontSize: 12, padding: "4px 6px" }}
            value={draft.match_type ?? ""}
            onChange={e => setDraft(d => ({ ...d, match_type: e.target.value || null }))}>
            <option value="">{t("notif.rules.anyType", "任意类型")}</option>
            {SEMANTIC_TYPES.map(s => <option key={s} value={s}>{s}</option>)}
          </select>
        </label>
        <label style={{ display: "flex", flexDirection: "column", gap: 3, fontSize: 11, minWidth: 120 }}>
          <span className="text-secondary">{t("notif.rules.dedupSecs", "去重窗口（秒）")}</span>
          <Input className="input" type="number" style={{ fontSize: 12 }} value={draft.action.dedup_window_secs}
            onChange={e => setDraft(d => ({ ...d, action: { ...d.action, dedup_window_secs: Number(e.target.value) || 0 } }))} />
        </label>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <div style={{ display: "flex", gap: 8, alignItems: "center", justifyContent: "space-between" }}>
          <span className="text-secondary" style={{ fontSize: 11 }}>
            {t("notif.rules.condition", "条件表达式（可选：字段比较 + && / ||，字段 type/severity/vars.*）")}
          </span>
          <div style={{ display: "flex", gap: 4 }}>
            <button type="button" id="notif-rule-mode-visual" data-state={mode === "visual" ? "active" : "idle"}
              onClick={() => mode === "text" && switchToVisual()}
              style={{ all: "unset", cursor: "pointer", fontSize: 11, padding: "1px 8px", borderRadius: "var(--radius-sm)",
                background: mode === "visual" ? "var(--accent-subtle)" : "transparent", color: "var(--text-secondary)" }}>
              {t("notif.rules.visualMode", "可视化")}
            </button>
            <button type="button" id="notif-rule-mode-text" data-state={mode === "text" ? "active" : "idle"}
              onClick={() => mode === "visual" && switchToText()}
              style={{ all: "unset", cursor: "pointer", fontSize: 11, padding: "1px 8px", borderRadius: "var(--radius-sm)",
                background: mode === "text" ? "var(--accent-subtle)" : "transparent", color: "var(--text-secondary)" }}>
              {t("notif.rules.textMode", "文本")}
            </button>
            <Button variant="ghost" size="sm" style={{ fontSize: 11, height: "auto", padding: "2px 8px" }}
              onClick={validate}>
              {t("notif.rules.validate", "校验")}
            </Button>
          </div>
        </div>
        {mode === "visual" ? (
          <div id="notif-rule-visual" data-state="open" style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {visual.terms.map((term, ti) => (
              <div key={ti} id={`notif-rule-term-${ti}`} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <input className="input" list="notif-rule-fields" style={{ fontSize: 12, width: 170 }}
                  value={term.field}
                  onChange={e => applyVisual({ ...visual, terms: visual.terms.map((x, i) => i === ti ? { ...x, field: e.target.value } : x) })} />
                <datalist id="notif-rule-fields">
                  <option value="type" /><option value="severity" /><option value="vars.platform" /><option value="vars.platform_id" /><option value="vars.project" />
                </datalist>
                <select className="input" style={{ fontSize: 12, width: 90 }} value={term.op}
                  onChange={e => applyVisual({ ...visual, terms: visual.terms.map((x, i) => i === ti ? { ...x, op: e.target.value as typeof term.op } : x) })}>
                  <option value="==">==</option><option value="!=">!=</option><option value="contains">contains</option>
                </select>
                <input className="input" style={{ fontSize: 12, flex: 1 }} value={term.value}
                  onChange={e => applyVisual({ ...visual, terms: visual.terms.map((x, i) => i === ti ? { ...x, value: e.target.value } : x) })} />
                <Button variant="ghost" size="sm" style={{ fontSize: 10, height: "auto", padding: "1px 6px" }}
                  onClick={() => applyVisual({ ...visual, terms: visual.terms.filter((_, i) => i !== ti) })}>
                  ✕
                </Button>
                {ti === 0 && (
                  <select className="input" style={{ fontSize: 12, width: 56 }} value={visual.join}
                    onChange={e => applyVisual({ ...visual, join: e.target.value as "&&" || "||" })}>
                    <option value="&&">&amp;&amp;</option><option value="||">||</option>
                  </select>
                )}
              </div>
            ))}
            <Button variant="ghost" size="sm" style={{ fontSize: 11, height: "auto", padding: "2px 8px", alignSelf: "flex-start" }}
              onClick={() => applyVisual({ ...visual, terms: [...visual.terms, { field: "severity", op: "==", value: "" }] })}>
              + {t("notif.rules.addField", "加条件")}
            </Button>
            <div className="text-tertiary" style={{ fontSize: 11, fontFamily: "var(--font-mono, monospace)" }}>{toText(visual)}</div>
          </div>
        ) : (
          <Input className="input" style={{ fontSize: 12, fontFamily: "var(--font-mono, monospace)" }}
            placeholder='severity == "error" && vars.platform contains "glm"'
            value={draft.condition ?? ""} onChange={e => { setDraft(d => ({ ...d, condition: e.target.value || null })); setDegrade(""); }} />
        )}
        {degrade && (
          <div id="notif-rule-degrade" data-state="shown" style={{ fontSize: 11, color: "var(--status-warn, #fbbf24)" }}>
            {t("notif.rules.degradeHint", "该表达式无法转成可视化（原因见下），已保留原文——请继续在文本模式编辑")}
            <div className="text-tertiary">{degrade}</div>
          </div>
        )}
      </div>
      {exprMsg && <div id={`notif-rule-exprmsg-${draft.id}`} data-ok={exprOk === "ok" ? "yes" : "no"}
        style={{ fontSize: 11, color: exprMsg.startsWith("✓") ? "var(--status-ok, #4ade80)" : "var(--status-error, #f87171)" }}>{exprMsg}</div>}
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
        <span className="text-secondary" style={{ fontSize: 11 }}>{t("notif.rules.channels", "渠道")}</span>
        {ALL_CHANNELS.map(c => (
          <label key={c} style={{ display: "flex", gap: 3, alignItems: "center", fontSize: 11 }}>
            <input type="checkbox" checked={draft.action.channels.includes(c)} onChange={() => toggleChannel(c)} />
            {t(`notif.channel.${c}`, c)}
          </label>
        ))}
        <label style={{ display: "flex", gap: 4, alignItems: "center", fontSize: 11, marginLeft: 8 }}>
          <Switch checked={draft.action.bypass_dnd}
            onCheckedChange={v => setDraft(d => ({ ...d, action: { ...d.action, bypass_dnd: v } }))} />
          {t("notif.rules.bypassDnd", "免打扰穿透")}
        </label>
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <Button variant="ghost" size="sm" style={{ fontSize: 11, height: "auto", padding: "4px 10px" }}
          onClick={() => onSave(draft)}>
          {t("common.save", "保存")}
        </Button>
        <Button variant="ghost" size="sm" style={{ fontSize: 11, height: "auto", padding: "4px 10px" }}
          onClick={onCancel}>
          {t("common.cancel", "取消")}
        </Button>
      </div>
    </div>
  );
}

export function NotificationRulesSection({ settings, persist }: { settings: NotificationSettings; persist: Persist }) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<Rule | null>(null);
  const rules = settings.rules;

  const setRules = (f: (rs: Rule[]) => Rule[]) => persist(prev => ({ ...prev, rules: f(prev.rules) }));

  const copyPreset = (r: Rule) => setRules(rs => [...rs, {
    ...r,
    id: `copy-of-${r.id}-${Date.now()}`,
    name: `${r.name}（${t("notif.rules.copy", "副本")}）`,
    preset: false,
  }]);

  return (
    <div className="glass-surface" style={{ padding: "14px 18px", display: "flex", flexDirection: "column", gap: 10 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("notif.rules.title", "通知规则")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("notif.rules.desc", "命中规则按渠道并集投递；预置规则只可开关/复制")}
          </div>
        </div>
        <Button variant="ghost" size="sm" style={{ fontSize: 11, height: "auto", padding: "4px 10px" }}
          onClick={() => setEditing(blankRule(rules.length + 1))}>
          + {t("notif.rules.add", "添加规则")}
        </Button>
      </div>
      {rules.length === 0 && (
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("notif.rules.emptyHint", "未存储规则——分发回落内置预置（每种类型一条，可在上方开关）")}
        </div>
      )}
      {editing && (
        <RuleEditor rule={editing}
          onSave={r => { setRules(rs => rs.some(x => x.id === r.id) ? rs.map(x => x.id === r.id ? r : x) : [...rs, r]); setEditing(null); }}
          onCancel={() => setEditing(null)} />
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {rules.map(r => (
          <div key={r.id} id={`notif-rule-${r.id}`} data-enabled={r.enabled ? "yes" : "no"}
            style={{ display: "flex", gap: 8, alignItems: "center", padding: "6px 8px", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)" }}>
            <Switch checked={r.enabled}
              onCheckedChange={v => setRules(rs => rs.map(x => x.id === r.id ? { ...x, enabled: v } : x))} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <span style={{ fontSize: 12, fontWeight: 600 }}>{r.name || r.id}</span>
              {r.preset && <span style={{ fontSize: 10, marginLeft: 6, padding: "0 5px", borderRadius: "var(--radius-sm)", background: "var(--accent-subtle)", color: "var(--accent)" }}>{t("notif.rules.preset", "预置")}</span>}
              <span style={{ fontSize: 11, color: "var(--text-tertiary)", marginLeft: 8 }}>
                {r.match_type ?? t("notif.rules.anyType", "任意类型")}{r.condition ? ` · ${r.condition}` : ""}
              </span>
              <span style={{ fontSize: 11, color: "var(--text-secondary)", marginLeft: 8 }}>
                → {r.action.channels.map(c => t(`notif.channel.${c}`, c)).join(", ")}
              </span>
            </div>
            {r.preset ? (
              <Button variant="ghost" size="sm" style={{ fontSize: 10, height: "auto", padding: "2px 8px" }}
                onClick={() => copyPreset(r)}>
                {t("notif.rules.copyAction", "复制")}
              </Button>
            ) : (
              <div style={{ display: "flex", gap: 4 }}>
                <Button variant="ghost" size="sm" style={{ fontSize: 10, height: "auto", padding: "2px 8px" }}
                  onClick={() => setEditing(r)}>
                  {t("notif.rules.edit", "编辑")}
                </Button>
                <Button variant="ghost" size="sm" style={{ fontSize: 10, height: "auto", padding: "2px 8px" }}
                  onClick={() => setRules(rs => rs.filter(x => x.id !== r.id))}>
                  {t("notif.rules.delete", "删除")}
                </Button>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
