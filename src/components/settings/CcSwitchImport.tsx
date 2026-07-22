// cc-switch 导入子模块 UI（ImportExport 第三块卡片）。
// 检测 cc-switch 配置 → 读 providers（仅 claude + codex）→ 前端匹配回退链
// → 选择性导入（级 1 provider 多选 + 级 2 维度 D1/D2/D4）→ 预览冲突 → 应用。
//
// 复用：
// - 后端 ccswitchApi.detect/read/import（services/api.ts）。
// - 前端 matchCcProvider + ccProviderToPlatformJson（utils/ccswitchMatch.ts）。
// - 冲突 UI（ConflictRow + decisions Map，复用 ImportExport.tsx 既有模式）。
// - 不新增 export scope，走 apply::apply 写入。

import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ccswitchApi,
  platformApi,
  groupDetailApi,
  type CcProvider,
  type CcswitchDetection,
  type ConflictItem,
  type ConflictDecision,
  type ImportDecision,
  type ImportReport,
  type Protocol,
  type GroupDetail,
} from "../../services/api";
import { matchCcProvider, ccProviderToPlatformJson, DEFAULT_DIMS, type CcImportDims, type CcMatchResult } from "../../utils/ccswitchMatch";
import { IconCheck } from "../icons";
import { StatChip } from "../shared/StatChip";
import type { ColorLevel } from "../shared/colorScale";
import { SectionHeader, TextButton } from "./ImportExport/primitives";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

function protocolColor(matchedBy: string): ColorLevel {
  switch (matchedBy) {
    case "preset_keyword":
      return "success";
    case "base_url_host":
      return "success";
    default:
      return "warning";
  }
}

function matchBadgeKey(matchedBy: string, t: TFunction): string {
  switch (matchedBy) {
    case "preset_keyword":
      return t("importExport.ccswitch.matched", "命中");
    case "base_url_host":
      return t("importExport.ccswitch.hostMatch", "host");
    default:
      return t("importExport.ccswitch.fallback", "回退");
  }
}

