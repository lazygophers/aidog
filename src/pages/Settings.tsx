import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { settingsApi } from "../services/api";

const CONFIG_KEY = "claude_code";
const EFFORT_LEVELS = ["low", "medium", "high", "xhigh"];

// ─── Sub-components ────────────────────────────────────────

/** String list editor (for permissions allow/ask/deny) */
function StringListEditor({
  items,
  onChange,
  addLabel,
}: {
  items: string[];
  onChange: (items: string[]) => void;
  addLabel: string;
}) {
  const [draft, setDraft] = useState("");

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      {items.map((item, i) => (
        <div key={i} style={{ display: "flex", gap: 4 }}>
          <input
            className="input"
            style={{ flex: 1, fontSize: 12 }}
            value={item}
            onChange={(e) => {
              const next = [...items];
              next[i] = e.target.value;
              onChange(next);
            }}
          />
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            style={{ width: 24, height: 24, minWidth: 24, fontSize: 12 }}
            onClick={() => onChange(items.filter((_, j) => j !== i))}
          >
            ×
          </button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 4 }}>
        <input
          className="input"
          style={{ flex: 1, fontSize: 12 }}
          placeholder={addLabel}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && draft.trim()) {
              onChange([...items, draft.trim()]);
              setDraft("");
            }
          }}
        />
        <button
          type="button"
          className="btn btn-ghost"
          style={{ fontSize: 12, padding: "4px 8px" }}
          onClick={() => {
            if (draft.trim()) {
              onChange([...items, draft.trim()]);
              setDraft("");
            }
          }}
        >
          +
        </button>
      </div>
    </div>
  );
}

/** Key-value editor (for env) */
function KvEditor({
  items,
  onChange,
}: {
  items: Record<string, string>;
  onChange: (items: Record<string, string>) => void;
}) {
  const [newKey, setNewKey] = useState("");
  const [newVal, setNewVal] = useState("");
  const entries = Object.entries(items);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      {entries.map(([k, v]) => (
        <div key={k} style={{ display: "flex", gap: 4 }}>
          <input
            className="input"
            style={{ flex: 2, fontSize: 12 }}
            value={k}
            readOnly
          />
          <input
            className="input"
            style={{ flex: 3, fontSize: 12 }}
            value={v}
            onChange={(e) => onChange({ ...items, [k]: e.target.value })}
          />
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            style={{ width: 24, height: 24, minWidth: 24, fontSize: 12 }}
            onClick={() => {
              const next = { ...items };
              delete next[k];
              onChange(next);
            }}
          >
            ×
          </button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 4 }}>
        <input
          className="input"
          style={{ flex: 2, fontSize: 12 }}
          placeholder="KEY"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
        />
        <input
          className="input"
          style={{ flex: 3, fontSize: 12 }}
          placeholder="VALUE"
          value={newVal}
          onChange={(e) => setNewVal(e.target.value)}
        />
        <button
          type="button"
          className="btn btn-ghost"
          style={{ fontSize: 12, padding: "4px 8px" }}
          onClick={() => {
            if (newKey.trim()) {
              onChange({ ...items, [newKey.trim()]: newVal });
              setNewKey("");
              setNewVal("");
            }
          }}
        >
          +
        </button>
      </div>
    </div>
  );
}

// ─── Main Settings Page (Global Claude Code Config) ────────

