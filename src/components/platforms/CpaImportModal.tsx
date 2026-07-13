// CpaImportModal — CPA(CLIProxyAPI) 配置导入模态框。
// 仿 CcSwitchImport 模式（检测→读取→预览→多选→冲突→apply），createPortal(document.body)。
//
// 三段式（design.md）：
// 1. 选源：Tauri open dialog（文件/压缩包/文件夹）+ 可选第二 dialog 选 auth-dir 凭据目录。
// 2. 预览：cpa_import_parse → 表格（多选/改名/选模型/冲突检测/惰性余额，并发 ≤5）。
// 3. apply：cpa_import_apply（非原子尽力）→ toast created/failed + 关 modal。
//
// 复用：Modal.tsx 基元、cpaImportApi（services/api/platforms.ts）、getDefaultModelList（async preset 兜底）。

import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { Modal } from "../shared/Modal";
import { StatChip } from "../shared/StatChip";
import { IconClose } from "../icons";
import {
  cpaImportApi,
  platformApi,
  type MappedPlatform,
  type Platform,
  type PlatformQuota,
} from "../../services/api";
import { getProtocolColorMap, getProtocolLabelMap, getDefaultModelList } from "../../domains/platforms/defaults";
import { formatCostUsd } from "../../utils/formatters";

/** 行可编辑字段（前端态：用户可改 name / models）。apply 时与原条目合并回 MappedPlatform。 */
interface RowState {
  /** 行稳定 key（base_url + name + index，防重名条目撞 key） */
  rowId: string;
  name: string;
  /** 用户填的模型槽（自由文本，逗号分隔）。apply 时拆分为 models[]。 */
  modelsText: string;
  selected: boolean;
  /** 惰性余额：undefined 未查 / null 不支持或失败 / number 余额美元。 */
  quota: number | null | undefined;
  querying: boolean;
  /** 冲突标记：与已存平台同名或同 base_url。 */
  conflict: boolean;
  conflictReason: string;
}

/** 脱敏 api_key：保留前 4 + 后 4，中间打码。 */
function maskKey(key?: string): string {
  if (!key) return "";
  if (key.length <= 10) return "••••";
  return `${key.slice(0, 4)}••••${key.slice(-4)}`;
}

