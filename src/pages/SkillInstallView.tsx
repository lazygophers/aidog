// ─── Skills 搜索安装子视图 ──────────────────────────────────
// Skills 页内子视图（本地 state 切换，非顶层 nav）。搜索驱动（skills.sh HTTP 端点 404，
// 仅 `npx skills find <kw>` 可用）。每条 catalog 条目可多选 agent 一次安装。
//
// CatalogEntry.id = `owner/repo@skill`，安装命令 `npx skills add <id> -a <slug> [-g] -y`
//（@skill 已选定子 skill，无需 -s）。

import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  skillsApi,
  type SkillAgent,
  type SkillScope,
  type CatalogEntry,
} from "../services/api";
import claudeIcon from "../assets/platforms/claude_code.svg";
import codexIcon from "../assets/platforms/openai.svg";
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

  const effectiveResults = results;
  const hasKeyword = keyword.trim() !== "";

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <Button
            variant="ghost"
            style={{ fontSize: 12 }}
            onClick={onBack}
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
          style={{ padding: "12px 16px", fontSize: 13, color: "var(--danger, #e06c75)" }}
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
          {effectiveResults.map((entry) => {
            const agents = selected.get(entry.id) ?? new Set<SkillAgent>();
            const already = installedNames.has(entry.name);
            const noAgent = agents.size === 0;
            const installing = busyId === entry.id;
            // 并发锁：另一条正在安装时本条按钮禁用，防止 setBusyId 覆盖丢失前一条"安装中"状态。
            const otherBusy = busyId !== null && !installing;
            const disabled =
              installing || otherBusy || !writeReady || already || noAgent;
            return (
              <Card
                key={entry.id}
                className="glass-elevated"
                style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 8 }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
                  <div style={{ display: "flex", flexDirection: "column", gap: 2, minWidth: 0, flex: 1 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                      <span style={{ fontSize: 14, fontWeight: 600 }}>{entry.name}</span>
                      {already && (
                        <span
                          style={{
                            fontSize: 11,
                            padding: "1px 6px",
                            borderRadius: 4,
                            background: "var(--accent-subtle, rgba(0,0,0,0.06))",
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
                        style={{ fontSize: 11, color: "var(--accent, #5b8def)" }}
                      >
                        {entry.repo_url}
                      </a>
                    )}
                  </div>
                  <Button
                    style={{ fontSize: 12, flexShrink: 0 }}
                    disabled={disabled}
                    onClick={() => handleInstall(entry)}
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
                        style={{
                          padding: "4px 8px",
                          display: "flex",
                          alignItems: "center",
                          gap: 4,
                          opacity: on ? 1 : 0.4,
                          fontSize: 11,
                        }}
                        onClick={() => toggleAgent(entry.id, a)}
                        title={t(`skills.agent.${a}`, a)}
                      >
                        <img
                          src={AGENT_ICONS[a]}
                          alt={a}
                          style={{ width: 16, height: 16, filter: on ? "none" : "grayscale(1)" }}
                        />
                        {t(`skills.agent.${a}`, a)}
                      </Button>
                    );
                  })}
                </div>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}
