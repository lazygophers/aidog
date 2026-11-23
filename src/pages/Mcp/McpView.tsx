import type { McpData } from "./useMcpData";
import { McpRow } from "./primitives";
import { Button } from "@/components/ui/button";
import { makeRipple } from "@/utils/motion";
import { PiUnsupportedNote } from "@/components/shared";
import { usePluginOrigins } from "@/components/PluginOrigin";

/**
 * 主列表视图（自原 Mcp.tsx L446-536 外迁）。
 * 顶栏（标题 + 4 个操作按钮）/ 消息条 / 列表（含空态与 loading 态）。
 */
export function McpView({ d }: { d: McpData }) {
  const {
    t, servers, loading, busyKey, message,
    openAdd, openScan, handleResync, setPasteOpen, setPasteText, setMessage,
  } = d;
  const origins = usePluginOrigins("servers");

  return (
    <>
      {/* 顶栏 */}
      <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <h1 style={{ fontSize: 22, fontWeight: 700, margin: 0, color: "var(--text-primary)" }}>
          {t("mcp.title", "MCP")}
        </h1>
        <span style={{ color: "var(--text-tertiary)", fontSize: 14 }}>
          {t("mcp.subtitle", { count: servers.length, defaultValue: `${servers.length}` })}
        </span>
        <div style={{ flex: 1 }} />
        <Button
          variant="outline"
          className="ripple"
          onClick={(e) => { makeRipple(e); openAdd(); }}
          disabled={busyKey !== null}
        >
          {t("mcp.add", "Add MCP")}
        </Button>
        <Button
          variant="outline"
          className="ripple"
          onClick={(e) => { makeRipple(e); setPasteText(""); setMessage(null); setPasteOpen(true); }}
          disabled={busyKey !== null}
        >
          {t("mcp.pasteImport", "Paste JSON")}
        </Button>
        <Button
          variant="outline"
          className="ripple"
          onClick={(e) => { makeRipple(e); void handleResync(); }}
          disabled={busyKey !== null}
          title={t("mcp.resyncHint", "Rewrite all enabled agent config files from the aidog database, fixing entries polluted by external tools")}
        >
          {t("mcp.resync", "Resync")}
        </Button>
        <Button
          className="ripple"
          onClick={(e) => { makeRipple(e); void openScan(); }}
          disabled={busyKey !== null}
        >
          {t("mcp.scanImport", "Scan & Import")}
        </Button>
      </div>

      <PiUnsupportedNote
        reasonKey="pi.unsupportedMcp"
        reasonFallback="pi deliberately ships no built-in MCP; capabilities come from TypeScript extensions, so there is no MCP config file to write."
      />

      {/* 消息条 */}
      {message && (
        <div
          style={{
            padding: "8px 12px",
            borderRadius: 8,
            border: `1px solid ${message.kind === "ok" ? "var(--success)" : "var(--danger)"}`,
            background: "var(--bg-elevated)",
            color: message.kind === "ok" ? "var(--success)" : "var(--danger)",
            fontSize: 14,
          }}
        >
          {message.text}
        </div>
      )}

      {/* 列表 */}
      {loading ? (
        <div style={{ color: "var(--text-tertiary)", fontSize: 14 }}>
          {t("common.loading", "Loading...")}
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
          {t("mcp.empty", "No MCP yet. Click \"Scan & Import\" to pull from agent configs.")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {servers.map((srv, idx) => (
            <McpRow
              key={srv.name}
              srv={srv}
              idx={idx}
              busyKey={busyKey}
              origin={origins[srv.name]}
              onToggle={d.handleToggle}
              onEdit={() => d.openEdit(srv)}
              onDelete={() => d.setDeleteTarget(srv)}
              onShare={() => void d.handleShare(srv)}
            />
          ))}
        </div>
      )}
    </>
  );
}
