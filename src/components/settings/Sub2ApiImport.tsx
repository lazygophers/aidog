// sub2api 导入子模块 UI（ImportExport 卡片）。
// 双入口（选 JSON 文件 / 粘贴文本）→ 后端 sub2api_parse 解析 → 预览每账号
// （name / Protocol 下拉可改 / base_url / api_key 脱敏 + 未识别徽标）→ 勾选
// + 「加入分组」toggle（默认开）→ sub2apiAccountToPlatformJson → sub2api_import。
//
// 与 cc-switch 路径差异：无本地探测（用户提供 JSON），平台匹配为直映射（更简单）。
// 复用：sub2apiApi.parse/import（services/api.ts）+ mapPlatformToProtocol /
// sub2apiAccountToPlatformJson（utils/sub2apiMatch.ts）+ apply::apply 写入。

import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import {
  sub2apiApi,
  type Sub2ApiAccount,
  type ImportReport,
  type Protocol,
} from "../../services/api";
import { buildProtocolsFromPresets, getProtocolLabelMap } from "../../domains/platforms/defaults";
import type { ProtocolOption } from "../../domains/platforms";
import { mapPlatformToProtocol, sub2apiAccountToPlatformJson } from "../../utils/sub2apiMatch";
import { SectionIcon } from "./editors";
import { IconCheck } from "../icons";
import { StatChip } from "../shared/StatChip";

/** 脱敏 api_key：保留前 4 + 后 4，中间打码。 */
function maskKey(key?: string): string {
  if (!key) return "";
  if (key.length <= 10) return "••••";
  return `${key.slice(0, 4)}••••${key.slice(-4)}`;
}

