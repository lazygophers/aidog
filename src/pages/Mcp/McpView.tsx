import type { McpData } from "./useMcpData";
import { McpRow } from "./primitives";
import { Button } from "@/components/ui/button";
import { makeRipple } from "@/utils/motion";
import { PiUnsupportedNote } from "@/components/shared";

/**
 * 主列表视图（自原 Mcp.tsx L446-536 外迁）。
 * 顶栏（标题 + 4 个操作按钮）/ 消息条 / 列表（含空态与 loading 态）。
 */
export function McpView({ d }: { d: McpData }) {
  const {
    t, servers, loading, busyKey, message,
    openAdd, openScan, handleResync, setPasteOpen, setPasteText, setMessage,
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
        <Button
          variant="outline"
          className="ripple"
          onClick={(e) => { makeRipple(e); openAdd(); }}
          disabled={busyKey !== null}
        >
          {t("mcp.add", "添加 MCP")}
        </Button>
        <Button
          variant="outline"
          className="ripple"
          onClick={(e) => { makeRipple(e); setPasteText(""); setMessage(null); setPasteOpen(true); }}
          disabled={busyKey !== null}
        >
          {t("mcp.pasteImport", "粘贴导入")}
        </Button>
        <Button
          variant="outline"
          className="ripple"
          onClick={(e) => { makeRipple(e); void handleResync(); }}
          disabled={busyKey !== null}
          title={t("mcp.resyncHint", "从 aidog 数据库重写所有已启用 agent 的配置文件，修复被外部工具污染的条目")}
        >
          {t("mcp.resync", "重新同步")}
        </Button>
        <Button
          className="ripple"
          onClick={(e) => { makeRipple(e); void openScan(); }}
          disabled={busyKey !== null}
        >
          {t("mcp.scanImport", "扫描导入")}
        </Button>
      </div>

      <PiUnsupportedNote
        reasonKey="pi.unsupportedMcp"
        reasonFallback="pi 刻意不内置 MCP，能力靠 extension 直接写 TypeScript 提供，没有可写入的 MCP 配置文件。"
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
          {servers.map((srv, idx) => (
            <McpRow
              key={srv.name}
              srv={srv}
              idx={idx}
              busyKey={busyKey}
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
