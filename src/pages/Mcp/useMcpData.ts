import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  mcpApi,
  type McpServerInfo,
  type McpScanItem,
  type McpAgentSlug,
  type McpImportPayload,
  type McpUpdatePayload,
  type McpTransport,
} from "../../services/api";
import { agentSupported } from "./constants";

/**
 * MCP 页全部 state + data actions（自原 Mcp.tsx L59-444 外迁）。
 * 14 useState + 数据加载/扫描/导入/编辑/删除/分享/deep-link，无逻辑变更。
 */
export function useMcpData() {
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

  // 粘贴 JSON 导入 modal
  const [pasteOpen, setPasteOpen] = useState(false);
  const [pasteText, setPasteText] = useState("");
  const [pasteBusy, setPasteBusy] = useState(false);

  // 删除确认 modal
  const [deleteTarget, setDeleteTarget] = useState<McpServerInfo | null>(null);

  // 编辑 modal
  const [editTarget, setEditTarget] = useState<McpServerInfo | null>(null);
  const [editOpen, setEditOpen] = useState(false);

  // 分享 modal（泛化 ShareModal，复用平台三格式切换）
  const [shareData, setShareData] = useState<{ share: Record<string, unknown>; name: string } | null>(null);
  const [editForm, setEditForm] = useState<{
    name: string;
    transport: McpTransport;
    command: string;
    argsText: string;
    envRows: { k: string; v: string }[];
    url: string;
    headersRows: { k: string; v: string }[];
  }>({
    name: "",
    transport: "stdio",
    command: "",
    argsText: "",
    envRows: [],
    url: "",
    headersRows: [],
  });

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

  // ─── 粘贴 JSON 导入（含 base64 分享文本识别）───
  const handlePasteImport = async () => {
    if (!pasteText.trim()) return;
    setPasteBusy(true);
    setMessage(null);
    try {
      // base64 分享文本（YAML/JSON 经 toBase64Utf8 编码）→ 解码后交给 mcp_import_json。
      // mcp_import_json 走 serde_json 解析；YAML 是 JSON 超集但非严格 JSON，故仅尝试 JSON.parse 兜底。
      let json = pasteText.trim();
      if (/^[A-Za-z0-9+/=\s]+$/.test(json) && json.length > 16) {
        try {
          const decoded = atob(json.replace(/\s/g, ""));
          // 验证解码后是合法 JSON；否则保持原文本走默认路径。
          JSON.parse(decoded);
          json = decoded;
        } catch {
          // 非 base64 或解码后非 JSON，保持原文本。
        }
      }
      const report = await mcpApi.importJson(json);
      await refresh();
      setPasteOpen(false);
      setPasteText("");
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
      setPasteBusy(false);
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

  const handleResync = async () => {
    if (busyKey !== null) return;
    setBusyKey("resync");
    setMessage(null);
    try {
      const n = await mcpApi.resync();
      setMessage({ kind: "ok", text: t("mcp.resyncDone", "已重新同步 {{n}} 项", { n }) });
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
    } finally {
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

  // ─── 分享（导出可分享配置 → 弹泛化 ShareModal）───
  const handleShare = async (srv: McpServerInfo) => {
    setMessage(null);
    try {
      const share = await mcpApi.shareExport(srv.name);
      setShareData({ share, name: srv.name });
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
    }
  };

  // ─── aidog://mcp/import?data=<base64> deep-link 导入 ───
  // 两路汇入：① mount 取 App.tsx 缓存（冷启动/他页唤起 → setActiveNav 挂载本页后到达）；
  // ② 运行时 window 'aidog:mcp' 事件（本页已 mount 热路径）。
  // data = toBase64Utf8(JSON) → atob → JSON → mcp_import_json（复用粘贴路径）。
  const openDeepLinkImport = useCallback(async (data: string) => {
    if (!data) return;
    setMessage(null);
    try {
      const json = atob(data.replace(/\s/g, ""));
      const report = await mcpApi.importJson(json);
      await refresh();
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
    }
  }, [refresh, t]);
  useEffect(() => {
    const w = window as unknown as { __aidogDeepLink?: Record<string, { action: string; data: string }> };
    const cached = w.__aidogDeepLink?.mcp;
    if (cached?.data) {
      delete w.__aidogDeepLink!.mcp; // 消费一次防重复
      void openDeepLinkImport(cached.data);
    }
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ action: string; data: string }>).detail;
      if (detail?.data) {
        delete w.__aidogDeepLink!.mcp;
        void openDeepLinkImport(detail.data);
      }
    };
    window.addEventListener("aidog:mcp", handler);
    return () => window.removeEventListener("aidog:mcp", handler);
  }, [openDeepLinkImport]);

  // ─── 编辑 ───
  const openEdit = (srv: McpServerInfo) => {
    setEditTarget(srv);
    setEditForm({
      name: srv.name,
      transport: srv.transport,
      command: srv.command,
      argsText: srv.args.join("\n"),
      envRows: Object.entries(srv.env).map(([k, v]) => ({ k, v })),
      url: srv.url,
      headersRows: Object.entries(srv.headers).map(([k, v]) => ({ k, v })),
    });
    setMessage(null);
    setEditOpen(true);
  };

  // ─── 添加（空表单）───
  const openAdd = () => {
    setEditTarget(null);
    setEditForm({
      name: "",
      transport: "stdio",
      command: "",
      argsText: "",
      envRows: [],
      url: "",
      headersRows: [],
    });
    setMessage(null);
    setEditOpen(true);
  };

  const handleEditSave = async () => {
    if (!editOpen) return;
    const f = editForm;
    if (!f.name.trim()) {
      setMessage({ kind: "err", text: t("mcp.nameRequired", "name 必填") });
      return;
    }
    const isAdd = editTarget === null;
    setBusyKey(isAdd ? "add::" : `edit::${editTarget.name}`);
    setMessage(null);
    const payload: McpUpdatePayload = {
      name: f.name.trim(),
      transport: f.transport,
      command: f.command,
      args: f.argsText
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
      env: Object.fromEntries(
        f.envRows.filter((r) => r.k.trim()).map((r) => [r.k.trim(), r.v]),
      ),
      url: f.url,
      headers: Object.fromEntries(
        f.headersRows.filter((r) => r.k.trim()).map((r) => [r.k.trim(), r.v]),
      ),
    };
    try {
      if (isAdd) {
        await mcpApi.add(payload);
      } else {
        await mcpApi.update(editTarget!.name, payload);
      }
      await refresh();
      setEditTarget(null);
      setEditOpen(false);
      setMessage({ kind: "ok", text: t("mcp.saved", "已保存") });
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  return {
    t,
    // list state
    servers, loading, busyKey, message,
    // scan modal
    scanOpen, setScanOpen, scanItems, scanning, selected, setSelected, importing, toggleSelect,
    openScan, handleImport,
    // paste modal
    pasteOpen, setPasteOpen, pasteText, setPasteText, pasteBusy, handlePasteImport,
    // delete modal
    deleteTarget, setDeleteTarget, handleDelete,
    // edit modal
    editTarget, editOpen, setEditTarget, setEditOpen, editForm, setEditForm, openEdit, openAdd, handleEditSave,
    // share modal
    shareData, setShareData, handleShare,
    // actions
    handleToggle, handleResync, setMessage,
  };
}

export type McpData = ReturnType<typeof useMcpData>;
