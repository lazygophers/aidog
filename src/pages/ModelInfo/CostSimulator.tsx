// 费用模拟器（计费预计算票 12）：ModelInfoTab 第三 tab。
// 四维 token 手填 + 滑杆实时重算；价格一律由 simulator_est_cost command 算（spec A3-A6），
// 前端只做 JSON 解析回填与展示；明细表小计由条目 price_data × tokens × multiplier 推导（纯展示）。

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  costSimulatorApi,
  platformApi,
  type ModelEntry,
  type ModelEntryGroup,
  type SimulatorEstCostDto,
} from "../../services/api";
import type { Platform } from "../../services/api/types/generated/Platform";
import { F } from "../../domains/shared/tokens";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatCostUsd } from "../../utils/formatters";
import { parsePriceData, type PriceTier } from "./priceData";

const DIMS = ["input", "output", "cache_read", "cache_write"] as const;
type Dim = (typeof DIMS)[number];

// 滑杆量程（票 07 原型定档）
const RANGE: Record<Dim, { max: number; step: number }> = {
  input: { max: 200_000, step: 500 },
  output: { max: 64_000, step: 500 },
  cache_read: { max: 500_000, step: 1000 },
  cache_write: { max: 200_000, step: 1000 },
};

interface Candidate {
  entry: ModelEntry;
  /** 用户已配置的同类型平台；null = 未配置（不出对比卡）。 */
  platform: Platform | null;
}

