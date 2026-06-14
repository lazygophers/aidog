// ─── MCP 管理页 ────────────────────────────────────────────
// 顶层侧栏入口。集中管 Claude Code / Codex 的 MCP server 配置。
// - 扫描导入：拉两 agent 配置 → 去重合并 → 勾选入 aidog DB（enabled = 来源 agent）。
// - per-agent 启用/禁用：每行右侧 claude/codex 图标，启用=accent，禁用=grayscale。
// - 删除：从 DB + 所有 enabled agent 配置移除（二次确认）。
//
// env/headers 敏感值经后端 mask_env 脱敏（***）。写 agent 配置用 DB 原值。
// transport: stdio（command+args）/ http|sse（url+headers）。codex 仅支持 stdio。

import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import {
  mcpApi,
  type McpServerInfo,
  type McpScanItem,
  type McpAgentSlug,
  type McpImportPayload,
} from "../services/api";
import claudeIcon from "../assets/platforms/claude_code.svg";
import codexIcon from "../assets/platforms/openai.svg";

const AGENTS: McpAgentSlug[] = ["claude-code", "codex"];
const AGENT_ICONS: Record<McpAgentSlug, string> = {
  "claude-code": claudeIcon,
  codex: codexIcon,
};

/** codex 仅 stdio；http/sse MCP 不能给 codex 启用。 */
function agentSupported(transport: string, agent: McpAgentSlug): boolean {
  if (agent === "codex") return transport === "stdio";
  return true; // claude-code 支持 stdio/http/sse
}

/** transport 配色 badge。 */
function transportStyle(transport: string): { bg: string; fg: string } {
  switch (transport) {
    case "http":
    case "sse":
      // 远程传输走 accent 系；区分靠 transport 文字本身。
      return { bg: "var(--accent-subtle)", fg: "var(--accent)" };
    default:
      return { bg: "var(--bg-elevated)", fg: "var(--text-tertiary)" };
  }
}

/** 摘要：stdio→command + 首参；http/sse→url。 */
function summaryOf(m: { transport: string; command: string; args: string[]; url: string }): string {
  if (m.transport === "stdio") {
    const first = m.args[0] ?? "";
    return [m.command, first].filter(Boolean).join(" ");
  }
  return m.url || "—";
}

