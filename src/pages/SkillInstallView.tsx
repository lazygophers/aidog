// ─── Skills 搜索安装子视图 ──────────────────────────────────
// Skills 页内子视图（本地 state 切换，非顶层 nav）。搜索驱动（skills.sh HTTP 端点 404，
// 仅 `npx skills find <kw>` 可用）。每条 catalog 条目可多选 agent 一次安装。
//
// CatalogEntry.id = `owner/repo@skill`，安装命令 `npx skills add <id> -a <slug> [-g] -y`
//（@skill 已选定子 skill，无需 -s）。

import { useState, useEffect, useCallback, useRef } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import {
  skillsApi,
  type SkillAgent,
  type SkillScope,
  type CatalogEntry,
} from "../services/api";
import claudeIcon from "../assets/platforms/claude_code.svg";
import codexIcon from "../assets/platforms/openai.svg";
import { useReveal, makeRipple } from "@/utils/motion";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card } from "@/components/ui/card";

const AGENTS: SkillAgent[] = ["claude", "codex"];
const AGENT_ICONS: Record<SkillAgent, string> = { claude: claudeIcon, codex: codexIcon };

interface Props {
  scope: SkillScope;
  /** 已装 skill name 集合（用于标记「已装」并禁重复安装）。 */
  installedNames: Set<string>;
  writeReady: boolean;
  onBack: () => void;
  onInstalled: () => void;
}

