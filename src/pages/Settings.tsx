import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  platformApi,
  groupDetailApi,
  settingsApi,
  type Platform,
  type GroupDetail,
} from "../services/api";

const CONFIG_KEY = "claude_code";
const EFFORT_LEVELS = ["low", "medium", "high", "xhigh"];
const KNOWN_FIELDS = [
  "model",
  "effortLevel",
  "outputStyle",
  "language",
  "alwaysThinkingEnabled",
  "permissions",
  "env",
];

// ─── Sub-components ────────────────────────────────────────

/** Alignment badge: "与全局对齐" or "已自定义" */
function AlignBadge({
  overridden,
  onReset,
  t,
}: {
  overridden: boolean;
  onReset?: () => void;
  t: (k: string) => string;
}) {
  if (!overridden) {
    return (
      <span
        style={{
          fontSize: 11,
          padding: "1px 6px",
          borderRadius: 4,
          background: "var(--bg-glass)",
          color: "var(--text-tertiary)",
          border: "1px solid var(--border)",
        }}
      >
        {t("settings.aligned")}
      </span>
    );
  }
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 4,
        fontSize: 11,
        padding: "1px 6px",
        borderRadius: 4,
        background: "var(--accent-subtle)",
        color: "var(--accent)",
        border: "1px solid var(--accent-subtle)",
      }}
    >
      {t("settings.customized")}
      {onReset && (
        <button
          type="button"
          onClick={onReset}
          style={{
            background: "none",
            border: "none",
            color: "var(--accent)",
            cursor: "pointer",
            fontSize: 10,
            padding: 0,
            lineHeight: 1,
          }}
          title={t("settings.reset")}
        >
          ✕
        </button>
      )}
    </span>
  );
}

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

// ─── Main Settings Page ────────────────────────────────────