/** 拆分用户输入模型文本为标准化 models[]。 */
function parseModelsText(text: string): string[] {
  return text
    .split(/[,，\n]/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

export interface CpaImportModalProps {
  open: boolean;
  onClose: () => void;
  /** apply 成功后回调（刷新平台列表 + toast）。 */
  onApplied: (created: Platform[], failed: { name: string; error: string }[]) => void;
}

export function CpaImportModal({ open: isOpen, onClose, onApplied }: CpaImportModalProps) {
  const { t, i18n } = useTranslation();
  const [sourcePath, setSourcePath] = useState<string>("");
  const [authDir, setAuthDir] = useState<string>("");
  const [parsing, setParsing] = useState(false);
  const [error, setError] = useState("");
  const [skipped, setSkipped] = useState<{ path: string; reason: string }[]>([]);
  const [sourceFiles, setSourceFiles] = useState<string[]>([]);

  // 后端解析的原始条目（按 rowId 索引，保 api_key/base_url/protocol 等不可改字段）。
  const [originals, setOriginals] = useState<Record<string, MappedPlatform>>({});
  const [order, setOrder] = useState<string[]>([]);
  const [rows, setRows] = useState<Record<string, RowState>>({});
  // 已存平台（冲突检测比对源）。
  const [existing, setExisting] = useState<Platform[]>([]);

  const [applying, setApplying] = useState(false);

  // 协议色 + label map（async 派生自 presets）。
  const [colorMap, setColorMap] = useState<Partial<Record<string, string>>>({});
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  useEffect(() => {
    let cancelled = false;
    Promise.all([getProtocolColorMap(), getProtocolLabelMap(i18n.language)]).then(([c, l]) => {
      if (!cancelled) { setColorMap(c); setLabelMap(l); }
    });
    return () => { cancelled = true; };
  }, [i18n.language]);

  // 打开 modal 时拉一次已存平台列表（冲突检测）。
  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    platformApi.list().then(list => { if (!cancelled) setExisting(list); }).catch(() => {});
    return () => { cancelled = true; };
  }, [isOpen]);

  // ponytail: 关闭时清状态，防下次再开残留旧数据。
  useEffect(() => {
    if (isOpen) return;
    setSourcePath(""); setAuthDir(""); setError("");
    setSkipped([]); setSourceFiles([]);
    setOriginals({}); setOrder([]); setRows({});
  }, [isOpen]);

  const handlePickSource = async () => {
    setError("");
    const picked = await open({
      multiple: false,
      // fileDialog 是否同时支持目录依平台；首版仅文件（用户选压缩包 / yaml / json / 文件夹）。
      // ponytail: Tauri open 在 macOS 上 directory:true 可选目录，false 可选文件；一次 dialog 二选一体验差，留两个按钮。
    });
    if (picked && typeof picked === "string") setSourcePath(picked);
  };

  const handlePickDir = async (setter: (v: string) => void) => {
    const picked = await open({ directory: true, multiple: false });
    if (picked && typeof picked === "string") setter(picked);
  };

  const handleParse = async () => {
    if (!sourcePath) {
      setError(t("platform.cpaImport.errNoSource", "请先选择配置源"));
      return;
    }
    setError(""); setParsing(true);
    try {
      const r = await cpaImportApi.parse(sourcePath, authDir || undefined);
      setSkipped(r.skipped);
      setSourceFiles(r.source_files);
      // 异步预填默认模型槽（getDefaultModels async）。每条目拉一次协议的默认模型槽位，
      // 合并源 models[] + 默认槽，去重保序。
      const plats = r.platforms;
      const existingNames = new Set(existing.map(p => p.name));
      const existingBaseUrls = new Set(existing.flatMap(p =>
        (p.endpoints ?? []).map(ep => ep.base_url).concat(p.base_url || []).filter(Boolean),
      ));
      // ponytail: getDefaultModelList 从 docPromise 缓存取，并发 N 次 O(1) RPC。
      const enriched = await Promise.all(plats.map(async (p, idx): Promise<[MappedPlatform, RowState]> => {
        const rowId = `${idx}::${p.name}::${p.base_url}`;
        let modelsList = p.models;
        if (modelsList.length === 0) {
          // 源无 models → 拉 preset model_list 兜底（MUST 不留空，contract 第 14 条）。
          modelsList = await getDefaultModelList(p.protocol);
        }
        const conflictReason = existingNames.has(p.name)
          ? t("platform.cpaImport.conflictName", "同名平台已存")
          : (p.base_url && existingBaseUrls.has(p.base_url)
            ? t("platform.cpaImport.conflictBaseUrl", "同 base_url 平台已存")
            : "");
        return [p, {
          rowId,
          name: p.name,
          modelsText: modelsList.join(", "),
          selected: !conflictReason,
          quota: undefined,
          querying: false,
          conflict: !!conflictReason,
          conflictReason,
        }];
      }));
      const nextOriginals: Record<string, MappedPlatform> = {};
      const nextRows: Record<string, RowState> = {};
      const nextOrder: string[] = [];
      for (const [p, row] of enriched) {
        nextOriginals[row.rowId] = p;
        nextRows[row.rowId] = row;
        nextOrder.push(row.rowId);
      }
      setOriginals(nextOriginals);
      setRows(nextRows);
      setOrder(nextOrder);
    } catch (e) {
      setError(String(e));
    } finally {
      setParsing(false);
    }
  };

  // ── 余额查询（惰性，并发 ≤5）──
  const quotaQueueRef = useRef<MappedPlatform[] | null>(null);
  const quotaRunningRef = useRef(0);

  const queryOneQuota = async (rowId: string) => {
    const orig = originals[rowId];
    const row = rows[rowId];
    if (!orig || !row || row.querying) return;
    if (!orig.api_key || !orig.base_url) {
      // OAuth 类平台 base_url 可能为空，无法查余额 → 显「—」。
      setRows(prev => ({ ...prev, [rowId]: { ...prev[rowId], quota: null, querying: false } }));
      return;
    }
    setRows(prev => ({ ...prev, [rowId]: { ...prev[rowId], querying: true } }));
    try {
      const q: PlatformQuota = await cpaImportApi.previewQuota(orig.base_url, orig.api_key);
      const remain = q.success && q.balance ? q.balance.remaining : null;
      setRows(prev => ({ ...prev, [rowId]: { ...prev[rowId], quota: remain, querying: false } }));
    } catch {
      setRows(prev => ({ ...prev, [rowId]: { ...prev[rowId], quota: null, querying: false } }));
    }
  };

  /** 并发 ≤5 跑队列。ponytail: 简单 semaphore，每完一个抢下一个；不引第三方 lib。 */
  const drainQuotaQueue = async () => {
    const queue = quotaQueueRef.current;
    if (!queue) return;
    while (queue.length > 0 && quotaRunningRef.current < 5) {
      const next = queue.shift();
      if (!next) break;
      const rowId = order.find(id => originals[id] === next);
      if (!rowId) continue;
      quotaRunningRef.current += 1;
      queryOneQuota(rowId).finally(() => {
        quotaRunningRef.current -= 1;
        drainQuotaQueue();
      });
    }
  };

  const handleQueryAllQuota = async () => {
    const selectedPlatforms = order
      .map(id => originals[id])
      .filter((p): p is MappedPlatform => !!p);
    if (selectedPlatforms.length === 0) return;
    quotaQueueRef.current = selectedPlatforms;
    quotaRunningRef.current = 0;
    await drainQuotaQueue();
  };

  // ── 选择 ──
  const toggleRow = (rowId: string) => {
    setRows(prev => ({
      ...prev,
      [rowId]: { ...prev[rowId], selected: !prev[rowId].selected },
    }));
  };
  const selectAll = () => {
    setRows(prev => {
      const next = { ...prev };
      for (const id of order) next[id] = { ...next[id], selected: !next[id].conflict };
      return next;
    });
  };
  const deselectAll = () => {
    setRows(prev => {
      const next = { ...prev };
      for (const id of order) next[id] = { ...next[id], selected: false };
      return next;
    });
  };
  const updateRowName = (rowId: string, name: string) => {
    setRows(prev => ({ ...prev, [rowId]: { ...prev[rowId], name } }));
  };
  const updateRowModels = (rowId: string, modelsText: string) => {
    setRows(prev => ({ ...prev, [rowId]: { ...prev[rowId], modelsText } }));
  };

  const selectedCount = useMemo(
    () => order.filter(id => rows[id]?.selected).length,
    [order, rows],
  );

  // ── apply ──
  const handleApply = async () => {
    setError(""); setApplying(true);
    try {
      const payload: MappedPlatform[] = [];
      for (const id of order) {
        const row = rows[id];
        const orig = originals[id];
        if (!row || !orig || !row.selected) continue;
        payload.push({
          ...orig,
          name: row.name.trim() || orig.name,
          models: parseModelsText(row.modelsText),
        });
      }
      if (payload.length === 0) {
        setError(t("platform.cpaImport.errNoSelect", "未选中任何条目"));
        return;
      }
      const report = await cpaImportApi.apply(payload);
      onApplied(report.created, report.failed);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setApplying(false);
    }
  };

  const hasParsed = order.length > 0;

  return (
    <Modal open={isOpen} onClose={onClose} maxWidth={920} maxHeight="88vh">
      <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10 }}>
          <div style={{ fontSize: 16, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("platform.cpaImport.title", "导入 CPA 配置")}
          </div>
          <button className="btn btn-ghost" style={{ padding: "4px 8px" }} onClick={onClose}>
            <IconClose size={14} />
          </button>
        </div>

        {/* 步骤 1：选源 */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8, padding: 12, borderRadius: "var(--radius-md)", border: "1px solid var(--border)", background: "var(--bg-glass)" }}>
          <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {t("platform.cpaImport.step1Hint", "选择 CPA 配置源：单文件 (yaml/json) / 压缩包 (zip/tgz/tar) / 文件夹。rar/7z 请先解压。")}
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
            <button className="btn" onClick={handlePickSource} disabled={parsing || applying}>
              {t("platform.cpaImport.pickSource", "选择源")}
            </button>
            {sourcePath && (
              <code style={{ fontSize: 11, color: "var(--text-tertiary)", wordBreak: "break-all", flex: 1, minWidth: 200 }}>
                {sourcePath}
              </code>
            )}
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
            <button
              className="btn btn-ghost"
              onClick={() => handlePickDir(setAuthDir)}
              disabled={parsing || applying}
              style={{ fontSize: 12 }}
            >
              {t("platform.cpaImport.pickAuthDir", "可选：OAuth 凭据目录")}
            </button>
            {authDir && (
              <code style={{ fontSize: 11, color: "var(--text-tertiary)", wordBreak: "break-all" }}>
                {authDir}
              </code>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              className="btn btn-primary"
              onClick={handleParse}
              disabled={!sourcePath || parsing || applying}
            >
              {parsing ? t("status.loading", "解析中…") : t("platform.cpaImport.parse", "解析")}
            </button>
          </div>
        </div>

        {error && (
          <div style={{ padding: "8px 12px", fontSize: 12, borderRadius: "var(--radius-md)",
            color: "var(--color-danger)", background: "var(--color-danger-bg)",
            border: "1px solid var(--color-danger)" }}>
            {error}
          </div>
        )}

        {/* 步骤 2：预览表格 */}
        {hasParsed && (
          <>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10, flexWrap: "wrap" }}>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <button className="btn btn-ghost" onClick={handleQueryAllQuota} style={{ fontSize: 12 }}>
                  {t("platform.cpaImport.queryAllQuota", "全部查询余额")}
                </button>
                <button className="btn btn-ghost" onClick={selectAll} style={{ fontSize: 12 }}>
                  {t("importExport.selectAll", "全选")}
                </button>
                <button className="btn btn-ghost" onClick={deselectAll} style={{ fontSize: 12 }}>
                  {t("importExport.deselectAll", "取消")}
                </button>
                <StatChip value={`${selectedCount}/${order.length}`} label={t("importExport.selectedLabel", "已选")} level={selectedCount > 0 ? "success" : "neutral"} />
              </div>
              <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                {t("platform.cpaImport.sourceFiles", "源文件 {{n}}", { n: sourceFiles.length })}
                {skipped.length > 0 && ` · ${t("platform.cpaImport.skipped", "跳过 {{n}}", { n: skipped.length })}`}
              </span>
            </div>

            <div style={{ overflowX: "auto", border: "1px solid var(--border)", borderRadius: "var(--radius-md)" }}>
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
                <thead>
                  <tr style={{ background: "var(--bg-subtle)", textAlign: "left" }}>
                    <th style={{ padding: "8px 6px", width: 32 }}></th>
                    <th style={{ padding: "8px 6px" }}>{t("platform.name", "名称")}</th>
                    <th style={{ padding: "8px 6px" }}>{t("platform.protocol", "协议")}</th>
                    <th style={{ padding: "8px 6px" }}>base_url</th>
                    <th style={{ padding: "8px 6px" }}>api_key</th>
                    <th style={{ padding: "8px 6px", minWidth: 180 }}>{t("platform.models", "模型")}</th>
                    <th style={{ padding: "8px 6px" }}>{t("platform.balance", "余额")}</th>
                    <th style={{ padding: "8px 6px" }}>{t("platform.cpaImport.conflict", "冲突")}</th>
                  </tr>
                </thead>
                <tbody>
                  {order.map(rowId => {
                    const row = rows[rowId];
                    const orig = originals[rowId];
                    if (!row || !orig) return null;
                    const color = colorMap[orig.protocol] || "var(--accent)";
                    return (
                      <tr key={rowId} style={{ borderTop: "1px solid var(--border)" }}>
                        <td style={{ padding: "6px", textAlign: "center" }}>
                          <input
                            type="checkbox"
                            checked={row.selected}
                            onChange={() => toggleRow(rowId)}
                            disabled={row.conflict}
                            style={{ cursor: row.conflict ? "not-allowed" : "pointer" }}
                          />
                        </td>
                        <td style={{ padding: "6px" }}>
                          <input
                            className="input"
                            value={row.name}
                            onChange={(e) => updateRowName(rowId, e.target.value)}
                            style={{ fontSize: 12, padding: "4px 6px", minWidth: 120, width: "100%" }}
                          />
                        </td>
                        <td style={{ padding: "6px" }}>
                          <span style={{
                            display: "inline-block", padding: "2px 8px", borderRadius: "var(--radius-sm)",
                            background: `${color}20`, color, fontSize: 11, fontWeight: 700,
                          }}>
                            {labelMap[orig.protocol] || orig.protocol}
                          </span>
                          <div style={{ fontSize: 10, color: "var(--text-tertiary)", marginTop: 2 }}>
                            {orig.source_label}
                          </div>
                        </td>
                        <td style={{ padding: "6px" }}>
                          <code style={{ fontSize: 11, color: "var(--text-tertiary)", wordBreak: "break-all" }}>
                            {orig.base_url || t("platform.cpaImport.empty", "—")}
                          </code>
                        </td>
                        <td style={{ padding: "6px" }}>
                          <code style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                            {maskKey(orig.api_key)}
                          </code>
                        </td>
                        <td style={{ padding: "6px" }}>
                          <input
                            className="input"
                            value={row.modelsText}
                            onChange={(e) => updateRowModels(rowId, e.target.value)}
                            placeholder={t("platform.cpaImport.modelsPlaceholder", "逗号分隔")}
                            style={{ fontSize: 11, padding: "4px 6px", minWidth: 180, width: "100%" }}
                          />
                        </td>
                        <td style={{ padding: "6px" }}>
                          {row.querying ? (
                            <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                              {t("status.loading", "查询中…")}
                            </span>
                          ) : row.quota === undefined ? (
                            <button
                              className="btn btn-ghost"
                              onClick={() => queryOneQuota(rowId)}
                              style={{ fontSize: 11, padding: "2px 8px" }}
                            >
                              {t("platform.cpaImport.query", "查")}
                            </button>
                          ) : row.quota === null ? (
                            <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>—</span>
                          ) : (
                            <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                              {formatCostUsd(row.quota)}
                            </span>
                          )}
                        </td>
                        <td style={{ padding: "6px" }}>
                          {row.conflict && (
                            <StatChip
                              value={t("platform.cpaImport.conflictShort", "冲突")}
                              label={row.conflictReason}
                              level="warning"
                            />
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>

            {/* 步骤 3：apply */}
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
              <button
                className="btn btn-primary"
                onClick={handleApply}
                disabled={applying || selectedCount === 0}
              >
                {applying
                  ? t("platform.cpaImport.applying", "创建中…")
                  : t("platform.cpaImport.apply", "确认创建 {{n}} 项", { n: selectedCount })}
              </button>
            </div>
          </>
        )}

        {skipped.length > 0 && (
          <details style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
            <summary>{t("platform.cpaImport.skippedList", "跳过的文件 ({{n}})", { n: skipped.length })}</summary>
            <ul style={{ margin: "6px 0 0 18px", padding: 0 }}>
              {skipped.map((s, i) => (
                <li key={i} style={{ wordBreak: "break-all" }}>
                  <code>{s.path}</code> — {s.reason}
                </li>
              ))}
            </ul>
          </details>
        )}
      </div>
    </Modal>
  );
}