export function CostSimulator({ groups, labelMap }: {
  groups: ModelEntryGroup[];
  labelMap: Record<string, string>;
}) {
  const { t } = useTranslation();
  const [canonical, setCanonical] = useState(groups[0]?.canonical_model ?? "");
  const [tokens, setTokens] = useState<Record<Dim, number>>({
    input: 12_000, output: 4_000, cache_read: 50_000, cache_write: 0,
  });
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [results, setResults] = useState<Record<string, SimulatorEstCostDto>>({});
  const [paste, setPaste] = useState("");
  const [hint, setHint] = useState("");

  const group = useMemo(
    () => groups.find(g => g.canonical_model === canonical) ?? groups[0] ?? null,
    [groups, canonical],
  );

  useEffect(() => {
    platformApi.list().then(setPlatforms).catch(e => console.error(e));
  }, []);

  // 候选平台 = 组条目 × 同 platform_type 的用户平台（一平台一次 invoke，不批量化）
  const candidates = useMemo<Candidate[]>(() => {
    if (!group) return [];
    return group.entries.map(entry => ({
      entry,
      platform: platforms.find(p => p.platform_type === entry.platform_code) ?? null,
    }));
  }, [group, platforms]);

  // 实时重算：候选平台各调一次 simulator_est_cost。150ms 防抖——滑杆每档位都触发时
  // 不对 N 平台逐档轰炸 IPC（review 修复轮）。
  useEffect(() => {
    let cancelled = false;
    const runnable = candidates.filter(c => c.platform);
    if (runnable.length === 0) { setResults({}); return; }
    const timer = setTimeout(() => {
      Promise.all(runnable.map(c =>
        costSimulatorApi.estimate({
          platformId: c.platform!.id,
          modelName: c.entry.model_id,
          inputTokens: Math.round(tokens.input),
          outputTokens: Math.round(tokens.output),
          cacheTokens: Math.round(tokens.cache_read),
          cacheWriteTokens: Math.round(tokens.cache_write),
        }).then(dto => [c.entry.platform_code, dto] as const)
          .catch(e => { console.error(e); return null; })
      )).then(pairs => {
        if (cancelled) return;
        const next: Record<string, SimulatorEstCostDto> = {};
        for (const p of pairs) if (p) next[p[0]] = p[1];
        setResults(next);
      });
    }, 150);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [candidates, tokens]);

  const primary = candidates.find(c => c.platform && c.entry.platform_code === group?.primary_platform)
    ?? candidates.find(c => c.platform) ?? null;
  const primaryDto = primary ? results[primary.entry.platform_code] : undefined;

  // 明细行单价：display 用（est_cost 总额仍以 command 返回为准）。
  // peak 命中且条目带 peak 绝对价时 command 的 multiplier 已被压成 1.0，行单价切 peak 档展示。
  const primaryPrice = useMemo(() => {
    if (!primary) return null;
    const p = parsePriceData(primary.entry.price_data);
    const tier: PriceTier = primaryDto?.is_peak && p.peak ? p.peak : p;
    return tier;
  }, [primary, primaryDto]);

  // CNY 条目：Rust 计费链已换算 USD，明细行单价是条目原币种，标 ¥ 避免被读成 $。
  const primaryCny = primary ? parsePriceData(primary.entry.price_data).currency === "CNY" : false;

  const unsupported = (dto: SimulatorEstCostDto) => dto.price_source === "unsupported";

  const comparable = candidates.filter(c => {
    const dto = results[c.entry.platform_code];
    return c.platform && dto && !unsupported(dto);
  });
  const lowestCost = comparable.length
    ? Math.min(...comparable.map(c => results[c.entry.platform_code].est_cost))
    : null;

  const setToken = (dim: Dim, v: number) =>
    setTokens(prev => ({ ...prev, [dim]: Math.max(0, v || 0) }));

  // 粘贴 JSON 折叠解析回填（A4/A5）：只取四维数字回填表单，不出价格
  const parsePaste = () => {
    try {
      const j = JSON.parse(paste) as {
        model?: string;
        messages?: Array<{ content?: unknown }>;
      };
      const modelId = j.model ?? "";
      const hit = groups.find(g =>
        g.canonical_model === modelId || g.entries.some(e => e.model_id === modelId));
      if (hit) setCanonical(hit.canonical_model);
      const chars = (j.messages ?? [])
        .map(m => typeof m.content === "string" ? m.content : JSON.stringify(m.content ?? ""))
        .join("").length;
      const est = Math.ceil(chars / 4);
      setTokens(prev => ({ ...prev, input: est }));
      setHint(t("simulator.parseHint")
        .replace("{model}", modelId || "?")
        .replace("{chars}", chars.toLocaleString())
        .replace("{tokens}", est.toLocaleString())
        + (hit ? "" : ` (${t("simulator.parseNoModel")})`));
    } catch (e) {
      setHint(t("simulator.parseError") + ": " + String(e));
    }
  };

  if (!group) return null;

  return (
    <div style={{ display: "flex", gap: 12, alignItems: "flex-start", flexWrap: "wrap" }}>
      {/* 输入侧 */}
      <div className="glass-surface" style={{ flex: "1 1 320px", padding: 14 }}>
        <div className="text-tertiary" style={{ fontSize: 10, letterSpacing: 1.2, textTransform: "uppercase", marginBottom: 8 }}>
          {t("simulator.pickModel")}
        </div>
        <Select value={canonical} onValueChange={setCanonical}>
          <SelectTrigger data-testid="sim-model" style={{ width: "100%", height: 32, fontSize: F.small }}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {groups.map(g => (
              <SelectItem key={g.canonical_model} value={g.canonical_model}>
                {g.display_name || g.canonical_model}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <div className="text-tertiary" style={{ fontSize: 10, letterSpacing: 1.2, textTransform: "uppercase", margin: "14px 0 8px" }}>
          {t("simulator.tokens")}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px 12px" }}>
          {DIMS.map(dim => (
            <div key={dim}>
              <label className="text-secondary" style={{ fontSize: 11, display: "block", marginBottom: 2 }}>
                {t(`simulator.dim.${dim}`)}
              </label>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <Input
                  data-testid={`sim-tok-${dim}`}
                  type="number" min={0} value={tokens[dim]}
                  onChange={e => setToken(dim, Number(e.target.value))}
                  style={{ width: 110, height: 30, fontSize: F.small, fontFamily: "var(--font-mono, monospace)" }}
                />
                <input
                  data-testid={`sim-range-${dim}`}
                  type="range" min={0} max={RANGE[dim].max} step={RANGE[dim].step}
                  value={Math.min(tokens[dim], RANGE[dim].max)}
                  onChange={e => setToken(dim, Number(e.target.value))}
                  style={{ flex: 1, accentColor: "var(--primary)" }}
                />
              </div>
            </div>
          ))}
        </div>

        <details style={{ marginTop: 12, borderTop: "1px dashed var(--border)", paddingTop: 10 }}>
          <summary className="text-tertiary" style={{ fontSize: 12, cursor: "pointer" }}>
            {t("simulator.pasteSummary")}
          </summary>
          <textarea
            data-testid="sim-paste"
            rows={6}
            value={paste}
            onChange={e => setPaste(e.target.value)}
            placeholder='{"model":"...","messages":[{"role":"user","content":"..."}]}'
            style={{
              width: "100%", marginTop: 8, border: "1px solid var(--border)", borderRadius: 7,
              padding: "7px 9px", fontSize: 11, fontFamily: "var(--font-mono, monospace)",
              background: "var(--bg-card, #fff)", color: "inherit", resize: "vertical",
            }}
          />
          <div style={{ marginTop: 8 }}>
            <Button data-testid="sim-parse" size="sm" onClick={parsePaste}>{t("simulator.parse")}</Button>
          </div>
          {hint && <div className="text-tertiary" data-testid="sim-hint" style={{ fontSize: 10.5, marginTop: 6 }}>{hint}</div>}
        </details>
      </div>

      {/* 输出侧 */}
      <div className="glass-surface" style={{ flex: "1 1 380px", padding: 14 }}>
        <div className="text-tertiary" style={{ fontSize: 10, letterSpacing: 1.2, textTransform: "uppercase", marginBottom: 8 }}>
          {t("simulator.result")}
        </div>
        {primaryDto ? (
          <>
            <div data-testid="sim-total" style={{ fontSize: 28, fontWeight: 700, fontFamily: "var(--font-mono, monospace)" }}>
              {unsupported(primaryDto) ? "-" : formatCostUsd(primaryDto.est_cost)}
              <small className="text-tertiary" style={{ fontSize: 12, fontWeight: 400 }}> {t("simulator.perRequest")}</small>
            </div>
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap", margin: "4px 0 12px" }}>
              <span
                data-testid="sim-peak"
                data-state={primaryDto.is_peak ? "peak" : "off-peak"}
                className="badge"
                style={{
                  fontSize: 10, borderRadius: 4, padding: "2px 7px",
                  background: primaryDto.is_peak ? "rgba(199,126,30,.13)" : "rgba(31,157,97,.12)",
                  color: primaryDto.is_peak ? "var(--warning, #C77E1E)" : "var(--success, #1F9D61)",
                }}
              >
                {primaryDto.is_peak
                  ? t("simulator.peak").replace("{mult}", String(primaryDto.multiplier))
                  : t("simulator.offPeak")}
              </span>
            </div>

            <Table style={{ fontSize: 11.5 }}>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("simulator.colDim")}</TableHead>
                  <TableHead style={{ textAlign: "right" }}>{t("simulator.colUnit")}</TableHead>
                  <TableHead style={{ textAlign: "right" }}>{t("simulator.colQty")}</TableHead>
                  <TableHead style={{ textAlign: "right" }}>{t("simulator.colMult")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {DIMS.map(dim => {
                  const qty = Math.round(tokens[dim]);
                  const unit = primaryPrice?.[dim];
                  return (
                    <TableRow key={dim} data-testid={`sim-row-${dim}`}>
                      <TableCell>{t(`simulator.dim.${dim}`)}</TableCell>
                      <TableCell style={{ textAlign: "right" }}>{typeof unit === "number" ? (primaryCny ? formatCostUsd(unit * 1e6).replace("$", "¥") : formatCostUsd(unit * 1e6)) : "-"}</TableCell>
                      <TableCell style={{ textAlign: "right" }}>{qty.toLocaleString()}</TableCell>
                      <TableCell style={{ textAlign: "right" }}>×{primaryDto.multiplier}</TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
            <div className="text-tertiary" data-testid="sim-source" style={{ fontSize: 10.5, marginTop: 8 }}>
              {t("simulator.priceSource")}: {primaryDto.price_source}
            </div>
          </>
        ) : (
          <div className="text-tertiary" style={{ fontSize: F.hint, padding: 12 }}>
            {t("simulator.noPlatform")}
          </div>
        )}

        <div className="text-tertiary" style={{ fontSize: 10, letterSpacing: 1.2, textTransform: "uppercase", margin: "14px 0 8px" }}>
          {t("simulator.compare")}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
          {candidates.filter(c => c.platform).map(c => {
            const dto = results[c.entry.platform_code];
            const isUnsupported = dto ? unsupported(dto) : false;
            const isLowest = dto != null && !isUnsupported && dto.est_cost === lowestCost
              && comparable.filter(x => results[x.entry.platform_code].est_cost === lowestCost).length === 1;
            return (
              <div
                key={c.entry.platform_code}
                data-testid={`sim-card-${c.entry.platform_code}`}
                data-state={isLowest ? "lowest" : isUnsupported ? "unsupported" : "normal"}
                style={{
                  border: "1px solid " + (isLowest ? "var(--primary)" : "var(--border)"),
                  borderRadius: 7, padding: "9px 11px",
                  background: isLowest ? "var(--accent-subtle)" : "transparent",
                }}
              >
                <div style={{ fontSize: 12, fontWeight: 600 }}>
                  {labelMap[c.entry.platform_code] ?? c.entry.platform_code}
                </div>
                <div style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 16, fontWeight: 700, marginTop: 3 }}>
                  {dto ? (isUnsupported ? "-" : formatCostUsd(dto.est_cost)) : "…"}
                </div>
                <div className="text-tertiary" style={{ fontSize: 10, fontFamily: "var(--font-mono, monospace)" }}>
                  {dto && isLowest && <span style={{ color: "var(--success, #1F9D61)" }}>{t("simulator.lowest")} </span>}
                  {dto && isUnsupported && t("simulator.unsupported")}
                  {c.entry.model_id !== group.canonical_model ? c.entry.model_id : ""}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