export function Settings() {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [selectedScope, setSelectedScope] = useState("global");
  const [mode, setMode] = useState<"json" | "gui">("gui");
  const [loading, setLoading] = useState(true);

  // Settings data
  const [globalJson, setGlobalJson] = useState<Record<string, any>>({});
  const [overrides, setOverrides] = useState<Record<string, any>>({});
  const [editJson, setEditJson] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [toast, setToast] = useState("");

  // Load platforms and groups
  useEffect(() => {
    const load = async () => {
      try {
        const [p, g] = await Promise.all([
          platformApi.list(),
          groupDetailApi.list(),
        ]);
        setPlatforms(p || []);
        setGroups(g || []);
      } catch (e) {
        console.error(e);
      }
      setLoading(false);
    };
    load();
  }, []);

  // Load global settings
  useEffect(() => {
    const load = async () => {
      try {
        const result = await settingsApi.get("global", CONFIG_KEY);
        setGlobalJson((result as Record<string, any>) ?? {});
      } catch (e) {
        console.error(e);
      }
    };
    load();
  }, []);

  // Load scope overrides when scope changes
  useEffect(() => {
    const load = async () => {
      if (selectedScope === "global") {
        setOverrides({});
        setEditJson(JSON.stringify(globalJson, null, 2));
      } else {
        try {
          const result = await settingsApi.get(selectedScope, CONFIG_KEY);
          const ov = (result as Record<string, any>) ?? {};
          setOverrides(ov);
          setEditJson(JSON.stringify({ ...globalJson, ...ov }, null, 2));
        } catch (e) {
          console.error(e);
          setOverrides({});
          setEditJson(JSON.stringify(globalJson, null, 2));
        }
      }
    };
    load();
  }, [selectedScope, globalJson]);

  // Effective settings (merged)
  const effective: Record<string, any> =
    selectedScope === "global"
      ? { ...globalJson }
      : { ...globalJson, ...overrides };

  const isOverridden = (field: string) =>
    selectedScope !== "global" && field in overrides;

  const updateField = useCallback(
    (field: string, value: any) => {
      if (selectedScope === "global") {
        setGlobalJson((prev) => {
          const next = { ...prev };
          if (value === undefined || value === null || value === "") {
            delete next[field];
          } else {
            next[field] = value;
          }
          return next;
        });
      } else {
        setOverrides((prev) => {
          const next = { ...prev };
          if (
            value === undefined ||
            value === null ||
            value === "" ||
            JSON.stringify(value) === JSON.stringify(globalJson[field])
          ) {
            delete next[field];
          } else {
            next[field] = value;
          }
          return next;
        });
      }
    },
    [selectedScope, globalJson],
  );

  const resetField = useCallback((field: string) => {
    setOverrides((prev) => {
      const next = { ...prev };
      delete next[field];
      return next;
    });
  }, []);

  const handleSave = async () => {
    setSaving(true);
    setSaveError("");
    try {
      if (selectedScope === "global") {
        const value =
          mode === "json" ? JSON.parse(editJson) : { ...globalJson };
        await settingsApi.set("global", CONFIG_KEY, value);
        setGlobalJson(value);
      } else {
        if (mode === "json") {
          const merged = JSON.parse(editJson);
          const diff: Record<string, any> = {};
          for (const [k, v] of Object.entries(merged)) {
            if (JSON.stringify(v) !== JSON.stringify(globalJson[k])) {
              diff[k] = v;
            }
          }
          await settingsApi.set(selectedScope, CONFIG_KEY, diff);
          setOverrides(diff);
        } else {
          if (Object.keys(overrides).length > 0) {
            await settingsApi.set(selectedScope, CONFIG_KEY, overrides);
          } else {
            await settingsApi.delete(selectedScope, CONFIG_KEY);
          }
        }
      }
      setToast(t("settings.saved"));
      setTimeout(() => setToast(""), 2000);
    } catch (e: any) {
      setSaveError(e.toString());
    }
    setSaving(false);
  };

  // ── Extra fields not covered by GUI ──
  const extraFields = Object.keys(effective).filter(
    (k) => !KNOWN_FIELDS.includes(k),
  );

  // ── Permissions helpers ──
  const perms = (effective.permissions ?? {}) as Record<string, string[]>;
  const permsOverridden = isOverridden("permissions");

  // ── Env helpers ──
  const envObj = (effective.env ?? {}) as Record<string, string>;

  // ── Render ──

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 20,
        maxWidth: 900,
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

      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>
          {t("status.loading")}
        </div>
      ) : (
        <div style={{ display: "flex", gap: 16 }}>
          {/* ── Left: Scope List ── */}
          <div
            className="glass-surface"
            style={{
              width: 180,
              minWidth: 180,
              padding: 12,
              display: "flex",
              flexDirection: "column",
              gap: 2,
              alignSelf: "flex-start",
            }}
          >
            <div
              style={{
                fontSize: 11,
                fontWeight: 600,
                color: "var(--text-tertiary)",
                padding: "4px 8px",
                textTransform: "uppercase",
                letterSpacing: "0.05em",
              }}
            >
              {t("settings.scope")}
            </div>

            {/* Global */}
            <button
              className="btn btn-ghost"
              style={{
                justifyContent: "flex-start",
                padding: "7px 8px",
                fontSize: 13,
                fontWeight: selectedScope === "global" ? 600 : 400,
                color:
                  selectedScope === "global"
                    ? "var(--accent)"
                    : "var(--text-primary)",
                background:
                  selectedScope === "global"
                    ? "var(--accent-subtle)"
                    : "transparent",
                borderRadius: "var(--radius-sm)",
              }}
              onClick={() => {
                setSelectedScope("global");
                setMode("gui");
              }}
            >
              ● {t("settings.global")}
            </button>

            {/* Platforms */}
            {platforms.length > 0 && (
              <>
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 600,
                    color: "var(--text-tertiary)",
                    padding: "8px 8px 2px",
                    textTransform: "uppercase",
                    letterSpacing: "0.05em",
                  }}
                >
                  {t("settings.platforms")}
                </div>
                {platforms.map((p) => {
                  const scope = `platform:${p.id}`;
                  return (
                    <button
                      key={p.id}
                      className="btn btn-ghost"
                      style={{
                        justifyContent: "flex-start",
                        padding: "6px 8px 6px 16px",
                        fontSize: 12,
                        fontWeight: selectedScope === scope ? 600 : 400,
                        color:
                          selectedScope === scope
                            ? "var(--accent)"
                            : "var(--text-secondary)",
                        background:
                          selectedScope === scope
                            ? "var(--accent-subtle)"
                            : "transparent",
                        borderRadius: "var(--radius-sm)",
                      }}
                      onClick={() => {
                        setSelectedScope(scope);
                        setMode("gui");
                      }}
                    >
                      {p.name}
                    </button>
                  );
                })}
              </>
            )}

            {/* Groups */}
            {groups.length > 0 && (
              <>
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 600,
                    color: "var(--text-tertiary)",
                    padding: "8px 8px 2px",
                    textTransform: "uppercase",
                    letterSpacing: "0.05em",
                  }}
                >
                  {t("settings.groups")}
                </div>
                {groups.map((g) => {
                  const scope = `group:${g.group.id}`;
                  return (
                    <button
                      key={g.group.id}
                      className="btn btn-ghost"
                      style={{
                        justifyContent: "flex-start",
                        padding: "6px 8px 6px 16px",
                        fontSize: 12,
                        fontWeight: selectedScope === scope ? 600 : 400,
                        color:
                          selectedScope === scope
                            ? "var(--accent)"
                            : "var(--text-secondary)",
                        background:
                          selectedScope === scope
                            ? "var(--accent-subtle)"
                            : "transparent",
                        borderRadius: "var(--radius-sm)",
                      }}
                      onClick={() => {
                        setSelectedScope(scope);
                        setMode("gui");
                      }}
                    >
                      {g.group.name}
                    </button>
                  );
                })}
              </>
            )}
          </div>

          {/* ── Right: Editor ── */}
          <div
            className="glass-surface"
            style={{
              flex: 1,
              padding: 16,
              display: "flex",
              flexDirection: "column",
              gap: 12,
              alignSelf: "flex-start",
              minWidth: 0,
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
                  setEditJson(JSON.stringify(effective, null, 2));
                  setMode("json");
                }}
              >
                {t("settings.jsonMode")}
              </button>
            </div>

            {/* JSON mode */}
            {mode === "json" && (
              <>
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
              </>
            )}

            {/* GUI mode */}
            {mode === "gui" && (
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: 14,
                }}
              >
                {/* Model */}
                <div>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      marginBottom: 4,
                    }}
                  >
                    <label
                      style={{
                        fontSize: 12,
                        fontWeight: 500,
                        color: "var(--text-secondary)",
                      }}
                    >
                      {t("settings.model")}
                    </label>
                    {selectedScope !== "global" && (
                      <AlignBadge
                        overridden={isOverridden("model")}
                        onReset={() => resetField("model")}
                        t={t}
                      />
                    )}
                  </div>
                  <input
                    className="input"
                    placeholder={t("settings.modelPlaceholder")}
                    value={effective.model ?? ""}
                    onChange={(e) =>
                      updateField("model", e.target.value || undefined)
                    }
                  />
                </div>

                {/* Effort Level */}
                <div>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      marginBottom: 4,
                    }}
                  >
                    <label
                      style={{
                        fontSize: 12,
                        fontWeight: 500,
                        color: "var(--text-secondary)",
                      }}
                    >
                      {t("settings.effortLevel")}
                    </label>
                    {selectedScope !== "global" && (
                      <AlignBadge
                        overridden={isOverridden("effortLevel")}
                        onReset={() => resetField("effortLevel")}
                        t={t}
                      />
                    )}
                  </div>
                  <select
                    className="input"
                    value={effective.effortLevel ?? ""}
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
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      marginBottom: 4,
                    }}
                  >
                    <label
                      style={{
                        fontSize: 12,
                        fontWeight: 500,
                        color: "var(--text-secondary)",
                      }}
                    >
                      {t("settings.outputStyle")}
                    </label>
                    {selectedScope !== "global" && (
                      <AlignBadge
                        overridden={isOverridden("outputStyle")}
                        onReset={() => resetField("outputStyle")}
                        t={t}
                      />
                    )}
                  </div>
                  <input
                    className="input"
                    placeholder="Explanatory"
                    value={effective.outputStyle ?? ""}
                    onChange={(e) =>
                      updateField("outputStyle", e.target.value || undefined)
                    }
                  />
                </div>

                {/* Language */}
                <div>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      marginBottom: 4,
                    }}
                  >
                    <label
                      style={{
                        fontSize: 12,
                        fontWeight: 500,
                        color: "var(--text-secondary)",
                      }}
                    >
                      {t("settings.language")}
                    </label>
                    {selectedScope !== "global" && (
                      <AlignBadge
                        overridden={isOverridden("language")}
                        onReset={() => resetField("language")}
                        t={t}
                      />
                    )}
                  </div>
                  <input
                    className="input"
                    placeholder="chinese, english, japanese..."
                    value={effective.language ?? ""}
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
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                    }}
                  >
                    <label
                      style={{
                        fontSize: 12,
                        fontWeight: 500,
                        color: "var(--text-secondary)",
                      }}
                    >
                      {t("settings.alwaysThinking")}
                    </label>
                    {selectedScope !== "global" && (
                      <AlignBadge
                        overridden={isOverridden("alwaysThinkingEnabled")}
                        onReset={() => resetField("alwaysThinkingEnabled")}
                        t={t}
                      />
                    )}
                  </div>
                  <div
                    className={`toggle ${
                      effective.alwaysThinkingEnabled ? "active" : ""
                    }`}
                    onClick={() =>
                      updateField(
                        "alwaysThinkingEnabled",
                        effective.alwaysThinkingEnabled ? undefined : true,
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
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      marginBottom: 8,
                    }}
                  >
                    <span
                      style={{
                        fontSize: 13,
                        fontWeight: 600,
                        color: "var(--text-primary)",
                      }}
                    >
                      {t("settings.permissions")}
                    </span>
                    {selectedScope !== "global" && (
                      <AlignBadge
                        overridden={permsOverridden}
                        onReset={() => resetField("permissions")}
                        t={t}
                      />
                    )}
                  </div>

                  {/* Allow */}
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
                        const next: Record<string, string[]> = { ...perms };
                        if (list.length === 0) { const { allow: _, ...rest } = next; Object.assign(next, rest); }
                        else { next.allow = list; }
                        updateField(
                          "permissions",
                          Object.keys(next).length > 0 ? next : undefined,
                        );
                      }}
                      addLabel={t("settings.addRule")}
                    />
                  </div>

                  {/* Ask */}
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
                        const next: Record<string, string[]> = { ...perms };
                        if (list.length === 0) { const { ask: _, ...rest } = next; Object.assign(next, rest); }
                        else { next.ask = list; }
                        updateField(
                          "permissions",
                          Object.keys(next).length > 0 ? next : undefined,
                        );
                      }}
                      addLabel={t("settings.addRule")}
                    />
                  </div>

                  {/* Deny */}
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
                        const next: Record<string, string[]> = { ...perms };
                        if (list.length === 0) { const { deny: _, ...rest } = next; Object.assign(next, rest); }
                        else { next.deny = list; }
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
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      marginBottom: 8,
                    }}
                  >
                    <span
                      style={{
                        fontSize: 13,
                        fontWeight: 600,
                        color: "var(--text-primary)",
                      }}
                    >
                      {t("settings.env")}
                    </span>
                    {selectedScope !== "global" && (
                      <AlignBadge
                        overridden={isOverridden("env")}
                        onReset={() => resetField("env")}
                        t={t}
                      />
                    )}
                  </div>
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

                {/* Extra fields */}
                {extraFields.length > 0 && (
                  <div
                    style={{
                      borderTop: "1px solid var(--border)",
                      paddingTop: 10,
                    }}
                  >
                    <div
                      style={{
                        fontSize: 12,
                        color: "var(--text-tertiary)",
                        marginBottom: 6,
                      }}
                    >
                      {t("settings.otherSettings")}
                    </div>
                    {extraFields.map((k) => (
                      <div
                        key={k}
                        style={{
                          display: "flex",
                          gap: 8,
                          alignItems: "center",
                          marginBottom: 4,
                        }}
                      >
                        <span
                          style={{
                            fontSize: 12,
                            fontWeight: 500,
                            color: "var(--text-secondary)",
                            width: 120,
                            flexShrink: 0,
                          }}
                        >
                          {k}
                        </span>
                        <span
                          style={{
                            fontSize: 12,
                            color: "var(--text-tertiary)",
                            fontFamily: "monospace",
                          }}
                        >
                          {JSON.stringify(effective[k])}
                        </span>
                      </div>
                    ))}
                    <div
                      style={{
                        fontSize: 11,
                        color: "var(--text-tertiary)",
                        marginTop: 4,
                      }}
                    >
                      {t("settings.editInJson")}
                    </div>
                  </div>
                )}
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
      )}
    </div>
  );
}
