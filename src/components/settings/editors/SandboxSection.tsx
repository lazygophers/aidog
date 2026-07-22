// ─── Sandbox Section (structured editor) ────────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).
// ponytail: 临界行数，超 800 再抽 <SandboxPathInput>。

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { IconClose } from "../../icons";
import { F, S } from "./tokens";
import { SvgIcon } from "./icons";
import { Toggle, Section, FieldRow, PathInput, Hint, SubHeading } from "./_shared";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";


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
          
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={placeholder}
          onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); add(); } }}
          style={{ flex: 1, fontSize: F.hint, fontFamily: "monospace", padding: "6px 10px" }}
        />
        <Button variant="outline" type="button" disabled={!draft.trim()} onClick={add}
          style={{
            background: "var(--primary)", color: "var(--primary-foreground)", border: "none", borderRadius: "var(--radius-sm)",
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
            placeholder={placeholder ?? t("settings.editor.dirOrPathPh", "选择目录或输入路径…")}
          />
        </div>
        <Button variant="outline" type="button" disabled={!draftStr.trim()} onClick={add}
          style={{
            background: "var(--primary)", color: "var(--primary-foreground)", border: "none", borderRadius: "var(--radius-sm)",
            padding: "5px 10px", fontSize: F.hint, cursor: draftStr.trim() ? "pointer" : "default",
            opacity: draftStr.trim() ? 1 : 0.4, flexShrink: 0,
          }}>+</Button>
      </div>
    </div>
  );
}

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
    // Remove empty arrays and falsy booleans at top level
    for (const k of Object.keys(next)) {
      if (Array.isArray(next[k]) && next[k].length === 0) delete next[k];
      if (next[k] === false || next[k] === undefined) delete next[k];
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
            {t("settings.sandbox.enable", "启用沙箱")}
          </div>
          <Hint>{t("settings.sandbox.enableDesc", "Bash 命令及其子进程的文件系统和网络隔离 (Seatbelt / bubblewrap)")}</Hint>
        </div>
        {enabled && (
          <span style={{
            fontSize: F.small, fontWeight: 600, color: "var(--color-success)",
            padding: "2px 8px", background: "color-mix(in srgb, var(--color-success) 12%, transparent)", borderRadius: "var(--radius-sm)",
          }}>● {t("settings.sandbox.enabled", "已启用")}</span>
        )}
      </div>

      {!enabled && (
        <div style={{
          fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.6,
          padding: "10px 14px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
        }}>
          {t("settings.sandbox.disabledHint", "启用后，Claude 运行的每个 Bash 命令将被限制在指定的文件系统和网络边界内。macOS 使用 Seatbelt，Linux/WSL2 使用 bubblewrap。不支持原生 Windows。")}
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
              {t("settings.sandbox.fsIsolation", "文件系统隔离")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.fsIsolationDesc", "默认：可读整个文件系统，仅可写当前工作目录。路径前缀：/（绝对）、~/（主目录）、./（项目相对）")}
            </Hint>

            <FieldRow label={t("settings.sandbox.allowWrite", "允许写入")}>
              <PathList
                items={fs.allowWrite ?? []}
                onChange={(v) => setFsArray("allowWrite", v)}
                placeholder={t("settings.sandbox.allowWritePh", "如 ~/.kube, /tmp/build")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.denyWrite", "拒绝写入")}>
              <PathList
                items={fs.denyWrite ?? []}
                onChange={(v) => setFsArray("denyWrite", v)}
                placeholder={t("settings.sandbox.denyWritePh", "如 ~/.bashrc, /etc")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.allowRead", "允许读取")}>
              <PathList
                items={fs.allowRead ?? []}
                onChange={(v) => setFsArray("allowRead", v)}
                placeholder={t("settings.sandbox.allowReadPh", "如 .（项目目录）")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.denyRead", "拒绝读取")}>
              <PathList
                items={fs.denyRead ?? []}
                onChange={(v) => setFsArray("denyRead", v)}
                placeholder={t("settings.sandbox.denyReadPh", "如 ~/（阻止读主目录）, ~/.ssh")}
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
              {t("settings.sandbox.netIsolation", "网络隔离")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.netIsolationDesc", "默认：无预允许域名。命令首次需要新域名时提示批准。设置 allowedDomains 可预授权域名。")}
            </Hint>

            <FieldRow label={t("settings.sandbox.allowedDomains", "允许域名")}>
              <TagList
                items={net.allowedDomains ?? []}
                onChange={(v) => setNetArray("allowedDomains", v)}
                placeholder={t("settings.sandbox.allowedDomainsPh", "如 api.anthropic.com, *.github.com")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.deniedDomains", "拒绝域名")}>
              <TagList
                items={net.deniedDomains ?? []}
                onChange={(v) => setNetArray("deniedDomains", v)}
                placeholder={t("settings.sandbox.deniedDomainsPh", "即使 allowedDomains 通配符允许，也会被阻止")}
              />
            </FieldRow>

            <div style={{ display: "flex", gap: 16, flexWrap: "wrap" }}>
              <FieldRow label={t("settings.sandbox.httpProxy", "HTTP 代理")}>
                <Input
                  
                  type="number"
                  value={net.httpProxyPort ?? ""}
                  onChange={(e) => setNetPort("httpProxyPort", e.target.value)}
                  placeholder={t("settings.sandbox.port", "端口")}
                  style={{ width: 100, fontSize: F.hint, padding: "6px 10px" }}
                />
              </FieldRow>
              <FieldRow label={t("settings.sandbox.socksProxy", "SOCKS 代理")}>
                <Input
                  
                  type="number"
                  value={net.socksProxyPort ?? ""}
                  onChange={(e) => setNetPort("socksProxyPort", e.target.value)}
                  placeholder={t("settings.sandbox.port", "端口")}
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
              {t("settings.sandbox.safety", "安全与策略")}
            </SubHeading>

            <FieldRow label={t("settings.sandbox.failIfUnavailable", "不可用时报错")}>
              <Toggle active={!!sb.failIfUnavailable} onChange={(v) => toggleSb("failIfUnavailable", v)} />
              <Hint>{t("settings.sandbox.failIfUnavailableDesc", "缺少依赖时阻止启动而非回退到非沙箱执行")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.noEscape", "禁止逃逸")}>
              <Toggle active={sb.allowUnsandboxedCommands === false} onChange={(v) => sync({ allowUnsandboxedCommands: !v })} />
              <Hint>{t("settings.sandbox.noEscapeDesc", "禁用 dangerouslyDisableSandbox 逃生舱，所有命令必须沙箱化")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.lockDomains", "锁定域名")}>
              <Toggle active={!!net.allowManagedDomainsOnly} onChange={(v) => sync({ network: { ...net, allowManagedDomainsOnly: v } })} />
              <Hint>{t("settings.sandbox.lockDomainsDesc", "仅尊重托管设置的 allowedDomains，忽略本地配置")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.lockReadPaths", "锁定读取路径")}>
              <Toggle active={!!sb.allowManagedReadPathsOnly} onChange={(v) => toggleSb("allowManagedReadPathsOnly", v)} />
              <Hint>{t("settings.sandbox.lockReadPathsDesc", "仅尊重托管设置的 allowRead，忽略本地配置")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.weakNet", "弱网络隔离")}>
              <Toggle active={!!sb.enableWeakerNetworkIsolation} onChange={(v) => toggleSb("enableWeakerNetworkIsolation", v)} />
              <Hint>{t("settings.sandbox.weakNetDesc", "MITM 代理 + 自定义 CA 场景下启用")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.weakNested", "弱嵌套沙箱")}>
              <Toggle active={!!sb.enableWeakerNestedSandbox} onChange={(v) => toggleSb("enableWeakerNestedSandbox", v)} />
              <Hint>{t("settings.sandbox.weakNestedDesc", "无特权容器内运行时启用（绑定挂载 /proc 而非新建）")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.unixSockets", "Unix 套接字")}>
              <Toggle active={!!sb.allowUnixSockets} onChange={(v) => toggleSb("allowUnixSockets", v)} />
              <Hint>{t("settings.sandbox.unixSocketsDesc", "允许 Unix 域套接字访问（注意：Docker socket 等可能绕过沙箱）")}</Hint>
            </FieldRow>
          </div>

          {/* ── Excluded Commands ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 10,
          }}>
            <SubHeading>
              <SvgIcon d="M18 6L6 18M6 6l12 12" size={15} />
              {t("settings.sandbox.excludedCommands", "排除命令")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.excludedCommandsDesc", "列出的命令在沙箱外运行（如 docker, gh, terraform 等与沙箱不兼容的工具）")}
            </Hint>
            <TagList
              items={sb.excludedCommands ?? []}
              onChange={setExcludedCommands}
              placeholder={t("settings.sandbox.excludedCommandsPh", "如 docker, gh, terraform, watchman")}
            />
          </div>
        </>
      )}
    </div>
  );
}

/** Sandbox with Section wrapper — for card-based layout */
export function SandboxSection({
  sandboxValue,
  updateField,
  t,
}: {
  sandboxValue: Record<string, any> | undefined;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  return (
    <Section title={t("settings.sectionSandbox")} defaultOpen>
      <SandboxEditor sandboxValue={sandboxValue} updateField={updateField} />
    </Section>
  );
}

/** Sandbox without Section wrapper — for tab content pane */
export function SandboxSectionInline({ sandboxValue, updateField }: {
  sandboxValue: Record<string, any> | undefined;
  updateField: (field: string, value: any) => void;
}) {
  return <SandboxEditor sandboxValue={sandboxValue} updateField={updateField} />;
}
