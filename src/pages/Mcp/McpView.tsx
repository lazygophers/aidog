import type { McpData } from "./useMcpData";
import { McpRow } from "./primitives";
import { btnGhost, btnPrimary } from "./styles";

/**
 * 主列表视图（自原 Mcp.tsx L446-536 外迁）。
 * 顶栏（标题 + 4 个操作按钮）/ 消息条 / 列表（含空态与 loading 态）。
 */
export function McpView({ d }: { d: McpData }) {
  const {
    t, servers, loading, busyKey, message,
    openAdd, openScan, handleResync, setPasteOpen, setPasteText, setMessage,
    handleToggle, openEdit, setDeleteTarget, handleShare,
  } = d;

  return (
    <>
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
          onClick={openAdd}
          disabled={busyKey !== null}
          style={btnGhost}
        >
          {t("mcp.add", "添加 MCP")}
        </button>
        <button
          onClick={() => { setPasteText(""); setMessage(null); setPasteOpen(true); }}
          disabled={busyKey !== null}
          style={btnGhost}
        >
          {t("mcp.pasteImport", "粘贴导入")}
        </button>
        <button
          onClick={handleResync}
          disabled={busyKey !== null}
          style={btnGhost}
          title={t("mcp.resyncHint", "从 aidog 数据库重写所有已启用 agent 的配置文件，修复被外部工具污染的条目")}
        >
          {t("mcp.resync", "重新同步")}
        </button>
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
              onEdit={() => openEdit(srv)}
              onDelete={() => setDeleteTarget(srv)}
              onShare={() => void handleShare(srv)}
            />
          ))}
        </div>
      )}
    </>
  );
}
