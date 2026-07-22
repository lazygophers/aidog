// ─── Plugins Section (structured editor) ─────────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { IconClose } from "../../icons";
import { F, S } from "./tokens";
import { SvgIcon, ICON_PATHS } from "./icons";
import { Section, Toggle, Hint, SubHeading } from "./_shared";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

const MARKETPLACE_SOURCE_TYPES = ["github", "git", "url", "npm", "file", "directory", "settings", "hostPattern", "pathPattern"] as const;
type SourceType = typeof MARKETPLACE_SOURCE_TYPES[number];

const SOURCE_TYPE_LABELS: Record<SourceType, string> = {
  github: "GitHub",
  git: "Git URL",
  url: "URL (marketplace.json)",
  npm: "NPM Package",
  file: "File (marketplace.json)",
  directory: "Directory",
  settings: "Inline Settings",
  hostPattern: "Host Pattern (regex)",
  pathPattern: "Path Pattern (regex)",
};

/** Type-specific field definitions */
const SOURCE_FIELDS: Record<SourceType, { key: string; label: string; placeholder: string; required?: boolean }[]> = {
  github: [
    { key: "repo", label: "Repository", placeholder: "owner/repo", required: true },
    { key: "ref", label: "Ref (branch/tag/sha)", placeholder: "main" },
    { key: "path", label: "Subdirectory", placeholder: "marketplace" },
  ],
  git: [
    { key: "url", label: "Git URL", placeholder: "https://git.example.com/plugins.git", required: true },
    { key: "ref", label: "Ref (branch/tag/sha)", placeholder: "main" },
    { key: "path", label: "Subdirectory", placeholder: "marketplace" },
  ],
  url: [
    { key: "url", label: "Marketplace JSON URL", placeholder: "https://plugins.example.com/marketplace.json", required: true },
  ],
  npm: [
    { key: "package", label: "NPM Package", placeholder: "@acme-corp/claude-plugins", required: true },
  ],
  file: [
    { key: "path", label: "File Path", placeholder: "/usr/local/share/claude/marketplace.json", required: true },
  ],
  directory: [
    { key: "path", label: "Directory Path", placeholder: "/usr/local/share/claude/plugins", required: true },
  ],
  settings: [
    { key: "name", label: "Marketplace Name", placeholder: "team-tools", required: true },
  ],
  hostPattern: [
    { key: "hostPattern", label: "Host Pattern (regex)", placeholder: "^github\\.example\\.com$", required: true },
  ],
  pathPattern: [
    { key: "pathPattern", label: "Path Pattern (regex)", placeholder: "^/opt/approved/", required: true },
  ],
};

/** Source config for a single marketplace entry */
function MarketplaceSourceEditor({
  source,
  onChange,
  compact = false,
}: {
  source: Record<string, any>;
  onChange: (s: Record<string, any>) => void;
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const srcType = (source.source ?? "github") as SourceType;
  const fields = SOURCE_FIELDS[srcType] ?? [];
  const setField = (key: string, val: string | boolean) => {
    onChange({ ...source, [key]: val || undefined });
  };
  const fs = compact ? F.hint : F.body;
  const pad = compact ? "4px 8px" : "6px 10px";

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6, paddingLeft: 8, borderLeft: "2px solid var(--border)" }}>
      {/* Source type selector */}
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: compact ? 50 : 80, flexShrink: 0, whiteSpace: "nowrap" }}>Type</span>
        <Select  
          value={srcType}
          onValueChange={(v) => {
            const newType = v as SourceType;
            // Keep only source type, clear type-specific fields
            onChange({ source: newType });
          }}>
<SelectTrigger style={{ fontSize: fs, padding: pad, flex: 1 }}><SelectValue/></SelectTrigger>
<SelectContent>
          {MARKETPLACE_SOURCE_TYPES.map((t) => (
            <SelectItem key={t} value={t}>{SOURCE_TYPE_LABELS[t]}</SelectItem>
          ))}
        </SelectContent>