export function CcSwitchImportSection({
  onReport,
}: {
  onReport: (r: ImportReport) => void;
}) {
  const { t } = useTranslation();
  const [detection, setDetection] = useState<CcswitchDetection | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [providers, setProviders] = useState<CcProvider[]>([]);
  // matchCcProvider async 化后预计算匹配结果（render 内禁止 await），按 provider id 缓存。
  const [matches, setMatches] = useState<Record<string, CcMatchResult>>({});
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const entries = await Promise.all(
        providers.map(async (p) => [p.id, await matchCcProvider(p)] as const),
      );
      if (!cancelled) setMatches(Object.fromEntries(entries));
    })();
    return () => { cancelled = true; };
  }, [providers]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [dims, setDims] = useState<CcImportDims>({ ...DEFAULT_DIMS });
  const [reading, setReading] = useState(false);
  const [conflicts, setConflicts] = useState<ConflictItem[]>([]);
  const [decisions, setDecisions] = useState<Map<string, ImportDecision>>(new Map());
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState("");
  // 批量分组归属（作用于本次导入的全部平台）：默认建默认分组 = 现行行为。
  const [batchAutoGroup, setBatchAutoGroup] = useState(true);
  const [batchJoinGroupIds, setBatchJoinGroupIds] = useState<number[]>([]);
  const [groupDetails, setGroupDetails] = useState<GroupDetail[]>([]);

  const handleDetect = async (overridePath?: string) => {
    setError("");
    setDetecting(true);
    try {
      const d = await ccswitchApi.detect(overridePath);
      setDetection(d);
      if (d.found) {
        await handleRead(d.path ?? undefined);
      } else {
        // 清空。
        setProviders([]);
        setSelected(new Set());
        setConflicts([]);
        setDecisions(new Map());
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setDetecting(false);
    }
  };

  const handleRead = async (path?: string) => {
    setError("");
    setReading(true);
    try {
      const r = await ccswitchApi.read(path);
      setProviders(r.providers);
      // 默认全选。
      setSelected(new Set(r.providers.map((p) => p.id)));
      setConflicts([]);
      setDecisions(new Map());
      // 拉分组列表供「加入已有分组」multi-select（失败不阻断）。
      groupDetailApi.list().then(setGroupDetails).catch(() => {});
    } catch (e) {
      setError(String(e));
    } finally {
      setReading(false);
    }
  };

  const handlePickDir = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (picked && typeof picked === "string") {
      await handleDetect(picked);
    }
  };

  const toggleProvider = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const decisionKey = (scope: string, key: string) => `${scope}::${key}`;

  const setDecision = (c: ConflictItem, d: ImportDecision) => {
    setDecisions((prev) => {
      const next = new Map(prev);
      next.set(decisionKey(c.scope, c.key), d);
      return next;
    });
  };

  // platform.name 非唯一（无 UNIQUE 约束）→ 导入始终建新行，无"覆盖现有"冲突。
  // conflicts/decisions 保留空态：handleImport 中 skip/rename 决策仍可由用户主动设置，
  // 但默认全空 = 全部建新（重复导入同 provider = 列表多个同名 platform）。
  const handlePreview = () => {
    setError("");
    setConflicts([]);
    setDecisions(new Map());
  };

  const handleImport = async () => {
    setError("");
    setImporting(true);
    try {
      // 构造 Platform JSON payload（仅选中的）。
      const payload: Record<string, unknown>[] = [];
      for (const p of providers) {
        if (!selected.has(p.id)) continue;
        const match = await matchCcProvider(p);
        const json = ccProviderToPlatformJson(p, match, dims);
        // 应用 rename 决策（若用户选了 rename）。
        const dk = decisionKey("platform", p.name);
        const dec = decisions.get(dk);
        if (dec?.kind === "rename") {
          json.name = dec.new_key;
        } else if (dec?.kind === "skip") {
          continue;
        }
        payload.push(json);
      }
      if (payload.length === 0) {
        setError(t("importExport.ccswitch.nothingSelected", "没有可导入的项（全部跳过或未选中）"));
        return;
      }
      const ds: ConflictDecision[] = Array.from(decisions.entries()).map(([k, d]) => {
        const [scope, key] = k.split("::");
        return { scope, key, decision: d };
      });
      // autoGroup 开 → 后端 ensure-by-name 建/加入固定 `cc-switch` 分组（toggle 默认开）。
      const r = await ccswitchApi.import(payload, ds, batchAutoGroup);
      // 批量分组归属回挂：按最终名匹配建出的平台，补建默认分组（ensureAutoGroup，幂等）
      // + 全量同步加入的已有分组（platform_update）。失败不阻断导入报告（平台已建好）。
      try {
        const finalNames = payload.map(j => String(j.name));
        const all = await platformApi.list();
        const imported = all.filter(p => finalNames.includes(p.name));
        await Promise.all(imported.map(async p => {
          if (batchAutoGroup) await platformApi.ensureAutoGroup(p.id);
          await platformApi.update({ id: p.id, join_group_ids: batchJoinGroupIds });
        }));
      } catch (e) {
        console.error("cc-switch group assign failed:", e);
      }
      onReport(r);
      setConflicts([]);
      setDecisions(new Map());
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const selectedCount = selected.size;
  const conflictCount = conflicts.length;

  return (
    <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}>
      <SectionHeader
        icon="download"
        title={t("importExport.ccswitch.title", "从 cc-switch 导入")}
        desc={t(
          "importExport.ccswitch.desc",
          "读取本地 cc-switch 配置（仅 claude + codex provider），按 base_url 自动识别平台类型。选择性导入 + 冲突逐项决策。",
        )}
      />

      {/* 检测 + 手动选目录 */}
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
        <Button variant="default"
          onClick={() => handleDetect()}
          disabled={detecting || reading}
          
          style={{ padding: "7px 16px", fontSize: 13 }}
        >
          {detecting
            ? t("importExport.ccswitch.detecting", "检测中…")
            : t("importExport.ccswitch.detectBtn", "检测 cc-switch")}
        </Button>
        <Button variant="outline"
          onClick={handlePickDir}
          disabled={detecting || reading}
          style={{
            padding: "7px 14px", fontSize: 12, cursor: "pointer",
            borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
            background: "transparent", color: "var(--text-secondary)",
          }}
        >
          {t("importExport.ccswitch.selectDir", "手动选择目录")}
        </Button>
        {detection && detection.found && (
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            <StatChip
              value={String(detection.providerCount)}
              label={t("importExport.ccswitch.providerCount", "个")}
              level="success"
            />
            <code style={{ fontSize: 11, color: "var(--text-tertiary)", wordBreak: "break-all" }}>
              {detection.path}
            </code>
          </div>
        )}
        {detection && !detection.found && (
          <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
            {t("importExport.ccswitch.notDetected", "未检测到，可手动选择目录")}
          </span>
        )}
      </div>

      {/* provider 列表 + 维度勾选 */}
      {providers.length > 0 && (
        <>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
            <strong style={{ fontSize: 14, color: "var(--text-primary)" }}>
              {t("importExport.ccswitch.providerList", "供应商列表")}
            </strong>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <TextButton
                onClick={() => setSelected(new Set(providers.map((p) => p.id)))}
              >
                {t("importExport.selectAll", "全选")}
              </TextButton>
              <TextButton onClick={() => setSelected(new Set())}>
                {t("importExport.deselectAll", "反选")}
              </TextButton>
              <StatChip value={`${selectedCount}/${providers.length}`} label={t("importExport.selectedLabel", "已选")} level={selectedCount > 0 ? "success" : "neutral"} />
            </div>
          </div>

          {/* 维度勾选条（级 2）*/}
          <div style={{ display: "flex", alignItems: "center", gap: 16, padding: "8px 12px", borderRadius: "var(--radius-md)", background: "var(--bg-subtle)", flexWrap: "wrap" }}>
            <DimCheckbox
              checked={dims.d1}
              onChange={(v) => setDims((d) => ({ ...d, d1: v }))}
              label={t("importExport.ccswitch.dimPlatformType", "平台类型")}
              hint={t("importExport.ccswitch.dimPlatformTypeHint", "含 endpoints")}
              disabled
            />
            <DimCheckbox
              checked={dims.d2}
              onChange={(v) => setDims((d) => ({ ...d, d2: v }))}
              label={t("importExport.ccswitch.dimModels", "模型映射")}
              hint={t("importExport.ccswitch.dimModelsHint", "ANTHROPIC_MODEL / codex model")}
            />
            <DimCheckbox
              checked={dims.d4}
              onChange={(v) => setDims((d) => ({ ...d, d4: v }))}
              label={t("importExport.ccswitch.dimApiKey", "密钥")}
              hint={t("importExport.ccswitch.dimApiKeyHint", "api_key")}
            />
          </div>

          {/* provider 行 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {providers.map((p) => {
              const match = matches[p.id];
              const isSelected = selected.has(p.id);
              // match 还在异步计算时跳过行（短暂空态，defaults.json 已缓存极快）
              if (!match) return null;
              return (
                <ProviderRow
                  key={p.id}
                  provider={p}
                  matchProtocol={match.protocol}
                  matchLabel={match.matchedLabel}
                  matchedBy={match.matchedBy}
                  conflict={false}
                  selected={isSelected}
                  onToggle={() => toggleProvider(p.id)}
                  noKey={!p.detectedApiKey}
                  t={t}
                />
              );
            })}
          </div>

          {/* 批量分组归属（作用于本次导入的全部平台） */}
          <div style={{ display: "flex", flexDirection: "column", gap: 8, padding: "10px 12px", borderRadius: "var(--radius-md)", border: "1px solid var(--border)" }}>
            <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("importExport.ccswitch.groupAssignHint", "导入后这些平台加入哪些分组（批量，作用于全部已导入平台）")}
            </div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
              <span style={{ fontSize: 13 }}>{t("importExport.ccswitch.autoGroup", "导入后自动加入「cc-switch」分组")}</span>
              <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
                <Input type="checkbox" checked={batchAutoGroup} onChange={e => setBatchAutoGroup(e.target.checked)} style={{ display: "none" }} />
                <span className={`toggle ${batchAutoGroup ? "active" : ""}`} />
              </label>
            </div>
            {groupDetails.length > 0 && (
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {groupDetails.map(gd => {
                  const checked = batchJoinGroupIds.includes(gd.group.id);
                  return (
                    <Button variant="outline"
                      key={gd.group.id}
                      type="button"
                      onClick={() => setBatchJoinGroupIds(prev => checked
                        ? prev.filter(id => id !== gd.group.id)
                        : [...prev, gd.group.id])}
                      style={{
                        display: "inline-flex", alignItems: "center",
                        padding: "4px 12px", borderRadius: 999, fontSize: 12, fontWeight: 500,
                        cursor: "pointer",
                        border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                        background: checked ? "var(--accent-subtle)" : "var(--bg-glass)",
                        color: checked ? "var(--accent)" : "var(--text-secondary)",
                        transition: "all 200ms cubic-bezier(0.4, 0, 0.2, 1)",
                      }}
                    >
                      {gd.group.name}
                    </Button>
                  );
                })}
              </div>
            )}
          </div>

          {/* 预览 + 导入按钮 */}
          <div style={{ display: "flex", alignItems: "center", gap: 10, justifyContent: "flex-end", flexWrap: "wrap" }}>
            <Button variant="outline"
              onClick={handlePreview}
              disabled={selectedCount === 0}
              style={{
                padding: "7px 14px", fontSize: 13, cursor: "pointer",
                borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
                background: "transparent", color: "var(--text-primary)",
                opacity: selectedCount === 0 ? 0.5 : 1,
              }}
            >
              {t("importExport.ccswitch.preview", "预览冲突")}
            </Button>
            <Button variant="default"
              onClick={handleImport}
              disabled={importing || selectedCount === 0}
              
              style={{ padding: "7px 16px", fontSize: 13 }}
            >
              {importing
                ? t("importExport.applying", "导入中…")
                : t("importExport.ccswitch.importBtn", "导入 {{n}} 项", { n: selectedCount })}
            </Button>
          </div>
        </>
      )}

      {/* 冲突列表 */}
      {conflictCount > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <strong style={{ fontSize: 14, color: "var(--color-warning)" }}>
            {t("importExport.conflicts", "冲突（{{n}} 项）", { n: conflictCount })}
          </strong>
          {conflicts.map((c) => {
            const dk = decisionKey(c.scope, c.key);
            const cur = decisions.get(dk) || { kind: "overwrite" };
            return (
              <ConflictRowSimple
                key={dk}
                item={c}
                current={cur}
                onChange={(d) => setDecision(c, d)}
                t={t}
              />
            );
          })}
        </div>
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

// ─── 子组件 ─────────────────────────────────────────────────
// SectionHeader / TextButton 已合并到 ./ImportExport/primitives.tsx（消 D-new.1/2）。

function DimCheckbox({
  checked,
  onChange,
  label,
  hint,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  hint?: string;
  disabled?: boolean;
}) {
  return (
    <label style={{ display: "flex", alignItems: "center", gap: 6, cursor: disabled ? "default" : "pointer", opacity: disabled ? 0.7 : 1 }}>
      <Input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        style={{ display: "none" }}
      />
      <span
        style={{
          width: 16, height: 16, borderRadius: 4,
          border: `1px solid ${checked ? "var(--accent)" : "var(--border-default)"}`,
          background: checked ? "var(--accent)" : "transparent",
          display: "inline-flex", alignItems: "center", justifyContent: "center",
        }}
      >
        {checked && <IconCheck size={11} color="#fff" strokeWidth={2.5} />}
      </span>
      <span style={{ fontSize: 13, color: "var(--text-primary)", fontWeight: 500 }}>{label}</span>
      {hint && <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{hint}</span>}
    </label>
  );
}

function ProviderRow({
  provider,
  matchProtocol,
  matchLabel,
  matchedBy,
  conflict,
  selected,
  onToggle,
  noKey,
  t,
}: {
  provider: CcProvider;
  matchProtocol: Protocol;
  matchLabel?: string;
  matchedBy: string;
  conflict: boolean;
  selected: boolean;
  onToggle: () => void;
  noKey: boolean;
  t: TFunction;
}) {
  const badgeLevel = protocolColor(matchedBy);
  return (
    <div
      className="glass-surface"
      role="button"
      tabIndex={0}
      onClick={onToggle}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onToggle();
        }
      }}
      style={{
        padding: 12, borderRadius: "var(--radius-md)", cursor: "pointer",
        border: `1px solid ${selected ? "var(--accent)" : "var(--border)"}`,
        background: selected ? "var(--accent-subtle)" : "transparent",
        display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap",
      }}
    >
      <span
        style={{
          width: 18, height: 18, borderRadius: "50%",
          display: "inline-flex", alignItems: "center", justifyContent: "center",
          border: `1px solid ${selected ? "var(--accent)" : "var(--border)"}`,
          background: selected ? "var(--accent)" : "transparent",
          flexShrink: 0,
        }}
      >
        {selected && <IconCheck size={12} color="#fff" strokeWidth={2.5} />}
      </span>
      <span style={{ fontSize: 11, color: "var(--text-tertiary)", textTransform: "uppercase" }}>
        {provider.appType}
      </span>
      <span style={{ fontWeight: 600, color: "var(--text-primary)", fontSize: 13 }}>{provider.name}</span>
      <StatChip value={matchLabel ?? matchProtocol} label={matchBadgeKey(matchedBy, t)} level={badgeLevel} />
      {provider.detectedBaseUrl && (
        <code style={{ fontSize: 11, color: "var(--text-tertiary)", wordBreak: "break-all" }}>
          {provider.detectedBaseUrl}
        </code>
      )}
      {conflict && (
        <StatChip value={t("importExport.ccswitch.conflict", "冲突")} label="" level="danger" />
      )}
      {noKey && (
        <StatChip value={t("importExport.ccswitch.noKey", "无密钥")} label="" level="warning" />
      )}
    </div>
  );
}

