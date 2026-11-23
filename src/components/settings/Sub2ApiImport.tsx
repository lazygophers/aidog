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
import { pickPath } from "../../services/pathPicker";
import {
  sub2apiApi,
  type Sub2ApiAccount,
  type ImportReport,
  type Protocol,
} from "../../services/api";
import { buildProtocolsFromPresets } from "../../domains/platforms/defaults";
import { useProtocolLabelMap } from "../../domains/platforms/useProtocolMeta";
import type { ProtocolOption } from "../../domains/platforms";
import { mapPlatformToProtocol, sub2apiAccountToPlatformJson } from "../../utils/sub2apiMatch";
import { SectionIcon } from "./editors";
import { IconCheck } from "../icons";
import { StatChip } from "../shared/StatChip";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useReveal, makeRipple, JsonCodeEditor } from "../../components/shared";

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
  // ponytail: 外层 section 单 useReveal, 账号行内部 glass-surface 各持 hover 边光。
  const sectionReveal = useReveal<HTMLElement>(0);
  const labelMap = useProtocolLabelMap(i18n.language);
  const [protocols, setProtocols] = useState<ProtocolOption[]>([]);
  useEffect(() => {
    let cancelled = false;
    buildProtocolsFromPresets(i18n.language).then(list => { if (!cancelled) setProtocols(list); });
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
      setError(t("importExport.sub2api.emptyInput", "Please paste or select a sub2api export JSON first"));
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
      const picked = await pickPath({
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (picked) {
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
        setError(t("importExport.sub2api.nothingSelected", "No accounts to import (none selected)"));
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
    <section
      ref={sectionReveal.ref}
      className={`glass glass-highlight hover-lift reveal${sectionReveal.shown ? " in" : ""}`}
      style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <SectionIcon name="download" size={18} style={{ color: "var(--accent)" }} />
          <h3 style={{ margin: 0, fontSize: 18, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("importExport.sub2api.title", "Import from sub2api")}
          </h3>
        </div>
        <p style={{ margin: 0, fontSize: 14, color: "var(--text-secondary)", lineHeight: 1.5 }}>
          {t(
            "importExport.sub2api.desc",
            "Import the account-data JSON exported from the sub2api admin panel (only platform / base_url / api_key). Import always creates new platforms; it never overwrites by name.",
          )}
        </p>
      </div>

      {/* 双入口：选文件 + 粘贴 */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
          <Button variant="default"
            onClick={handlePickFile}
            disabled={parsing || importing}
            
            style={{ padding: "7px 16px", fontSize: 14 }}
          >
            {t("importExport.sub2api.pickFile", "Select JSON file")}
          </Button>
          <Button variant="outline"
            onClick={() => handleParseText(pasteText)}
            disabled={parsing || importing}
            style={{
              padding: "7px 14px", fontSize: 14, cursor: "pointer",
              borderRadius: "var(--radius-md)", border: "1px solid var(--border)",
              background: "transparent", color: "var(--text-primary)",
            }}
          >
            {parsing
              ? t("importExport.sub2api.parsing", "Parsing…")
              : t("importExport.sub2api.parsePaste", "Parse pasted text")}
          </Button>
        </div>
        <JsonCodeEditor
          value={pasteText}
          onChange={setPasteText}
          minHeight={120}
          maxHeight={320}
          placeholder={t("importExport.sub2api.pastePlaceholder", "Paste the JSON text exported from sub2api…")}
        />
      </div>

      {/* 账号预览列表 */}
      {accounts.length > 0 && (
        <>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
            <strong style={{ fontSize: 14, color: "var(--text-primary)" }}>
              {t("importExport.sub2api.accountList", "Account list")}
            </strong>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <Button variant="outline"
                onClick={() => setSelected(new Set(accounts.map((_, i) => i)))}
                style={{ background: "transparent", border: "none", color: "var(--accent)", fontSize: 14, fontWeight: 500, cursor: "pointer", padding: 0 }}
              >
                {t("importExport.selectAll", "Select all")}
              </Button>
              <Button variant="outline"
                onClick={() => setSelected(new Set())}
                style={{ background: "transparent", border: "none", color: "var(--accent)", fontSize: 14, fontWeight: 500, cursor: "pointer", padding: 0 }}
              >
                {t("importExport.deselectAll", "Deselect all")}
              </Button>
              <StatChip value={`${selectedCount}/${accounts.length}`} label={t("importExport.selectedLabel", "selected")} level={selectedCount > 0 ? "success" : "neutral"} />
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
                    border: `1px solid ${isSelected ? "var(--accent-edge)" : "var(--border)"}`,
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
                      border: `1px solid ${isSelected ? "var(--accent-edge)" : "var(--border)"}`,
                      background: isSelected ? "var(--primary)" : "transparent", flexShrink: 0,
                    }}
                  >
                    {isSelected && <IconCheck size={12} color="var(--primary-foreground)" strokeWidth={2.5} />}
                  </span>
                  <span style={{ fontWeight: 600, color: "var(--text-primary)", fontSize: 14 }}>{acc.name}</span>
                  {/* Protocol 下拉手改 */}
                  <Select
                    
                    value={protocol}
                    onValueChange={(v) => setOverride(i, v as Protocol)}
                    
                  >
<SelectTrigger style={{ fontSize: 14, padding: "4px 8px", width: "auto", minWidth: 140 }}><SelectValue/></SelectTrigger>
<SelectContent>
                    {protocols.filter((p) => !p.codingPlan).map((p) => (
                      <SelectItem key={`${p.value}-${p.label}`} value={p.value}>{labelMap[p.value] || p.label}</SelectItem>
                    ))}
                  </SelectContent>
</Select>
                  {!map.recognized && !overrides.has(i) && (
                    <StatChip value={t("importExport.sub2api.unrecognized", "Unrecognized · fell back to OpenAI")} label="" level="warning" />
                  )}
                  {acc.baseUrl && (
                    <code style={{ fontSize: 14, color: "var(--text-tertiary)", wordBreak: "break-all" }}>{acc.baseUrl}</code>
                  )}
                  {acc.apiKey ? (
                    <code style={{ fontSize: 14, color: "var(--text-tertiary)" }}>{maskKey(acc.apiKey)}</code>
                  ) : (
                    <StatChip value={t("importExport.sub2api.noKey", "No key")} label="" level="warning" />
                  )}
                </div>
              );
            })}
          </div>

          {/* 加入分组 toggle */}
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, padding: "10px 12px", borderRadius: "var(--radius-md)", border: "1px solid var(--border)" }}>
            <span style={{ fontSize: 14 }}>{t("importExport.sub2api.autoGroup", "Auto-join the \"sub2api\" group after import")}</span>
            <Switch checked={autoGroup} onCheckedChange={setAutoGroup} />
          </div>

          {/* 导入按钮 */}
          <div style={{ display: "flex", alignItems: "center", gap: 10, justifyContent: "flex-end", flexWrap: "wrap" }}>
            <Button variant="default"
              className="ripple"
              onClick={(e) => { makeRipple(e); handleImport(); }}
              disabled={importing || selectedCount === 0}

              style={{ padding: "7px 16px", fontSize: 14 }}
            >
              {importing
                ? t("importExport.applying", "Importing…")
                : t("importExport.sub2api.importBtn", "Import {{n}} items", { n: selectedCount })}
            </Button>
          </div>
        </>
      )}

      {error && (
        <div
          style={{
            padding: "8px 12px", fontSize: 14, borderRadius: "var(--radius-md)",
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