export function Mcp() {
  const { t } = useTranslation();
  const [servers, setServers] = useState<McpServerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [message, setMessage] = useState<{ kind: "ok" | "err"; text: string } | null>(null);

  // 扫描导入 modal
  const [scanOpen, setScanOpen] = useState(false);
  const [scanItems, setScanItems] = useState<McpScanItem[]>([]);
  const [scanning, setScanning] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [importing, setImporting] = useState(false);

  // 删除确认 modal
  const [deleteTarget, setDeleteTarget] = useState<McpServerInfo | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await mcpApi.list();
      setServers(list);
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // ─── per-agent 切换（乐观更新） ───
  const handleToggle = async (srv: McpServerInfo, agent: McpAgentSlug) => {
    if (busyKey !== null) return;
    const enabled = srv.enabledAgents.includes(agent);
    if (enabled) {
      // 禁用总是允许
    } else if (!agentSupported(srv.transport, agent)) {
      setMessage({
        kind: "err",
        text: t("mcp.unsupportedTransport", {
          transport: srv.transport,
          agent: t(`mcp.agent.${agent}`),
          defaultValue: "不支持",
        }),
      });
      return;
    }
    setBusyKey(`${srv.name}::${agent}`);
    setMessage(null);
    const prev = servers;
    setServers((list) =>
      list.map((s) =>
        s.name === srv.name
          ? {
              ...s,
              enabledAgents: enabled
                ? s.enabledAgents.filter((a) => a !== agent)
                : [...s.enabledAgents, agent],
            }
          : s,
      ),
    );
    try {
      await mcpApi.setAgent(srv.name, agent, !enabled);
      setMessage({
        kind: "ok",
        text: t(enabled ? "mcp.disabled" : "mcp.enabled", "操作成功"),
      });
    } catch (e) {
      setServers(prev); // 回滚
      setMessage({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  // ─── 扫描 ───
  const openScan = async () => {
    setScanOpen(true);
    setScanning(true);
    setSelected(new Set());
    try {
      const items = await mcpApi.scan();
      setScanItems(items);
      // 默认预选所有未导入项
      setSelected(new Set(items.filter((i) => !i.alreadyImported).map((i) => i.name)));
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
      setScanOpen(false);
    } finally {
      setScanning(false);
    }
  };

  const handleImport = async () => {
    if (selected.size === 0) return;
    setImporting(true);
    setMessage(null);
    try {
      const payload: McpImportPayload[] = scanItems
        .filter((it) => selected.has(it.name))
        .map((it) => ({
          name: it.name,
          transport: it.transport,
          command: it.command,
          args: it.args,
          env: it.env,
          url: it.url,
          headers: it.headers,
          // 取首个发现 agent 作来源（启用初始 = 该 agent）
          sourceAgent: it.foundInAgents[0] ?? "claude-code",
        }));
      const report = await mcpApi.import(payload);
      await refresh();
      setScanOpen(false);
      const skipped = report.skipped.length;
      setMessage({
        kind: skipped > 0 ? "err" : "ok",
        text:
          skipped > 0
            ? t("mcp.importPartial", {
                ok: report.imported.length,
                skip: skipped,
                defaultValue: `导入 ${report.imported.length}，跳过 ${skipped}`,
              })
            : t("mcp.imported", {
                count: report.imported.length,
                defaultValue: `已导入 ${report.imported.length}`,
              }),
      });
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
    } finally {
      setImporting(false);
    }
  };

  // ─── 删除 ───
  const handleDelete = async () => {
    if (!deleteTarget) return;
    const name = deleteTarget.name;
    setBusyKey(`del::${name}`);
    try {
      await mcpApi.delete(name);
      setServers((list) => list.filter((s) => s.name !== name));
      setMessage({ kind: "ok", text: t("mcp.deleted", "已删除") });
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
    } finally {
      setDeleteTarget(null);
      setBusyKey(null);
    }
  };

  const toggleSelect = (name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, maxWidth: 920 }}>
      {/* 顶栏 */}
      <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <h1 style={{ fontSize: 22, fontWeight: 700, margin: 0, color: "var(--text-primary)" }}>
          {t("mcp.title", "MCP")}
        </h1>
        <span style={{ color: "var(--text-tertiary)", fontSize: 13 }}>
          {t("mcp.subtitle", { count: servers.length, defaultValue: `${servers.length}` })}
        </span>
        <div style={{ flex: 1 }} />
        <button
          onClick={openScan}
          disabled={busyKey !== null}
          style={btnPrimary}
        >
          {t("mcp.scanImport", "扫描导入")}
        </button>
      </div>

      {/* 消息条 */}
      {message && (
        <div
          style={{
            padding: "8px 12px",
            borderRadius: 8,
            border: `1px solid ${message.kind === "ok" ? "var(--success)" : "var(--danger)"}`,
            background: "var(--bg-elevated)",
            color: message.kind === "ok" ? "var(--success)" : "var(--danger)",
            fontSize: 13,
          }}
        >
          {message.text}
        </div>
      )}

      {/* 列表 */}
      {loading ? (
        <div style={{ color: "var(--text-tertiary)", fontSize: 14 }}>
          {t("common.loading", "加载中…")}
        </div>
      ) : servers.length === 0 ? (
        <div
          style={{
            padding: 32,
            textAlign: "center",
            color: "var(--text-tertiary)",
            fontSize: 14,
            border: "1px dashed var(--border)",
            borderRadius: 12,
          }}
        >
          {t("mcp.empty", "暂无 MCP，点「扫描导入」从 agent 配置拉取")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {servers.map((srv) => (
            <McpRow
              key={srv.name}
              srv={srv}
              busyKey={busyKey}
              onToggle={handleToggle}
              onDelete={() => setDeleteTarget(srv)}
            />
          ))}
        </div>
      )}

      {/* 扫描导入 modal */}
      {scanOpen &&
        createPortal(
          <div style={modalOverlay} onClick={() => !scanning && !importing && setScanOpen(false)}>
            <div style={modalBody} onClick={(e) => e.stopPropagation()}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
                <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
                  {t("mcp.scanTitle", "扫描导入 MCP")}
                </h2>
                <span style={{ color: "var(--text-tertiary)", fontSize: 12 }}>
                  {scanning ? t("mcp.scanning", "扫描中…") : `${scanItems.length}`}
                </span>
                <div style={{ flex: 1 }} />
                <button
                  onClick={() => {
                    const all =
                      selected.size === scanItems.filter((i) => !i.alreadyImported).length;
                    setSelected(
                      all
                        ? new Set()
                        : new Set(
                            scanItems.filter((i) => !i.alreadyImported).map((i) => i.name),
                          ),
                    );
                  }}
                  disabled={scanning || importing}
                  style={btnGhost}
                >
                  {t("mcp.toggleAll", "全选/反选")}
                </button>
              </div>

              <div style={{ maxHeight: "50vh", overflow: "auto", display: "flex", flexDirection: "column", gap: 6 }}>
                {scanning ? (
                  <div style={{ padding: 24, textAlign: "center", color: "var(--text-tertiary)", fontSize: 13 }}>
                    {t("mcp.scanning", "扫描中…")}
                  </div>
                ) : scanItems.length === 0 ? (
                  <div style={{ padding: 24, textAlign: "center", color: "var(--text-tertiary)", fontSize: 13 }}>
                    {t("mcp.scanEmpty", "两 agent 配置无 MCP")}
                  </div>
                ) : (
                  scanItems.map((it) => {
                    const checked = selected.has(it.name);
                    const disabled = it.alreadyImported;
                    return (
                      <label
                        key={it.name}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 10,
                          padding: "8px 10px",
                          borderRadius: 8,
                          border: `1px solid var(--border)`,
                          background: disabled ? "var(--bg-elevated)" : "transparent",
                          opacity: disabled ? 0.5 : 1,
                          cursor: disabled ? "not-allowed" : "pointer",
                          fontSize: 13,
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={checked || disabled}
                          disabled={disabled}
                          onChange={() => toggleSelect(it.name)}
                        />
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                            <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{it.name}</span>
                            <TransportBadge transport={it.transport} />
                            {it.foundInAgents.map((a) => (
                              <span
                                key={a}
                                style={{
                                  fontSize: 10,
                                  padding: "1px 5px",
                                  borderRadius: 4,
                                  background: "var(--bg-elevated)",
                                  color: "var(--text-tertiary)",
                                }}
                              >
                                {t(`mcp.agent.${a}`, a)}
                              </span>
                            ))}
                            {disabled && (
                              <span style={{ fontSize: 10, color: "var(--success)" }}>
                                {t("mcp.alreadyImported", "已导入")}
                              </span>
                            )}
                          </div>
                          <div style={{ color: "var(--text-tertiary)", fontSize: 11, marginTop: 2, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {summaryOf(it)}
                          </div>
                        </div>
                      </label>
                    );
                  })
                )}
              </div>

              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 12 }}>
                <button onClick={() => setScanOpen(false)} disabled={importing} style={btnGhost}>
                  {t("common.cancel", "取消")}
                </button>
                <button
                  onClick={handleImport}
                  disabled={importing || selected.size === 0}
                  style={btnPrimary}
                >
                  {importing
                    ? t("mcp.importing", "导入中…")
                    : t("mcp.import", { count: selected.size, defaultValue: `导入 ${selected.size}` })}
                </button>
              </div>
            </div>
          </div>,
          document.body,
        )}

      {/* 删除确认 modal */}
      {deleteTarget &&
        createPortal(
          <div style={modalOverlay} onClick={() => setDeleteTarget(null)}>
            <div style={modalBody} onClick={(e) => e.stopPropagation()}>
              <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
                {t("mcp.deleteTitle", "删除 MCP")}
              </h2>
              <p style={{ margin: "12px 0", fontSize: 13, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
                {t("mcp.deleteConfirm", {
                  name: deleteTarget.name,
                  defaultValue: `将从 DB + 所有 enabled agent 配置移除「${deleteTarget.name}」，不可恢复`,
                })}
              </p>
              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
                <button onClick={() => setDeleteTarget(null)} style={btnGhost}>
                  {t("common.cancel", "取消")}
                </button>
                <button onClick={handleDelete} style={btnDanger}>
                  {t("common.delete", "删除")}
                </button>
              </div>
            </div>
          </div>,
          document.body,
        )}
    </div>
  );
}

// ─── 单行 ───

function McpRow({
  srv,
  busyKey,
  onToggle,
  onDelete,
}: {
  srv: McpServerInfo;
  busyKey: string | null;
  onToggle: (s: McpServerInfo, a: McpAgentSlug) => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "10px 12px",
        borderRadius: 10,
        border: "1px solid var(--border)",
        background: "var(--bg-elevated)",
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <span style={{ fontWeight: 600, color: "var(--text-primary)", fontSize: 14 }}>{srv.name}</span>
          <TransportBadge transport={srv.transport} />
        </div>
        <div
          style={{
            color: "var(--text-tertiary)",
            fontSize: 12,
            marginTop: 3,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {summaryOf(srv)}
        </div>
        {/* env 脱敏展示 */}
        {Object.keys(srv.env).length > 0 && (
          <div style={{ marginTop: 4, display: "flex", gap: 6, flexWrap: "wrap" }}>
            {Object.entries(srv.env).map(([k, v]) => (
              <span
                key={k}
                style={{
                  fontSize: 10,
                  padding: "1px 5px",
                  borderRadius: 4,
                  background: "var(--bg)",
                  color: "var(--text-tertiary)",
                  fontFamily: "var(--font-mono)",
                }}
              >
                {k}={v}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* per-agent toggle */}
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        {AGENTS.map((agent) => {
          const enabled = srv.enabledAgents.includes(agent);
          const supported = agentSupported(srv.transport, agent);
          const busy = busyKey === `${srv.name}::${agent}`;
          return (
            <button
              key={agent}
              title={
                !supported
                  ? t("mcp.unsupportedTransportTip", {
                      transport: srv.transport,
                      defaultValue: `${srv.transport} 不支持`,
                    })
                  : t(`mcp.agent.${agent}`, agent)
              }
              disabled={!supported || busy}
              onClick={() => onToggle(srv, agent)}
              style={{
                width: 30,
                height: 30,
                borderRadius: 8,
                border: enabled
                  ? "1.5px solid var(--accent)"
                  : "1px solid var(--border)",
                background: enabled ? "var(--accent-subtle)" : "transparent",
                padding: 0,
                cursor: supported && !busy ? "pointer" : "not-allowed",
                opacity: !supported ? 0.3 : enabled ? 1 : 0.55,
                transition: "opacity 0.15s, border-color 0.15s, background 0.15s",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <img
                src={AGENT_ICONS[agent]}
                alt={agent}
                style={{
                  width: 18,
                  height: 18,
                  filter: enabled ? "none" : "grayscale(1)",
                }}
              />
            </button>
          );
        })}

        {/* 删除 */}
        <button
          title={t("common.delete", "删除")}
          onClick={onDelete}
          disabled={busyKey?.startsWith(`del::${srv.name}`)}
          style={{
            width: 30,
            height: 30,
            borderRadius: 8,
            border: "1px solid var(--border)",
            background: "transparent",
            cursor: "pointer",
            color: "var(--danger)",
            padding: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 4h9M5.5 4V2.8a.8.8 0 0 1 .8-.8h2.4a.8.8 0 0 1 .8.8V4M4.2 4l.5 8.2a1 1 0 0 0 1 .8h3.6a1 1 0 0 0 1-.8L11.3 4" />
          </svg>
        </button>
      </div>
    </div>
  );
}

function TransportBadge({ transport }: { transport: string }) {
  const { t } = useTranslation();
  const s = transportStyle(transport);
  return (
    <span
      style={{
        fontSize: 10,
        fontWeight: 600,
        padding: "1px 6px",
        borderRadius: 4,
        background: s.bg,
        color: s.fg,
        textTransform: "uppercase",
        letterSpacing: 0.3,
      }}
    >
      {t(`mcp.transport.${transport}`, transport)}
    </span>
  );
}

// ─── 样式 ───

const btnPrimary: React.CSSProperties = {
  padding: "7px 14px",
  borderRadius: 8,
  border: "1px solid var(--accent)",
  background: "var(--accent)",
  color: "#fff",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};

const btnGhost: React.CSSProperties = {
  padding: "7px 14px",
  borderRadius: 8,
  border: "1px solid var(--border)",
  background: "transparent",
  color: "var(--text-primary)",
  fontSize: 13,
  cursor: "pointer",
};

const btnDanger: React.CSSProperties = {
  padding: "7px 14px",
  borderRadius: 8,
  border: "1px solid var(--danger)",
  background: "var(--danger)",
  color: "#fff",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};

const modalOverlay: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(0,0,0,0.5)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  zIndex: 1000,
};

const modalBody: React.CSSProperties = {
  background: "var(--bg-elevated)",
  border: "1px solid var(--border)",
  borderRadius: 12,
  padding: 20,
  width: "min(560px, 90vw)",
  maxHeight: "80vh",
  display: "flex",
  flexDirection: "column",
  boxShadow: "0 8px 32px rgba(0,0,0,0.3)",
};