export function Sub2ApiImportSection({
  onReport,
}: {
  onReport: (r: ImportReport) => void;
}) {
  const { t, i18n } = useTranslation();
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  const [protocols, setProtocols] = useState<ProtocolOption[]>([]);
  useEffect(() => {
    let cancelled = false;
    Promise.all([getProtocolLabelMap(i18n.language), buildProtocolsFromPresets(i18n.language)]).then(([m, list]) => {
      if (!cancelled) { setLabelMap(m); setProtocols(list); }
    });
    return () => { cancelled = true; };
  }, [i18n.language]);
  const [pasteText, setPasteText] = useState("");
  const [accounts, setAccounts] = useState<Sub2ApiAccount[]>([]);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  // 每账号的 protocol 覆盖（下拉手改）；缺省走 platform 映射。
  const [overrides, setOverrides] = useState<Map<number, Protocol>>(new Map());
  const [autoGroup, setAutoGroup] = useState(true);
  const [parsing, setParsing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState("");

  const applyParsed = (r: { accounts: Sub2ApiAccount[] }) => {
    setAccounts(r.accounts);
    setSelected(new Set(r.accounts.map((_, i) => i)));
    setOverrides(new Map());
  };

  const handleParseText = async (text: string) => {
    setError("");
    if (!text.trim()) {
      setError(t("importExport.sub2api.emptyInput", "请先粘贴或选择 sub2api 导出 JSON"));
      return;
    }
    setParsing(true);
    try {
      const r = await sub2apiApi.parse(text);
      applyParsed(r);
    } catch (e) {
      setError(String(e));
      setAccounts([]);
      setSelected(new Set());
    } finally {
      setParsing(false);
    }
  };

  const handlePickFile = async () => {
    setError("");
    try {
      const picked = await open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (picked && typeof picked === "string") {
        const text = await sub2apiApi.readFile(picked);
        setPasteText(text);
        await handleParseText(text);
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleAccount = (i: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(i)) next.delete(i);
      else next.add(i);
      return next;
    });
  };

  const setOverride = (i: number, protocol: Protocol) => {
    setOverrides((prev) => {
      const next = new Map(prev);
      next.set(i, protocol);
      return next;
    });
  };

  const handleImport = async () => {
    setError("");
    setImporting(true);
    try {
      const payload: Record<string, unknown>[] = [];
      for (let i = 0; i < accounts.length; i++) {
        const acc = accounts[i];
        if (!selected.has(i)) continue;
        const protocol = overrides.get(i) ?? mapPlatformToProtocol(acc.platform).protocol;
        payload.push(await sub2apiAccountToPlatformJson(acc, protocol));
      }
      if (payload.length === 0) {
        setError(t("importExport.sub2api.nothingSelected", "没有可导入的账号（未选中）"));
        return;
      }
      // decisions 空数组：platform 不参与冲突检测（always INSERT，无覆盖语义）。
      const r = await sub2apiApi.import(payload, [], autoGroup);
      onReport(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const selectedCount = selected.size;

  return (
    <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <SectionIcon name="download" size={18} style={{ color: "var(--accent)" }} />
          <h3 style={{ margin: 0, fontSize: 18, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("importExport.sub2api.title", "从 sub2api 导入")}
          </h3>
        </div>
        <p style={{ margin: 0, fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>
          {t(
            "importExport.sub2api.desc",
            "导入 sub2api 管理后台导出的账号数据 JSON（仅取 platform / base_url / api_key）。导入将新建平台，不覆盖同名。",
          )}
        </p>
      </div>

      {/* 双入口：选文件 + 粘贴 */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
          <button
            onClick={handlePickFile}
            disabled={parsing || importing}
            className="btn btn-primary"
            style={{ padding: "7px 16px", fontSize: 13 }}
          >
            {t("importExport.sub2api.pickFile", "选择 JSON 文件")}
          </button>
          <button
            onClick={() => handleParseText(pasteText)}
            disabled={parsing || importing}
            style={{
              padding: "7px 14px", fontSize: 12, cursor: "pointer",
              borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
              background: "transparent", color: "var(--text-primary)",
            }}
          >
            {parsing
              ? t("importExport.sub2api.parsing", "解析中…")
              : t("importExport.sub2api.parsePaste", "解析粘贴内容")}
          </button>
        </div>
        <textarea
          className="input"
          value={pasteText}
          onChange={(e) => setPasteText(e.target.value)}
          placeholder={t("importExport.sub2api.pastePlaceholder", "粘贴 sub2api 导出的 JSON 文本…")}
          style={{ minHeight: 80, fontFamily: "monospace", fontSize: 12, resize: "vertical" }}
        />
      </div>

      {/* 账号预览列表 */}
      {accounts.length > 0 && (
        <>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
            <strong style={{ fontSize: 14, color: "var(--text-primary)" }}>
              {t("importExport.sub2api.accountList", "账号列表")}
            </strong>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <button
                onClick={() => setSelected(new Set(accounts.map((_, i) => i)))}
                style={{ background: "transparent", border: "none", color: "var(--accent)", fontSize: 13, fontWeight: 500, cursor: "pointer", padding: 0 }}
              >
                {t("importExport.selectAll", "全选")}
              </button>
              <button
                onClick={() => setSelected(new Set())}
                style={{ background: "transparent", border: "none", color: "var(--accent)", fontSize: 13, fontWeight: 500, cursor: "pointer", padding: 0 }}
              >
                {t("importExport.deselectAll", "反选")}
              </button>
              <StatChip value={`${selectedCount}/${accounts.length}`} label={t("importExport.selectedLabel", "已选")} level={selectedCount > 0 ? "success" : "neutral"} />
            </div>
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {accounts.map((acc, i) => {
              const map = mapPlatformToProtocol(acc.platform);
              const protocol = overrides.get(i) ?? map.protocol;
              const isSelected = selected.has(i);
              return (
                <div
                  key={`${acc.name}-${i}`}
                  className="glass-surface"
                  style={{
                    padding: 12, borderRadius: "var(--radius-md)",
                    border: `1px solid ${isSelected ? "var(--accent)" : "var(--border)"}`,
                    background: isSelected ? "var(--accent-subtle)" : "transparent",
                    display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap",
                  }}
                >
                  <span
                    role="button"
                    tabIndex={0}
                    onClick={() => toggleAccount(i)}
                    onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleAccount(i); } }}
                    style={{
                      width: 18, height: 18, borderRadius: "50%", cursor: "pointer",
                      display: "inline-flex", alignItems: "center", justifyContent: "center",
                      border: `1px solid ${isSelected ? "var(--accent)" : "var(--border)"}`,
                      background: isSelected ? "var(--accent)" : "transparent", flexShrink: 0,
                    }}
                  >
                    {isSelected && <IconCheck size={12} color="#fff" strokeWidth={2.5} />}
                  </span>
                  <span style={{ fontWeight: 600, color: "var(--text-primary)", fontSize: 13 }}>{acc.name}</span>
                  {/* Protocol 下拉手改 */}
                  <select
                    className="input"
                    value={protocol}
                    onChange={(e) => setOverride(i, e.target.value as Protocol)}
                    style={{ fontSize: 12, padding: "4px 8px", width: "auto", minWidth: 140 }}
                  >
                    {protocols.filter((p) => !p.codingPlan).map((p) => (
                      <option key={`${p.value}-${p.label}`} value={p.value}>{labelMap[p.value] || p.label}</option>
                    ))}
                  </select>
                  {!map.recognized && !overrides.has(i) && (
                    <StatChip value={t("importExport.sub2api.unrecognized", "未识别·已兜底 OpenAI")} label="" level="warning" />
                  )}
                  {acc.baseUrl && (
                    <code style={{ fontSize: 11, color: "var(--text-tertiary)", wordBreak: "break-all" }}>{acc.baseUrl}</code>
                  )}
                  {acc.apiKey ? (
                    <code style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{maskKey(acc.apiKey)}</code>
                  ) : (
                    <StatChip value={t("importExport.sub2api.noKey", "无密钥")} label="" level="warning" />
                  )}
                </div>
              );
            })}
          </div>

          {/* 加入分组 toggle */}
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, padding: "10px 12px", borderRadius: "var(--radius-md)", border: "1px solid var(--border)" }}>
            <span style={{ fontSize: 13 }}>{t("importExport.sub2api.autoGroup", "导入后自动加入「sub2api」分组")}</span>
            <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
              <input type="checkbox" checked={autoGroup} onChange={(e) => setAutoGroup(e.target.checked)} style={{ display: "none" }} />
              <span className={`toggle ${autoGroup ? "active" : ""}`} />
            </label>
          </div>

          {/* 导入按钮 */}
          <div style={{ display: "flex", alignItems: "center", gap: 10, justifyContent: "flex-end", flexWrap: "wrap" }}>
            <button
              onClick={handleImport}
              disabled={importing || selectedCount === 0}
              className="btn btn-primary"
              style={{ padding: "7px 16px", fontSize: 13 }}
            >
              {importing
                ? t("importExport.applying", "导入中…")
                : t("importExport.sub2api.importBtn", "导入 {{n}} 项", { n: selectedCount })}
            </button>
          </div>
        </>
      )}

      {error && (
        <div
          style={{
            padding: "8px 12px", fontSize: 12, borderRadius: "var(--radius-md)",
            color: "var(--color-danger)", background: "var(--color-danger-bg)",
            border: "1px solid var(--color-danger)",
          }}
        >
          {error}
        </div>
      )}
    </section>
  );
}
