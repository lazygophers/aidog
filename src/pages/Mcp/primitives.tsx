import { useTranslation } from "react-i18next";
import { type McpServerInfo, type McpAgentSlug } from "../../services/api";
import {
  AGENTS,
  AGENT_ICONS,
  agentSupported,
  transportStyle,
  summaryOf,
} from "./constants";
import { inputStyle, btnGhost } from "./styles";

// ─── 单行 ───

export function McpRow({
  srv,
  busyKey,
  onToggle,
  onEdit,
  onDelete,
  onShare,
}: {
  srv: McpServerInfo;
  busyKey: string | null;
  onToggle: (s: McpServerInfo, a: McpAgentSlug) => void;
  onEdit: () => void;
  onDelete: () => void;
  onShare: () => void;
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

        {/* 分享 */}
        <button
          title={t("mcp.share", "分享")}
          onClick={onShare}
          style={{
            width: 30,
            height: 30,
            borderRadius: 8,
            border: "1px solid var(--border)",
            background: "transparent",
            cursor: "pointer",
            color: "var(--text-secondary)",
            padding: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="11" cy="4" r="2" />
            <circle cx="4" cy="7.5" r="2" />
            <circle cx="11" cy="11" r="2" />
            <path d="M6 6.5l3-1.5M6 8.5l3 1.5" />
          </svg>
        </button>

        {/* 编辑 */}
        <button
          title={t("mcp.edit", "编辑")}
          onClick={onEdit}
          disabled={busyKey?.startsWith(`edit::${srv.name}`)}
          style={{
            width: 30,
            height: 30,
            borderRadius: 8,
            border: "1px solid var(--border)",
            background: "transparent",
            cursor: "pointer",
            color: "var(--text-secondary)",
            padding: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
            <path d="M9.5 2.5l3 3L5 13H2v-3z" />
            <path d="M8 4l3 3" />
          </svg>
        </button>

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

export function TransportBadge({ transport }: { transport: string }) {
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

// ─── 动态键值行（env / headers 编辑）───

export function KVRows({
  label,
  rows,
  onChange,
}: {
  label: string;
  rows: { k: string; v: string }[];
  onChange: (rows: { k: string; v: string }[]) => void;
}) {
  const { t } = useTranslation();
  const update = (i: number, patch: Partial<{ k: string; v: string }>) =>
    onChange(rows.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
        {label}
        <span style={{ color: "var(--text-tertiary)", marginLeft: 6, fontSize: 11 }}>
          ({t("mcp.maskedHint", "未改值填 *** 保持原密钥")})
        </span>
      </span>
      {rows.map((r, i) => (
        <div key={i} style={{ display: "flex", gap: 6 }}>
          <input
            style={{ ...inputStyle, flex: 1 }}
            value={r.k}
            placeholder="KEY"
            onChange={(e) => update(i, { k: e.target.value })}
          />
          <input
            style={{ ...inputStyle, flex: 1.4, fontFamily: "var(--font-mono, monospace)" }}
            value={r.v}
            placeholder="***"
            onChange={(e) => update(i, { v: e.target.value })}
          />
          <button
            onClick={() => onChange(rows.filter((_, idx) => idx !== i))}
            style={{ ...btnGhost, padding: "6px 10px" }}
            title={t("common.delete", "删除")}
          >
            ×
          </button>
        </div>
      ))}
      <button
        onClick={() => onChange([...rows, { k: "", v: "" }])}
        style={{ ...btnGhost, alignSelf: "flex-start", padding: "5px 10px", fontSize: 12 }}
      >
        + {t("mcp.addRow", "添加")}
      </button>
    </div>
  );
}