</Select>
      </div>

      {/* Type-specific fields */}
      {fields.map((f) => (
        <div key={f.key} style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: compact ? 50 : 80, flexShrink: 0, whiteSpace: "nowrap" }}>
            {f.label}{f.required && " *"}
          </span>
          <Input  style={{ fontSize: fs, padding: pad, flex: 1 }}
            placeholder={f.placeholder} value={source[f.key] ?? ""}
            onChange={(e) => setField(f.key, e.target.value)} />
        </div>
      ))}

      {/* skipLfs for github/git */}
      {(srcType === "github" || srcType === "git") && (
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: compact ? 50 : 80, flexShrink: 0, whiteSpace: "nowrap" }}>skipLfs</span>
          <Toggle active={!!source.skipLfs} onChange={(v) => setField("skipLfs", v)} />
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.plugins.skipLfs", "跳过 LFS 下载")}</span>
        </div>
      )}

      {/* URL headers for url type */}
      {srcType === "url" && (
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: 80, flexShrink: 0, whiteSpace: "nowrap" }}>Headers</span>
          <Input  style={{ fontSize: fs, padding: pad, flex: 1 }}
            placeholder='{"Authorization": "Bearer ${TOKEN}"}'
            value={source.headers ? JSON.stringify(source.headers) : ""}
            onChange={(e) => {
              try { onChange({ ...source, headers: JSON.parse(e.target.value) }); }
              catch { /* invalid JSON, keep as-is */ }
            }} />
        </div>
      )}

      {/* settings: inline plugins list */}
      {srcType === "settings" && (
        <>
          {(source.plugins as Array<Record<string, any>> | undefined)?.map((plug, pi) => (
            <div key={pi} style={{ display: "flex", gap: 4, alignItems: "flex-start", paddingLeft: 8, paddingTop: 4 }}>
              <Input  style={{ fontSize: F.hint, padding: "4px 8px", width: 100, flexShrink: 0 }}
                placeholder="plugin-name" value={plug.name ?? ""}
                onChange={(e) => {
                  const plugs = [...(source.plugins ?? [])];
                  plugs[pi] = { ...plug, name: e.target.value };
                  onChange({ ...source, plugins: plugs });
                }} />
              <div style={{ flex: 1 }}>
                <MarketplaceSourceEditor
                  source={plug.source ?? { source: "github" }}
                  onChange={(s) => {
                    const plugs = [...(source.plugins ?? [])];
                    plugs[pi] = { ...plug, source: s };
                    onChange({ ...source, plugins: plugs });
                  }}
                  compact
                />
              </div>
              <Button variant="outline" type="button" onClick={() => {
                const plugs = (source.plugins ?? []).filter((_: any, j: number) => j !== pi);
                onChange({ ...source, plugins: plugs.length > 0 ? plugs : undefined });
              }} style={{
                background: "none", border: "none", cursor: "pointer",
                color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1, flexShrink: 0,
              }}><IconClose size={12} /></Button>
            </div>
          ))}
          <Button variant="ghost" type="button"  style={{ fontSize: F.small, padding: "4px 10px", alignSelf: "flex-start", marginLeft: 8 }}
            onClick={() => {
              const plugs = [...(source.plugins ?? []), { name: "", source: { source: "github" } }];
              onChange({ ...source, plugins: plugs });
            }}>+ Plugin</Button>
        </>
      )}

      {/* autoUpdate toggle */}
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: compact ? 50 : 80, flexShrink: 0, whiteSpace: "nowrap" }}>auto</span>
        <Toggle active={!!source.autoUpdate} onChange={(v) => setField("autoUpdate", v)} />
        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.plugins.autoRefresh", "启动时自动刷新")}</span>
      </div>
    </div>
  );
}