export function Settings() {
  const { t } = useTranslation();
  const [mode, setMode] = useState<"json" | "gui">("gui");
  const [config, setConfig] = useState<Record<string, any>>({});
  const [editJson, setEditJson] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [toast, setToast] = useState("");

  useEffect(() => {
    const load = async () => {
      try {
        const result = await settingsApi.get("global", CONFIG_KEY);
        const data = (result as Record<string, any>) ?? {};
        setConfig(data);
        setEditJson(JSON.stringify(data, null, 2));
      } catch (e) {
        console.error(e);
      }
    };
    load();
  }, []);

  const updateField = (field: string, value: any) => {
    setConfig((prev) => {
      const next: Record<string, any> = {};
      for (const [k, v] of Object.entries(prev)) {
        if (k !== field) next[k] = v;
      }
      if (value !== undefined && value !== null && value !== "") {
        next[field] = value;
      }
      return next;
    });
  };

  const handleSave = async () => {
    setSaving(true);
    setSaveError("");
    try {
      const value =
        mode === "json" ? JSON.parse(editJson) : { ...config };
      await settingsApi.set("global", CONFIG_KEY, value);
      setConfig(value);
      setEditJson(JSON.stringify(value, null, 2));
      setToast(t("settings.saved"));
      setTimeout(() => setToast(""), 2000);
    } catch (e: any) {
      setSaveError(e.toString());
    }
    setSaving(false);
  };

  // Permissions helpers
  const perms = (config.permissions ?? {}) as Record<string, string[]>;
  const envObj = (config.env ?? {}) as Record<string, string>;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 20,
        maxWidth: 720,
        width: "100%",
      }}
    >
      {/* Header */}
      <div className="section-header">
        <div>
          <div className="section-title">{t("settings.title")}</div>
          <div className="section-desc">{t("settings.desc")}</div>
        </div>
      </div>

      <div
        className="glass-surface"
        style={{
          padding: 16,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}
      >
        {/* Mode toggle */}
        <div
          style={{
            display: "flex",
            gap: 4,
            borderBottom: "1px solid var(--border)",
            paddingBottom: 8,
          }}
        >
          <button
            className={`btn ${mode === "gui" ? "btn-primary" : "btn-ghost"}`}
            style={{ fontSize: 12, padding: "5px 12px" }}
            onClick={() => setMode("gui")}
          >
            {t("settings.guiMode")}
          </button>
          <button
            className={`btn ${mode === "json" ? "btn-primary" : "btn-ghost"}`}
            style={{ fontSize: 12, padding: "5px 12px" }}
            onClick={() => {
              setEditJson(JSON.stringify(config, null, 2));
              setMode("json");
            }}
          >
            {t("settings.jsonMode")}
          </button>
        </div>

        {/* JSON mode */}
        {mode === "json" && (
          <textarea
            className="input"
            style={{
              fontFamily: '"SF Mono", "Fira Code", monospace',
              fontSize: 12,
              lineHeight: 1.6,
              minHeight: 360,
              resize: "vertical",
              whiteSpace: "pre",
            }}
            value={editJson}
            onChange={(e) => setEditJson(e.target.value)}
            spellCheck={false}
          />
        )}

        {/* GUI mode */}
        {mode === "gui" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
            {/* Model */}
            <div>
              <label
                style={{
                  display: "block",
                  fontSize: 12,
                  fontWeight: 500,
                  color: "var(--text-secondary)",
                  marginBottom: 4,
                }}
              >
                {t("settings.model")}
              </label>
              <input
                className="input"
                placeholder={t("settings.modelPlaceholder")}
                value={config.model ?? ""}
                onChange={(e) =>
                  updateField("model", e.target.value || undefined)
                }
              />
            </div>

            {/* Effort Level */}
            <div>
              <label
                style={{
                  display: "block",
                  fontSize: 12,
                  fontWeight: 500,
                  color: "var(--text-secondary)",
                  marginBottom: 4,
                }}
              >
                {t("settings.effortLevel")}
              </label>
              <select
                className="input"
                value={config.effortLevel ?? ""}
                onChange={(e) =>
                  updateField("effortLevel", e.target.value || undefined)
                }
              >
                <option value="">—</option>
                {EFFORT_LEVELS.map((lv) => (
                  <option key={lv} value={lv}>
                    {lv}
                  </option>
                ))}
              </select>
            </div>

            {/* Output Style */}
            <div>
              <label
                style={{
                  display: "block",
                  fontSize: 12,
                  fontWeight: 500,
                  color: "var(--text-secondary)",
                  marginBottom: 4,
                }}
              >
                {t("settings.outputStyle")}
              </label>
              <input
                className="input"
                placeholder="Explanatory"
                value={config.outputStyle ?? ""}
                onChange={(e) =>
                  updateField("outputStyle", e.target.value || undefined)
                }
              />
            </div>

            {/* Language */}
            <div>
              <label
                style={{
                  display: "block",
                  fontSize: 12,
                  fontWeight: 500,
                  color: "var(--text-secondary)",
                  marginBottom: 4,
                }}
              >
                {t("settings.language")}
              </label>
              <input
                className="input"
                placeholder="chinese, english, japanese..."
                value={config.language ?? ""}
                onChange={(e) =>
                  updateField("language", e.target.value || undefined)
                }
              />
            </div>

            {/* Always Thinking */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
              }}
            >
              <span
                style={{
                  fontSize: 12,
                  fontWeight: 500,
                  color: "var(--text-secondary)",
                }}
              >
                {t("settings.alwaysThinking")}
              </span>
              <div
                className={`toggle ${config.alwaysThinkingEnabled ? "active" : ""}`}
                onClick={() =>
                  updateField(
                    "alwaysThinkingEnabled",
                    config.alwaysThinkingEnabled ? undefined : true,
                  )
                }
              />
            </div>

            {/* Permissions */}
            <div
              style={{
                borderTop: "1px solid var(--border)",
                paddingTop: 10,
              }}
            >
              <span
                style={{
                  display: "block",
                  fontSize: 13,
                  fontWeight: 600,
                  color: "var(--text-primary)",
                  marginBottom: 8,
                }}
              >
                {t("settings.permissions")}
              </span>

              <div style={{ marginBottom: 8 }}>
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 500,
                    color: "var(--text-tertiary)",
                    marginBottom: 4,
                  }}
                >
                  {t("settings.permissionsAllow")}
                </div>
                <StringListEditor
                  items={perms.allow ?? []}
                  onChange={(list) => {
                    const next: Record<string, string[]> = {};
                    if (perms.ask?.length) next.ask = perms.ask;
                    if (perms.deny?.length) next.deny = perms.deny;
                    if (list.length > 0) next.allow = list;
                    updateField(
                      "permissions",
                      Object.keys(next).length > 0 ? next : undefined,
                    );
                  }}
                  addLabel={t("settings.addRule")}
                />
              </div>

              <div style={{ marginBottom: 8 }}>
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 500,
                    color: "var(--text-tertiary)",
                    marginBottom: 4,
                  }}
                >
                  {t("settings.permissionsAsk")}
                </div>
                <StringListEditor
                  items={perms.ask ?? []}
                  onChange={(list) => {
                    const next: Record<string, string[]> = {};
                    if (perms.allow?.length) next.allow = perms.allow;
                    if (perms.deny?.length) next.deny = perms.deny;
                    if (list.length > 0) next.ask = list;
                    updateField(
                      "permissions",
                      Object.keys(next).length > 0 ? next : undefined,
                    );
                  }}
                  addLabel={t("settings.addRule")}
                />
              </div>

              <div>
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 500,
                    color: "var(--text-tertiary)",
                    marginBottom: 4,
                  }}
                >
                  {t("settings.permissionsDeny")}
                </div>
                <StringListEditor
                  items={perms.deny ?? []}
                  onChange={(list) => {
                    const next: Record<string, string[]> = {};
                    if (perms.allow?.length) next.allow = perms.allow;
                    if (perms.ask?.length) next.ask = perms.ask;
                    if (list.length > 0) next.deny = list;
                    updateField(
                      "permissions",
                      Object.keys(next).length > 0 ? next : undefined,
                    );
                  }}
                  addLabel={t("settings.addRule")}
                />
              </div>
            </div>

            {/* Env */}
            <div
              style={{
                borderTop: "1px solid var(--border)",
                paddingTop: 10,
              }}
            >
              <span
                style={{
                  display: "block",
                  fontSize: 13,
                  fontWeight: 600,
                  color: "var(--text-primary)",
                  marginBottom: 8,
                }}
              >
                {t("settings.env")}
              </span>
              <KvEditor
                items={envObj}
                onChange={(newEnv) =>
                  updateField(
                    "env",
                    Object.keys(newEnv).length > 0 ? newEnv : undefined,
                  )
                }
              />
            </div>
          </div>
        )}

        {/* Error */}
        {saveError && (
          <div
            className="toast"
            style={{
              fontSize: 12,
              wordBreak: "break-all",
              color: "#ff453a",
            }}
          >
            {saveError}
          </div>
        )}

        {/* Actions */}
        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
            paddingTop: 8,
            borderTop: "1px solid var(--border)",
          }}
        >
          {toast && (
            <span
              style={{
                fontSize: 12,
                color: "#34c759",
                alignSelf: "center",
                marginRight: "auto",
              }}
            >
              {toast}
            </span>
          )}
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? t("status.loading") : t("action.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