export function SkillInstallView({
  scope,
  installedNames,
  writeReady,
  onBack,
  onInstalled,
}: Props) {
  const { t } = useTranslation();

  const [keyword, setKeyword] = useState("");
  const [results, setResults] = useState<CatalogEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // 每条 catalog 的 agent 选择（id → 选中 agents），默认全选。
  const [selected, setSelected] = useState<Map<string, Set<SkillAgent>>>(new Map());
  // 批量安装勾选集合（catalog id）。
  const [checked, setChecked] = useState<Set<string>>(new Set());
  // 正在安装的条目 id（非 null 时禁并发）。
  const [busyId, setBusyId] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  // debounce 搜索（350ms）。
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const doSearch = useCallback(
    async (kw: string) => {
      const k = kw.trim();
      if (k === "") {
        setResults([]);
        setError(null);
        setLoading(false);
        return;
      }
      setLoading(true);
      setError(null);
      try {
        const list = await skillsApi.search(k);
        setResults(list);
        // 初始化每条默认全选 agent。
        setSelected((prev) => {
          const next = new Map(prev);
          for (const e of list) {
            if (!next.has(e.id)) next.set(e.id, new Set(AGENTS));
          }
          return next;
        });
      } catch (e: any) {
        setError(e?.toString?.() ?? String(e));
        setResults([]);
      } finally {
        setLoading(false);
      }
    },
    []
  );

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => doSearch(keyword), 350);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [keyword, doSearch]);

  const toggleAgent = (id: string, agent: SkillAgent) => {
    setSelected((prev) => {
      const next = new Map(prev);
      const set = new Set(next.get(id) ?? []);
      if (set.has(agent)) set.delete(agent);
      else set.add(agent);
      next.set(id, set);
      return next;
    });
  };

  const handleInstall = async (entry: CatalogEntry) => {
    const agents = selected.get(entry.id);
    if (!agents || agents.size === 0) return;
    setBusyId(entry.id);
    setMessage(null);
    try {
      const res = await skillsApi.install(
        entry.id,
        Array.from(agents),
        scope
      );
      if (res.success) {
        setMessage(
          t("skills.install.installSuccess", {
            name: entry.name,
            defaultValue: "已安装 {{name}}",
          })
        );
        onInstalled();
      } else {
        setMessage(
          res.stderr?.trim() ||
            res.stdout?.trim() ||
            t("skills.install.installFailed", {
              defaultValue: "安装失败",
            })
        );
      }
    } catch (e: any) {
      setMessage(e?.toString?.() ?? String(e));
    } finally {
      setBusyId(null);
    }
  };

  const toggleChecked = (id: string) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  // 批量安装勾选项：按各条 agent 选择分组（同组 agents 相同 → 一次 installBatch，
  // 后端再把同仓库合并成单次 npx 调用）。结果按组聚合 ok/fail。
  const handleInstallBatch = async () => {
    const entries = effectiveResults.filter((e) => checked.has(e.id));
    if (entries.length === 0) return;
    setBusyId("__batch__");
    setMessage(null);
    const groups = new Map<string, { ids: string[]; agents: SkillAgent[] }>();
    for (const e of entries) {
      const agents = Array.from(selected.get(e.id) ?? []);
      if (agents.length === 0) continue;
      const key = [...agents].sort().join(",");
      const g = groups.get(key) ?? { ids: [], agents };
      g.ids.push(e.id);
      groups.set(key, g);
    }
    let ok = 0;
    let fail = 0;
    for (const g of groups.values()) {
      try {
        const res = await skillsApi.installBatch(g.ids, g.agents, scope);
        if (res.success) ok += g.ids.length;
        else {
          fail += g.ids.length;
          setMessage(res.stderr?.trim() || res.stdout?.trim() || t("skills.install.installFailed", { defaultValue: "安装失败" }));
        }
      } catch (e: any) {
        fail += g.ids.length;
        setMessage(e?.toString?.() ?? String(e));
      }
    }
    setBusyId(null);
    if (ok > 0 && fail === 0) {
      setMessage(t("skills.importOk", { defaultValue: "已导入 {{count}} 项", count: ok }));
    } else if (fail > 0 && ok > 0) {
      setMessage(t("skills.importPartial", { defaultValue: "成功 {{ok}}，失败 {{fail}}", ok, fail }));
    }
    if (ok > 0) {
      setChecked(new Set());
      onInstalled();
    }
  };

  const effectiveResults = results;
  const hasKeyword = keyword.trim() !== "";

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* 批量安装全页 loading（portal 到 document.body，fixed 全屏遮罩） */}
      {busyId === "__batch__" && createPortal(
        <div
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 400,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0, 0, 0, 0.25)",
            backdropFilter: "blur(4px)",
          }}
        >
          <div className="glass-elevated" style={{ padding: "24px 32px", fontSize: 14, display: "flex", alignItems: "center", gap: 12 }}>
            <span style={{ width: 16, height: 16, borderRadius: "50%", border: "2px solid var(--border)", borderTopColor: "var(--accent)", animation: "spin 0.8s linear infinite", flexShrink: 0 }} />
            {t("skills.install.installing", { defaultValue: "安装中…" })}
          </div>
        </div>,
        document.body,
      )}
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <Button
            variant="ghost"
            className="ripple"
            style={{ fontSize: 12 }}
            onClick={(e) => { makeRipple(e); onBack(); }}
            disabled={busyId !== null}
            title={t("skills.install.back", { defaultValue: "返回" })}
          >
            {t("skills.install.back", { defaultValue: "← 返回" })}
          </Button>
          <h2 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>
            {t("skills.install.title", { defaultValue: "添加 Skills" })}
          </h2>
          {loading && (
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("skills.install.searching", { defaultValue: "搜索中…" })}
            </span>
          )}
        </div>
        {/* 批量安装（勾选 ≥1 时可用） */}
        <Button
          className="ripple"
          style={{ fontSize: 12 }}
          disabled={busyId !== null || !writeReady || checked.size === 0}
          onClick={(e) => { makeRipple(e); void handleInstallBatch(); }}
        >
          {busyId === "__batch__"
            ? t("skills.install.installing", { defaultValue: "安装中…" })
            : t("skills.install.installSelected", { defaultValue: "安装选中 ({{count}})", count: checked.size })}
        </Button>
      </div>

      {/* 搜索框 */}
      <Input
        style={{ width: "100%" }}
        value={keyword}
        onChange={(e) => setKeyword(e.target.value)}
        placeholder={t("skills.install.searchPlaceholder", {
          defaultValue: "输入关键字搜索 skills（如 git、docs、deploy）",
        })}
        autoFocus
      />

      {/* 消息条 */}
      {message && (
        <div
          className="glass-surface"
          style={{
            padding: "8px 12px",
            fontSize: 13,
            color: "var(--text-secondary)",
          }}
        >
          {message}
        </div>
      )}

      {/* 错误 */}
      {error && (
        <div
          className="glass-surface"
          style={{ padding: "12px 16px", fontSize: 13, color: "var(--color-danger)" }}
        >
          {t("skills.install.loadFailed", { defaultValue: "加载失败" })}: {error}
        </div>
      )}

      {/* 空态 */}
      {!loading && !error && !hasKeyword && (
        <div
          className="glass-surface"
          style={{ padding: "32px 16px", textAlign: "center", color: "var(--text-secondary)", fontSize: 13 }}
        >
          {t("skills.install.emptyHint", {
            defaultValue: "输入关键字开始搜索可安装的 skills",
          })}
        </div>
      )}

      {/* 无结果 */}
      {!loading && !error && hasKeyword && effectiveResults.length === 0 && (
        <div
          className="glass-surface"
          style={{ padding: "32px 16px", textAlign: "center", color: "var(--text-secondary)", fontSize: 13 }}
        >
          {t("skills.install.noResults", { defaultValue: "无结果" })}
        </div>
      )}

      {/* 结果列表（loading 时隐藏旧数据，避免与新"搜索中"提示并列误导） */}
      {!loading && effectiveResults.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {effectiveResults.map((entry, idx) => (
            <CatalogRow
              key={entry.id}
              entry={entry}
              idx={idx}
              agents={selected.get(entry.id) ?? new Set<SkillAgent>()}
              already={installedNames.has(entry.name)}
              checked={checked.has(entry.id)}
              busyId={busyId}
              writeReady={writeReady}
              onToggle={toggleAgent}
              onCheck={toggleChecked}
              onInstall={handleInstall}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ─── 单行 Catalog ───

// ponytail: 行级 reveal 包装 — 每实例独立 useReveal (React 规则禁 map 内 hook),
// stagger idx*60 错峰，hover-lift + glass-elevated 萤火虫流光描边。
interface CatalogRowProps {
  entry: CatalogEntry;
  idx: number;
  agents: Set<SkillAgent>;
  already: boolean;
  checked: boolean;
  busyId: string | null;
  writeReady: boolean;
  onToggle: (id: string, agent: SkillAgent) => void;
  onCheck: (id: string) => void;
  onInstall: (entry: CatalogEntry) => void;
}

function CatalogRow({ entry, idx, agents, already, checked, busyId, writeReady, onToggle, onCheck, onInstall }: CatalogRowProps) {
  const { t } = useTranslation();
  const { ref, shown } = useReveal<HTMLDivElement>(idx * 60);
  const noAgent = agents.size === 0;
  const installing = busyId === entry.id;
  const otherBusy = busyId !== null && !installing;
  const disabled = installing || otherBusy || !writeReady || already || noAgent;
  return (
    <Card
      ref={ref}
      className={`glass-elevated hover-lift reveal${shown ? " in" : ""}`}
      style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 8 }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
        {/* 批量安装勾选 */}
        <input
          type="checkbox"
          aria-label={t("skills.install.installSelected", { defaultValue: "安装选中 ({{count}})", count: 1 })}
          checked={checked}
          disabled={busyId !== null}
          onChange={() => onCheck(entry.id)}
          style={{ width: 15, height: 15, flexShrink: 0, marginTop: 3, cursor: "pointer", accentColor: "var(--accent)" }}
        />
        <div style={{ display: "flex", flexDirection: "column", gap: 2, minWidth: 0, flex: 1 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            <span style={{ fontSize: 14, fontWeight: 600 }}>{entry.name}</span>
            {already && (
              <span
                style={{
                  fontSize: 11,
                  padding: "1px 6px",
                  borderRadius: 4,
                  background: "var(--accent-subtle)",
                  color: "var(--text-secondary)",
                }}
              >
                {t("skills.install.installed", { defaultValue: "已装" })}
              </span>
            )}
          </div>
          <span
            style={{
              fontSize: 11,
              color: "var(--text-secondary)",
              fontFamily: "var(--font-mono, monospace)",
              wordBreak: "break-all",
            }}
          >
            {entry.id}
          </span>
          {entry.description && (
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {entry.description}
            </span>
          )}
          {entry.repo_url && (
            <a
              href={entry.repo_url}
              target="_blank"
              rel="noreferrer"
              style={{ fontSize: 11, color: "var(--accent)" }}
            >
              {entry.repo_url}
            </a>
          )}
        </div>
        <Button
          className="ripple"
          style={{ fontSize: 12, flexShrink: 0 }}
          disabled={disabled}
          onClick={(e) => { makeRipple(e); onInstall(entry); }}
          title={
            otherBusy
              ? t("skills.install.busyOther", {
                  defaultValue: "等待当前安装完成",
                })
              : undefined
          }
        >
          {installing
            ? t("skills.install.installing", { defaultValue: "安装中…" })
            : already
              ? t("skills.install.installed", { defaultValue: "已装" })
              : t("skills.install.install", { defaultValue: "安装" })}
        </Button>
      </div>
      {/* agent 选择 */}
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
          {t("skills.install.selectAgent", { defaultValue: "安装到" })}:
        </span>
        {AGENTS.map((a) => {
          const on = agents.has(a);
          return (
            <Button
              key={a}
              variant={on ? "default" : "outline"}
              className="ripple"
              style={{
                padding: "4px 8px",
                display: "flex",
                alignItems: "center",
                gap: 4,
                opacity: on ? 1 : 0.4,
                fontSize: 11,
              }}
              onClick={(e) => { makeRipple(e); onToggle(entry.id, a); }}
              title={t(`skills.agent.${a}`, a)}
            >
              <img
                src={AGENT_ICONS[a]}
                alt={a}
                className="hover-lift"
                style={{ width: 16, height: 16, filter: on ? "none" : "grayscale(1)" }}
              />
              {t(`skills.agent.${a}`, a)}
            </Button>
          );
        })}
      </div>
    </Card>
  );
}
