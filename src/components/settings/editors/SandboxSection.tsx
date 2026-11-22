// ─── Sandbox Section (structured editor) ────────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).
// ponytail: 临界行数，超 800 再抽 <SandboxPathInput>。

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { IconClose } from "../../icons";
import { F, S } from "./tokens";
import { SvgIcon } from "./icons";
import { Toggle, FieldRow, PathInput, Hint, SubHeading } from "./_shared";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { isImeComposing } from "../../../utils/ime";


/** Editable string list with add/remove — plain text input */
function TagList({
  items,
  onChange,
  placeholder,
}: {
  items: string[];
  onChange: (v: string[]) => void;
  placeholder?: string;
}) {
  const [draft, setDraft] = useState("");
  const add = () => {
    const v = draft.trim();
    if (v && !items.includes(v)) onChange([...items, v]);
    setDraft("");
  };
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {items.map((p, i) => (
        <div key={i} style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <code style={{
            flex: 1, fontSize: F.hint, padding: "6px 10px",
            background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
            color: "var(--text-primary)", fontFamily: "monospace",
            overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
          }}>
            {p}
          </code>
          <Button variant="outline" type="button" onClick={() => onChange(items.filter((_, j) => j !== i))}
            style={{
              background: "none", border: "none", cursor: "pointer",
              color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1,
            }}><IconClose size={12} /></Button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <Input
          
          data-testid="path-list-draft"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={placeholder}
          onKeyDown={(e) => { if (e.key === "Enter" && !isImeComposing(e)) { e.preventDefault(); add(); } }}
          style={{ flex: 1, fontSize: F.hint, fontFamily: "monospace", padding: "6px 10px" }}
        />
        <Button variant="outline" type="button" disabled={!draft.trim()} onClick={add}
          style={{
            background: "var(--primary)", color: "var(--primary-foreground)", border: "1px solid var(--accent-edge)", borderRadius: "var(--radius-sm)",
            padding: "5px 10px", fontSize: F.hint, cursor: draft.trim() ? "pointer" : "default",
            opacity: draft.trim() ? 1 : 0.4,
          }}>+</Button>
      </div>
    </div>
  );
}

/** Editable path list with add/remove — uses PathInput with directory picker + autocomplete */
function PathList({
  items,
  onChange,
  placeholder,
}: {
  items: string[];
  onChange: (v: string[]) => void;
  placeholder?: string;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<string | undefined>(undefined);
  const draftStr = draft ?? "";
  const add = () => {
    const v = draftStr.trim();
    if (v && !items.includes(v)) onChange([...items, v]);
    setDraft(undefined);
  };
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {items.map((p, i) => (
        <div key={i} style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <code style={{
            flex: 1, fontSize: F.hint, padding: "6px 10px",
            background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
            color: "var(--text-primary)", fontFamily: "monospace",
            overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
          }}>
            {p}
          </code>
          <Button variant="outline" type="button" onClick={() => onChange(items.filter((_, j) => j !== i))}
            style={{
              background: "none", border: "none", cursor: "pointer",
              color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1,
            }}><IconClose size={12} /></Button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6, alignItems: "stretch" }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <PathInput
            value={draft}
            onChange={setDraft}
            pathType="directory"
            placeholder={placeholder ?? t("settings.editor.dirOrPathPh", "Choose a directory or type a path…")}
          />
        </div>
        <Button variant="outline" type="button" disabled={!draftStr.trim()} onClick={add}
          style={{
            background: "var(--primary)", color: "var(--primary-foreground)", border: "1px solid var(--accent-edge)", borderRadius: "var(--radius-sm)",
            padding: "5px 10px", fontSize: F.hint, cursor: draftStr.trim() ? "pointer" : "default",
            opacity: draftStr.trim() ? 1 : 0.4, flexShrink: 0,
          }}>+</Button>
      </div>
    </div>
  );
}

/** 这些 key 的默认值是 true，所以存 `false` 有意义，不能当「与默认值相同」删掉。 */
const FALSE_IS_MEANINGFUL = new Set(["allowUnsandboxedCommands"]);

function SandboxEditor({
  sandboxValue,
  updateField,
}: {
  sandboxValue: Record<string, any> | undefined;
  updateField: (field: string, value: any) => void;
}) {
  const { t } = useTranslation();
  const sb = sandboxValue ?? {};
  const fs = sb.filesystem ?? {};
  const net = sb.network ?? {};
  const enabled = !!sb.enabled;

  const sync = (patch: Record<string, any>) => {
    const next = { ...sb, ...patch };
    // Remove empty arrays and falsy booleans at top level.
    // 例外：`allowUnsandboxedCommands` 的默认值是 true，`false` 才是用户的选择
    //（界面上叫「禁止逃逸」，读的是 `=== false`）。跟着别的 key 一起删掉，就等于
    // 这个开关点了存不进去、自己弹回来。其余 boolean 都用 `!!x` 读，默认 false，
    // 删掉 false 只是省掉一个与默认值相同的字段，是对的。
    for (const k of Object.keys(next)) {
      if (Array.isArray(next[k]) && next[k].length === 0) delete next[k];
      if (next[k] === undefined) delete next[k];
      if (next[k] === false && !FALSE_IS_MEANINGFUL.has(k)) delete next[k];
    }
    // Clean empty sub-objects
    if (next.filesystem) {
      const fso = next.filesystem as Record<string, any>;
      for (const k of Object.keys(fso)) {
        if (Array.isArray(fso[k]) && fso[k].length === 0) delete fso[k];
      }
      if (Object.keys(fso).length === 0) delete next.filesystem;
    }
    if (next.network) {
      const no = next.network as Record<string, any>;
      for (const k of Object.keys(no)) {
        if (Array.isArray(no[k]) && no[k].length === 0) delete no[k];
        if (no[k] === false || no[k] === undefined) delete no[k];
      }
      if (Object.keys(no).length === 0) delete next.network;
    }
    updateField("sandbox", Object.keys(next).length > 0 ? next : undefined);
  };

  const toggleSb = (key: string, val: boolean) => {
    sync({ [key]: val });
  };

  const setFsArray = (key: string, arr: string[]) => {
    sync({ filesystem: { ...fs, [key]: arr } });
  };

  const setNetArray = (key: string, arr: string[]) => {
    sync({ network: { ...net, [key]: arr } });
  };

  const setNetPort = (key: string, val: string) => {
    const port = parseInt(val, 10);
    if (val && (isNaN(port) || port < 0 || port > 65535)) return;
    sync({ network: { ...net, [key]: val ? port : undefined } });
  };

  const setExcludedCommands = (arr: string[]) => {
    sync({ excludedCommands: arr });
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* ── Enable Toggle ── */}
      <div style={{
        display: "flex", alignItems: "center", gap: 12,
        padding: "12px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
      }}>
        <Toggle active={enabled} onChange={(v) => sync({ enabled: v })} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("settings.sandbox.enable", "Enable Sandbox")}
          </div>
          <Hint>{t("settings.sandbox.enableDesc", "Filesystem and network isolation for Bash commands and their child processes (Seatbelt / bubblewrap)")}</Hint>
        </div>
        {enabled && (
          <span style={{
            fontSize: F.small, fontWeight: 600, color: "var(--color-success)",
            padding: "2px 8px", background: "color-mix(in srgb, var(--color-success) 12%, transparent)", borderRadius: "var(--radius-sm)",
          }}>● {t("settings.sandbox.enabled", "Enabled")}</span>
        )}
      </div>

      {!enabled && (
        <div style={{
          fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.6,
          padding: "10px 14px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
        }}>
          {t("settings.sandbox.disabledHint", "Once enabled, every Bash command Claude runs is confined to the specified filesystem and network boundaries. macOS uses Seatbelt, Linux/WSL2 uses bubblewrap. Native Windows is not supported.")}
        </div>
      )}

      {enabled && (
        <>
          {/* ── Filesystem Isolation ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 12,
          }}>
            <SubHeading>
              <SvgIcon d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" size={15} />
              {t("settings.sandbox.fsIsolation", "Filesystem Isolation")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.fsIsolationDesc", "Default: read the entire filesystem, write only the current working directory. Path prefixes: / (absolute), ~/ (home), ./ (project-relative)")}
            </Hint>

            <FieldRow label={t("settings.sandbox.allowWrite", "Allow Write")}>
              <PathList
                items={fs.allowWrite ?? []}
                onChange={(v) => setFsArray("allowWrite", v)}
                placeholder={t("settings.sandbox.allowWritePh", "e.g. ~/.kube, /tmp/build")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.denyWrite", "Deny Write")}>
              <PathList
                items={fs.denyWrite ?? []}
                onChange={(v) => setFsArray("denyWrite", v)}
                placeholder={t("settings.sandbox.denyWritePh", "e.g. ~/.bashrc, /etc")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.allowRead", "Allow Read")}>
              <PathList
                items={fs.allowRead ?? []}
                onChange={(v) => setFsArray("allowRead", v)}
                placeholder={t("settings.sandbox.allowReadPh", "e.g. . (project directory)")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.denyRead", "Deny Read")}>
              <PathList
                items={fs.denyRead ?? []}
                onChange={(v) => setFsArray("denyRead", v)}
                placeholder={t("settings.sandbox.denyReadPh", "e.g. ~/ (blocks reading home), ~/.ssh")}
              />
            </FieldRow>
          </div>

          {/* ── Network Isolation ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 12,
          }}>
            <SubHeading>
              <SvgIcon d="M12 2a10 10 0 100 20 10 10 0 000-20zM2 12h20M12 2a15 15 0 014 10 15 15 0 01-4 10 15 15 0 01-4-10A15 15 0 0112 2z" size={15} />
              {t("settings.sandbox.netIsolation", "Network Isolation")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.netIsolationDesc", "Default: no pre-allowed domains. Prompts for approval the first time a command needs a new domain. Set allowedDomains to pre-authorize domains.")}
            </Hint>

            <FieldRow label={t("settings.sandbox.allowedDomains", "Allowed Domains")}>
              <TagList
                items={net.allowedDomains ?? []}
                onChange={(v) => setNetArray("allowedDomains", v)}
                placeholder={t("settings.sandbox.allowedDomainsPh", "e.g. api.anthropic.com, *.github.com")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.deniedDomains", "Denied Domains")}>
              <TagList
                items={net.deniedDomains ?? []}
                onChange={(v) => setNetArray("deniedDomains", v)}
                placeholder={t("settings.sandbox.deniedDomainsPh", "Blocked even if allowedDomains wildcards permit")}
              />
            </FieldRow>

            <div style={{ display: "flex", gap: 16, flexWrap: "wrap" }}>
              <FieldRow label={t("settings.sandbox.httpProxy", "HTTP Proxy")}>
                <Input
                  
                  type="number"
                  value={net.httpProxyPort ?? ""}
                  onChange={(e) => setNetPort("httpProxyPort", e.target.value)}
                  placeholder={t("settings.sandbox.port", "Port")}
                  style={{ width: 100, fontSize: F.hint, padding: "6px 10px" }}
                />
              </FieldRow>
              <FieldRow label={t("settings.sandbox.socksProxy", "SOCKS Proxy")}>
                <Input
                  
                  type="number"
                  value={net.socksProxyPort ?? ""}
                  onChange={(e) => setNetPort("socksProxyPort", e.target.value)}
                  placeholder={t("settings.sandbox.port", "Port")}
                  style={{ width: 100, fontSize: F.hint, padding: "6px 10px" }}
                />
              </FieldRow>
            </div>
          </div>

          {/* ── Safety & Policy Toggles ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 10,
          }}>
            <SubHeading>
              <SvgIcon d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10zM9 12l2 2 4-4" size={15} />
              {t("settings.sandbox.safety", "Safety & Policy")}
            </SubHeading>

            <FieldRow label={t("settings.sandbox.failIfUnavailable", "Fail If Unavailable")}>
              <Toggle active={!!sb.failIfUnavailable} onChange={(v) => toggleSb("failIfUnavailable", v)} />
              <Hint>{t("settings.sandbox.failIfUnavailableDesc", "Block startup instead of falling back to non-sandboxed execution when dependencies are missing")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.noEscape", "No Escape")}>
              <Toggle active={sb.allowUnsandboxedCommands === false} onChange={(v) => sync({ allowUnsandboxedCommands: !v })} />
              <Hint>{t("settings.sandbox.noEscapeDesc", "Disable the dangerouslyDisableSandbox escape hatch; all commands must be sandboxed")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.lockDomains", "Lock Domains")}>
              <Toggle active={!!net.allowManagedDomainsOnly} onChange={(v) => sync({ network: { ...net, allowManagedDomainsOnly: v } })} />
              <Hint>{t("settings.sandbox.lockDomainsDesc", "Only honor allowedDomains from managed settings, ignore local config")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.lockReadPaths", "Lock Read Paths")}>
              <Toggle active={!!sb.allowManagedReadPathsOnly} onChange={(v) => toggleSb("allowManagedReadPathsOnly", v)} />
              <Hint>{t("settings.sandbox.lockReadPathsDesc", "Only honor allowRead from managed settings, ignore local config")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.weakNet", "Weaker Network Isolation")}>
              <Toggle active={!!sb.enableWeakerNetworkIsolation} onChange={(v) => toggleSb("enableWeakerNetworkIsolation", v)} />
              <Hint>{t("settings.sandbox.weakNetDesc", "Enable for MITM proxy + custom CA scenarios")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.weakNested", "Weaker Nested Sandbox")}>
              <Toggle active={!!sb.enableWeakerNestedSandbox} onChange={(v) => toggleSb("enableWeakerNestedSandbox", v)} />
              <Hint>{t("settings.sandbox.weakNestedDesc", "Enable when running inside an unprivileged container (bind-mount /proc instead of creating a new one)")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.unixSockets", "Unix Sockets")}>
              <Toggle active={!!sb.allowUnixSockets} onChange={(v) => toggleSb("allowUnixSockets", v)} />
              <Hint>{t("settings.sandbox.unixSocketsDesc", "Allow Unix domain socket access (note: Docker socket etc. may bypass the sandbox)")}</Hint>
            </FieldRow>
          </div>

          {/* ── Excluded Commands ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 10,
          }}>
            <SubHeading>
              <SvgIcon d="M18 6L6 18M6 6l12 12" size={15} />
              {t("settings.sandbox.excludedCommands", "Excluded Commands")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.excludedCommandsDesc", "Listed commands run outside the sandbox (e.g. docker, gh, terraform and other tools incompatible with the sandbox)")}
            </Hint>
            <TagList
              items={sb.excludedCommands ?? []}
              onChange={setExcludedCommands}
              placeholder={t("settings.sandbox.excludedCommandsPh", "e.g. docker, gh, terraform, watchman")}
            />
          </div>
        </>
      )}
    </div>
  );
}

/** Sandbox without Section wrapper — for tab content pane */
export function SandboxSectionInline({ sandboxValue, updateField }: {
  sandboxValue: Record<string, any> | undefined;
  updateField: (field: string, value: any) => void;
}) {
  return <SandboxEditor sandboxValue={sandboxValue} updateField={updateField} />;
}