/** Main plugins structured editor */
function PluginsEditor({
  config,
  updateField,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
}) {
  const { t } = useTranslation();
  const enabledPlugins = (config.enabledPlugins ?? {}) as Record<string, boolean>;
  const extraMarketplaces = (config.extraKnownMarketplaces ?? {}) as Record<string, any>;

  // ── Enabled Plugins ──
  const [newPluginKey, setNewPluginKey] = useState("");
  const pluginEntries = Object.entries(enabledPlugins);

  const setPluginEnabled = (key: string, val: boolean) => {
    const next = { ...enabledPlugins, [key]: val };
    updateField("enabledPlugins", next);
  };
  const addPlugin = () => {
    const k = newPluginKey.trim();
    if (!k) return;
    setPluginEnabled(k, true);
    setNewPluginKey("");
  };
  const removePlugin = (key: string) => {
    const next = { ...enabledPlugins };
    delete next[key];
    updateField("enabledPlugins", Object.keys(next).length > 0 ? next : undefined);
  };

  // ── Extra Marketplaces ──
  const [newMktName, setNewMktName] = useState("");
  const mktEntries = Object.entries(extraMarketplaces);

  const addMarketplace = () => {
    const name = newMktName.trim();
    if (!name) return;
    const next = { ...extraMarketplaces, [name]: { source: { source: "github" } } };
    updateField("extraKnownMarketplaces", next);
    setNewMktName("");
  };
  const updateMarketplace = (name: string, val: any) => {
    const next = { ...extraMarketplaces, [name]: val };
    updateField("extraKnownMarketplaces", next);
  };
  const removeMarketplace = (name: string) => {
    const next = { ...extraMarketplaces };
    delete next[name];
    updateField("extraKnownMarketplaces", Object.keys(next).length > 0 ? next : undefined);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.sectionGap }}>
      {/* ── Enabled Plugins ── */}
      <div>
        <SubHeading>
          <SvgIcon d={ICON_PATHS.plugins} size={14} style={{ opacity: 0.6 }} />
          Enabled Plugins
        </SubHeading>
        <Hint>{t("settings.plugins.enabledHint", "格式: plugin-name@marketplace → 启用/禁用")}</Hint>
        <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 8 }}>
          {pluginEntries.map(([key, val]) => (
            <div key={key} style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <code style={{
                flex: 1, fontSize: F.hint, padding: "6px 10px",
                background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
                color: "var(--text-primary)", fontFamily: "monospace",
                overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
              }}>
                {key}
              </code>
              <Toggle active={val} onChange={(v) => setPluginEnabled(key, v)} />
              <Button variant="outline" type="button" onClick={() => removePlugin(key)}
                title={t("settings.plugins.removePlugin", "删除插件")}
                aria-label={t("settings.plugins.removePlugin", "删除插件")}
                style={{
                  background: "none", border: "none", cursor: "pointer",
                  color: "var(--text-secondary)", padding: 4, lineHeight: 1,
                  marginLeft: 4, display: "inline-flex", alignItems: "center",
                }}
                onMouseEnter={(e) => { e.currentTarget.style.color = "var(--danger)"; }}
                onMouseLeave={(e) => { e.currentTarget.style.color = "var(--text-secondary)"; }}
              ><SvgIcon d={ICON_PATHS.trash} size={15} /></Button>
            </div>
          ))}
          <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 2 }}>
            <Input
              
              style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
              placeholder="plugin-name@marketplace"
              value={newPluginKey}
              onChange={(e) => setNewPluginKey(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addPlugin()}
            />
            <Button variant="ghost" type="button"  style={{ fontSize: F.small, padding: "4px 12px" }}
              onClick={addPlugin}>+</Button>
          </div>
        </div>
      </div>

      {/* ── Extra Marketplaces ── */}
      <div>
        <SubHeading>
          <SvgIcon d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2Z" size={14} style={{ opacity: 0.6 }} />
          Extra Marketplaces
        </SubHeading>
        <Hint>{t("settings.plugins.marketplacesHint", "命名市场源定义（github / git / directory / settings）")}</Hint>
        <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 8 }}>
          {mktEntries.map(([name, mktConfig]) => (
            <div key={name} style={{
              padding: "10px 12px", background: "var(--bg-glass)",
              borderRadius: "var(--radius-md)", display: "flex", flexDirection: "column", gap: 6,
            }}>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <span style={{
                  fontSize: F.body, fontWeight: 600, color: "var(--accent)",
                  fontFamily: "monospace",
                }}>{name}</span>
                <div style={{ flex: 1 }} />
                <Button variant="outline" type="button" onClick={() => removeMarketplace(name)}
                  title={t("settings.plugins.removeMarketplace", "删除市场源")}
                  aria-label={t("settings.plugins.removeMarketplace", "删除市场源")}
                  style={{
                    background: "none", border: "none", cursor: "pointer",
                    color: "var(--text-secondary)", padding: 4, lineHeight: 1,
                    marginLeft: 4, display: "inline-flex", alignItems: "center",
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.color = "var(--danger)"; }}
                  onMouseLeave={(e) => { e.currentTarget.style.color = "var(--text-secondary)"; }}
                ><SvgIcon d={ICON_PATHS.trash} size={15} /></Button>
              </div>
              <MarketplaceSourceEditor
                source={mktConfig.source ?? { source: "github" }}
                onChange={(s) => updateMarketplace(name, { ...mktConfig, source: s })}
              />
              {/* Path field — local installation path */}
              <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 2 }}>
                <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: 80, flexShrink: 0, whiteSpace: "nowrap" }}>Path</span>
                <Input  style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                  placeholder={t("settings.plugins.localPathPh", "本地安装路径（留空自动管理）")}
                  value={mktConfig.path ?? ""}
                  onChange={(e) => updateMarketplace(name, { ...mktConfig, path: e.target.value || undefined })}
                />
              </div>
            </div>
          ))}
          <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 2 }}>
            <Input
              
              style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
              placeholder="marketplace-name"
              value={newMktName}
              onChange={(e) => setNewMktName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addMarketplace()}
            />
            <Button variant="ghost" type="button"  style={{ fontSize: F.small, padding: "4px 12px" }}
              onClick={addMarketplace}>+</Button>
          </div>
        </div>
      </div>
    </div>
  );
}

/** Plugins with Section wrapper — for card-based layout */
export function PluginsSection({
  config,
  updateField,
  t,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  return (
    <Section title={t("settings.sectionPlugins")} defaultOpen>
      <PluginsEditor config={config} updateField={updateField} />
    </Section>
  );
}

/** Plugins without Section wrapper — for tab content pane */
export function PluginsSectionInline({ config, updateField }: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
}) {
  return <PluginsEditor config={config} updateField={updateField} />;
}