function ConflictRowSimple({
  item,
  current,
  onChange,
  t,
}: {
  item: ConflictItem;
  current: ImportDecision;
  onChange: (d: ImportDecision) => void;
  t: TFunction;
}) {
  const isRename = current.kind === "rename";
  return (
    <div
      className="glass-surface"
      style={{
        padding: 12, borderRadius: "var(--radius-md)",
        border: "1px solid var(--border)",
        display: "flex", flexDirection: "column", gap: 8,
      }}
    >
      <span style={{ fontWeight: 600, color: "var(--text-primary)", fontSize: 13, wordBreak: "break-all" }}>
        {item.key}
      </span>
      <div style={{ fontSize: 12, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{item.existing_summary}</div>
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        {(["overwrite", "skip", "rename"] as const).map((kind, i) => {
          const active = current.kind === kind;
          const labelKey = kind === "overwrite" ? "importExport.overwrite" : kind === "skip" ? "importExport.skip" : "importExport.rename";
          const defaultLabel = kind === "overwrite" ? "覆盖" : kind === "skip" ? "跳过" : "重命名";
          return (
            <Button variant="outline"
              key={kind}
              onClick={() => {
                if (kind === "rename") onChange({ kind: "rename", new_key: item.key + "-imported" });
                else onChange({ kind });
              }}
              style={{
                padding: "5px 12px", fontSize: 12, fontWeight: active ? 600 : 500, cursor: "pointer",
                border: "none",
                borderLeft: i > 0 ? "1px solid var(--border)" : "none",
                background: active ? "var(--accent-subtle)" : "transparent",
                color: active ? "var(--accent)" : "var(--text-secondary)",
              }}
            >
              {t(labelKey, defaultLabel)}
            </Button>
          );
        })}
        {isRename && (
          <Input
            
            type="text"
            value={(current as { kind: "rename"; new_key: string }).new_key}
            onChange={(e) => onChange({ kind: "rename", new_key: e.target.value })}
            style={{ width: 220 }}
          />
        )}
      </div>
    </div>
  );
}
