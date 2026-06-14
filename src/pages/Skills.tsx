// ─── Skills 管理页 ──────────────────────────────────────────
// 顶层侧栏入口。统一已装列表（一条/skill，不分 agent）。
// 每行右侧展示 claude/codex 图标：在 enabled_agents 内=启用样式，否则=未启用样式，可点切换。
// 所有操作（list/enable/disable/update）全走后端 npx skills（无手动 fs）。
//
// scope 默认 Global（用户级全局 -g），可选 Project（选某项目目录）。
// npx/node 缺失 → 顶部提示条引导装 node，不阻塞整页。

import { useState, useEffect, useCallback, useMemo } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import {
  skillsApi,
  type SkillAgent,
  type SkillScope,
  type SkillsEnv,
  type SkillInfo,
  type SkillsOpResult,
} from "../services/api";
import { SkillInstallView } from "./SkillInstallView";
import claudeIcon from "../assets/platforms/claude_code.svg";
import codexIcon from "../assets/platforms/openai.svg";

const AGENTS: SkillAgent[] = ["claude", "codex"];
const AGENT_ICONS: Record<SkillAgent, string> = { claude: claudeIcon, codex: codexIcon };

export function Skills() {
  const { t } = useTranslation();

  const [env, setEnv] = useState<SkillsEnv | null>(null);
  const [scopeKind, setScopeKind] = useState<"global" | "project">("global");
  const [projectPath, setProjectPath] = useState("");
  // 子视图：list = 已装列表（默认）；install = 搜索安装页（按钮切换 + 返回）。
  const [subView, setSubView] = useState<"list" | "install">("list");

  const [installed, setInstalled] = useState<SkillInfo[]>([]);
  // 冷启动加载态（仅无缓存命中时显整页 loading）。
  const [installedLoading, setInstalledLoading] = useState(false);
  // 后台刷新态（SWR revalidate 中，显小"刷新中"指示，不阻塞列表）。
  const [refreshing, setRefreshing] = useState(false);

  // 切换中标识："<name>::<agent>" 或 "__update__" / "__uninstall__"；非 null 时禁并发。
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  // 一键卸载二次确认 modal（破坏性操作，禁 native confirm）。
  const [confirmUninstall, setConfirmUninstall] = useState(false);
  // 单条卸载目标（破坏性，二次确认）。
  const [uninstallTarget, setUninstallTarget] = useState<SkillInfo | null>(null);
  // 对齐配置 modal：使 to agent 的启用配置与 from 完全一致。
  const [alignOpen, setAlignOpen] = useState(false);
  const [alignFrom, setAlignFrom] = useState<SkillAgent>("claude");
  const [alignTo, setAlignTo] = useState<SkillAgent>("codex");
  // 组级卸载目标（破坏性，二次确认）。
  const [uninstallGroupTarget, setUninstallGroupTarget] = useState<{
    source: string | null;
    label: string;
    count: number;
  } | null>(null);

  // 当前 scope 对象（供 API 调用）。
  const scope: SkillScope =
    scopeKind === "project"
      ? { kind: "project", path: projectPath }
      : { kind: "global" };

  const writeReady = !!env?.npx_available;
  const scopeInvalid = scopeKind === "project" && projectPath.trim() === "";

  // 折叠组集合（key = source 或 "__other__"）。默认全展开。
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const toggleGroup = (key: string) =>
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  // 按 source 分组：有 source → owner/repo 组（字母序）；无 source → 「其他」组（置末）。
  const groups = useMemo(() => {
    const map = new Map<string, { source: string | null; skills: SkillInfo[] }>();
    for (const s of installed) {
      const key = s.source ?? "__other__";
      if (!map.has(key)) map.set(key, { source: s.source, skills: [] });
      map.get(key)!.skills.push(s);
    }
    return Array.from(map.values()).sort((a, b) => {
      if (a.source === null && b.source !== null) return 1;
      if (a.source !== null && b.source === null) return -1;
      return (a.source ?? "").localeCompare(b.source ?? "");
    });
  }, [installed]);

  // 环境探测（进页一次）。
  useEffect(() => {
    skillsApi.checkEnv().then(setEnv).catch((e) => console.error("check env failed", e));
  }, []);

  // SWR 后台刷新：强制跑 npx 取最新、更新缓存与列表（写操作后 + scope 切换 revalidate 调用）。
  // 不阻塞列表，仅置 refreshing 指示。
  const refreshInstalled = useCallback(async () => {
    if (scopeInvalid) {
      setInstalled([]);
      return;
    }
    setRefreshing(true);
    try {
      const res = await skillsApi.listRefresh(scope);
      setInstalled(res.items);
    } catch (e) {
      console.error("refresh installed failed", e);
    } finally {
      setRefreshing(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKind, projectPath]);

  // 开页/切 scope：纯缓存渲染（命中即 0 子进程，无自动 refresh、无 spinner、无列表跳变）。
  // 仅冷启动（无缓存命中 stale，或缓存读取失败）才跑一次 listRefresh 填充并落盘（显整页 loading）。
  // 命中缓存绝不自动跑 npx；用户需最新态时走显式「刷新」按钮（refreshInstalled）。
  const loadInstalled = useCallback(async () => {
    if (scopeInvalid) {
      setInstalled([]);
      return;
    }
    try {
      const cached = await skillsApi.listInstalled(scope);
      if (!cached.stale) {
        // 命中缓存：瞬间渲染，结束（不自动 refresh）。
        setInstalled(cached.items);
        return;
      }
      // 冷启动：无缓存 → 落到下方 refresh 填充。
    } catch (e) {
      console.error("list installed (cache) failed", e);
      // 缓存读取失败也兜底走 refresh。
    }
    // 冷启动 / 缓存失败：显加载态，跑一次 refresh 填充并落盘。
    setInstalledLoading(true);
    try {
      const res = await skillsApi.listRefresh(scope);
      setInstalled(res.items);
    } catch (e) {
      console.error("list installed (refresh) failed", e);
    } finally {
      setInstalledLoading(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKind, projectPath]);

  useEffect(() => {
    loadInstalled();
  }, [loadInstalled]);

  // 操作结果消息自动消失（4s），避免遮屏。
  useEffect(() => {
    if (!message) return;
    const id = setTimeout(() => setMessage(null), 4000);
    return () => clearTimeout(id);
  }, [message]);

  // 统计：总计 + 每 agent 启用数（从已装列表派生，随列表刷新）。
  const total = installed.length;
  const agentCounts: Record<SkillAgent, number> = {
    claude: installed.filter((s) => s.enabled_agents.includes("claude")).length,
    codex: installed.filter((s) => s.enabled_agents.includes("codex")).length,
  };

  const pickProjectDir = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("skills.chooseProjectDir", "选择项目目录"),
      });
      if (typeof selected === "string") setProjectPath(selected);
    } catch {
      // user cancelled
    }
  };

  // 统一处理写操作结果 → toast + 刷新已装。
  const applyResult = async (res: SkillsOpResult, okKey: string) => {
    if (res.success) {
      setMessage(t(okKey, "操作成功"));
      // 写后缓存已失效（后端），强制 refresh 取真实态。
      await refreshInstalled();
    } else {
      const err = res.stderr.trim() || res.stdout.trim() || t("skills.opFailed", "操作失败");
      setMessage(err);
    }
  };

  // 切换某 skill 在某 agent 的启用态：已启用→disable，未启用→enable。
  // 乐观更新：立即翻转本地状态（counts 派生自动跟随），失败回滚 + 弹错；成功保留乐观态，不全量重载。
  const handleToggle = async (skill: SkillInfo, agent: SkillAgent) => {
    if (!writeReady || scopeInvalid || busyKey !== null) return;
    const enabled = skill.enabled_agents.includes(agent);
    setBusyKey(`${skill.name}::${agent}`);
    setMessage(null);

    // 乐观翻转：保存回滚快照 → 立即更新 UI。
    const prev = installed;
    setInstalled((list) =>
      list.map((s) =>
        s.name === skill.name
          ? {
              ...s,
              enabled_agents: enabled
                ? s.enabled_agents.filter((a) => a !== agent)
                : [...s.enabled_agents, agent],
            }
          : s,
      ),
    );

    try {
      const res = enabled
        ? await skillsApi.disable(skill.name, agent, scope)
        : await skillsApi.enable(
            skill.name,
            skill.installed_path ?? "",
            agent,
            scope,
          );
      if (res.success) {
        setMessage(t(enabled ? "skills.disabled" : "skills.enabled", "操作成功"));
      } else {
        // 后端失败：回滚乐观改动 + 弹错。
        setInstalled(prev);
        const err = res.stderr.trim() || res.stdout.trim() || t("skills.opFailed", "操作失败");
        setMessage(err);
      }
    } catch (e) {
      console.error("toggle failed", e);
      setInstalled(prev);
      setMessage(String(e));
    } finally {
      setBusyKey(null);
    }
  };

  const handleUpdate = async () => {
    if (!writeReady || scopeInvalid || busyKey !== null) return;
    setBusyKey("__update__");
    setMessage(null);
    try {
      const res = await skillsApi.update(scope);
      await applyResult(res, "skills.updated");
    } catch (e) {
      console.error("update failed", e);
      setMessage(String(e));
    } finally {
      setBusyKey(null);
    }
  };

  // 一键卸载当前 scope 所有平台所有 skills（破坏性，需二次确认）。
  const handleUninstallAll = async () => {
    setConfirmUninstall(false);
    if (!writeReady || scopeInvalid || busyKey !== null) return;
    setBusyKey("__uninstall__");
    setMessage(null);
    try {
      const res = await skillsApi.uninstallAll(scope);
      await applyResult(res, "skills.uninstallAllDone");
    } catch (e) {
      console.error("uninstall all failed", e);
      setMessage(String(e));
    } finally {
      setBusyKey(null);
    }
  };

  // 卸载单一 skill（破坏性，二次确认）：删规范存储 + 所有 agent 启用配置。
  const handleUninstallSingle = async () => {
    const target = uninstallTarget;
    setUninstallTarget(null);
    if (!target || !writeReady || scopeInvalid || busyKey !== null) return;
    setBusyKey(`__uninstall_single_${target.name}__`);
    setMessage(null);
    try {
      const res = await skillsApi.uninstall(target.name, scope);
      await applyResult(res, "skills.uninstallDone");
    } catch (e) {
      console.error("uninstall single failed", e);
      setMessage(String(e));
    } finally {
      setBusyKey(null);
    }
  };

  // 组级卸载（破坏性，二次确认 modal 已过）：卸载某 source 组内所有 skill。
  const handleUninstallGroup = async () => {
    const target = uninstallGroupTarget;
    setUninstallGroupTarget(null);
    if (!target || !writeReady || scopeInvalid || busyKey !== null) return;
    setBusyKey(`__uninstall_group_${target.source ?? "__other__"}__`);
    setMessage(null);
    try {
      const res = await skillsApi.uninstallGroup(target.source, scope);
      await applyResult(res, "skills.uninstallDone");
    } catch (e) {
      console.error("uninstall group failed", e);
      setMessage(String(e));
    } finally {
      setBusyKey(null);
    }
  };

  // 对齐配置：使 to 的启用配置与 from 完全一致。
  const handleAlign = async () => {
    if (alignFrom === alignTo) return;
    setAlignOpen(false);
    if (!writeReady || scopeInvalid || busyKey !== null) return;
    setBusyKey("__align__");
    setMessage(null);
    try {
      const res = await skillsApi.alignAgents(alignFrom, alignTo, scope);
      if (res.success) {
        // stdout 形如 "aligned N changes (...)"；N=0 视为 noop。
        const m = res.stdout.match(/aligned (\d+) changes/);
        const n = m ? Number(m[1]) : 0;
        setMessage(
          n === 0
            ? t("skills.alignNoop", "两 agent 配置已一致，无需对齐")
            : t("skills.alignDone", "已对齐 {{count}} 项变更", { count: n }),
        );
        await refreshInstalled();
      } else {
        const err = res.stderr.trim() || res.stdout.trim() || t("skills.opFailed", "操作失败");
        setMessage(err);
      }
    } catch (e) {
      console.error("align failed", e);
      setMessage(String(e));
    } finally {
      setBusyKey(null);
    }
  };

  // 为某 agent 启用全部已装 skills（只增不减，非破坏性）。
  const handleEnableAll = async (agent: SkillAgent) => {
    if (!writeReady || scopeInvalid || busyKey !== null) return;
    setBusyKey(`__enableall_${agent}__`);
    setMessage(null);
    try {
      const res = await skillsApi.enableAll(agent, scope);
      if (res.success) {
        // stdout 形如 "enabled N skills"；N=0 视为 noop。
        const m = res.stdout.match(/enabled (\d+) skills/);
        const n = m ? Number(m[1]) : 0;
        setMessage(
          n === 0
            ? t("skills.enableAllNoop", "{{agent}} 已全部启用", { agent: t(`skills.agent.${agent}`, agent) })
            : t("skills.enableAllDone", "已为 {{agent}} 启用 {{count}} 项", {
                agent: t(`skills.agent.${agent}`, agent),
                count: n,
              }),
        );
        await refreshInstalled();
      } else {
        const err = res.stderr.trim() || res.stdout.trim() || t("skills.opFailed", "操作失败");
        setMessage(err);
      }
    } catch (e) {
      console.error("enable all failed", e);
      setMessage(String(e));
    } finally {
      setBusyKey(null);
    }
  };

  // 组级 agent 批量：全启用→点=批量 disable；部分/全未→点=批量 enable（补齐缺的）。
  const handleSetGroupAgent = async (source: string | null, agent: SkillAgent) => {
    const key = `group:${source ?? "__other__"}:${agent}`;
    if (busyKey !== null) return;
    const groupSkills = groups.find((g) => g.source === source)?.skills ?? [];
    if (groupSkills.length === 0) return;
    const enabledCount = groupSkills.filter((s) => s.enabled_agents.includes(agent)).length;
    const enable = enabledCount < groupSkills.length;
    setBusyKey(key);
    setMessage(null);
    try {
      const res = await skillsApi.setGroupAgent(source, agent, enable, scope);
      if (res.success) {
        setMessage(
          t("skills.groupUpdated", "已更新组内 skills", {
            count: groupSkills.length,
            agent: t(`skills.agent.${agent}`, agent),
          }),
        );
        await refreshInstalled();
      } else {
        const err = res.stderr.trim() || res.stdout.trim() || t("skills.opFailed", "操作失败");
        setMessage(err);
      }
    } catch (e) {
      console.error("set group agent failed", e);
      setMessage(String(e));
    } finally {
      setBusyKey(null);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <h2 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>{t("skills.title", "Skills")}</h2>
          {refreshing && !installedLoading && (
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("skills.refreshing", "刷新中…")}
            </span>
          )}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="btn btn-primary"
            style={{ fontSize: 12 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null}
            onClick={() => setSubView("install")}
            title={t("skills.install.addBtn", "添加 Skills")}
          >
            {t("skills.install.addBtn", "+ 添加")}
          </button>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12 }}
            disabled={scopeInvalid || busyKey !== null || refreshing}
            onClick={refreshInstalled}
            title={t("skills.refresh", "刷新")}
          >
            {refreshing ? t("skills.refreshing", "刷新中…") : t("skills.refresh", "刷新")}
          </button>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null}
            onClick={handleUpdate}
          >
            {busyKey === "__update__" ? t("skills.updating", "更新中…") : t("skills.updateAll", "更新全部")}
          </button>
          <button
            className="btn btn-danger"
            style={{ fontSize: 12 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0}
            onClick={() => setConfirmUninstall(true)}
          >
            {busyKey === "__uninstall__" ? t("skills.uninstalling", "卸载中…") : t("skills.uninstallAll", "卸载全部")}
          </button>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0}
            onClick={() => setAlignOpen(true)}
          >
            {busyKey === "__align__" ? t("skills.aligning", "对齐中…") : t("skills.alignTitle", "对齐配置")}
          </button>
        </div>
      </div>

      {/* 子视图: list = 已装列表 (默认); install = 搜索安装页 */}
      {subView === "list" && (
      <>
      {/* 环境缺失提示条 */}
      {env && !env.npx_available && (
        <div
          className="glass-surface"
          style={{
            padding: "12px 16px",
            fontSize: 13,
            color: "var(--text-secondary)",
            borderInlineStart: "3px solid var(--accent)",
          }}
        >
          {t("skills.envMissing", "未检测到 npx / Node.js，安装与更新功能不可用。请先安装 Node.js。")}
        </div>
      )}

      {/* 操作结果消息（portal 到 document.body：脱离 Skills 页 transform 祖先，fixed 始终相对 viewport 顶部居中；4s 自动消失） */}
      {message && createPortal(
        <div
          style={{
            position: "fixed",
            top: 16,
            left: "50%",
            transform: "translateX(-50%)",
            zIndex: 300,
            maxWidth: "calc(100vw - 32px)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            style={{
              padding: "10px 16px",
              fontSize: 12,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
              display: "flex",
              alignItems: "center",
              gap: 12,
              boxShadow: "0 8px 24px rgba(0,0,0,0.18)",
            }}
          >
            <span style={{ flex: 1 }}>{message}</span>
            <button
              type="button"
              onClick={() => setMessage(null)}
              aria-label={t("action.dismiss", "关闭")}
              style={{
                background: "transparent",
                border: "none",
                color: "var(--text-secondary)",
                cursor: "pointer",
                fontSize: 14,
                padding: 0,
                lineHeight: 1,
              }}
            >
              ✕
            </button>
          </div>
        </div>,
        document.body,
      )}

      {/* 统计 + scope 筛选 (合并单卡: 左统计 右筛选右对齐) */}
      <div
        className="glass-elevated"
        style={{
          padding: "20px 24px",
          display: "flex",
          alignItems: "center",
          gap: 28,
          flexWrap: "wrap",
          justifyContent: "space-between",
        }}
      >
        {/* 左: 统计 */}
        <div style={{ display: "flex", alignItems: "center", gap: 28, flexWrap: "wrap" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            <span style={{ fontSize: 40, fontWeight: 800, lineHeight: 1, color: "var(--accent)" }}>
              {total}
            </span>
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("skills.total", "已安装总计")}
            </span>
          </div>
          <div style={{ display: "flex", gap: 20 }}>
            {AGENTS.map((a) => (
              <div key={a} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <img src={AGENT_ICONS[a]} alt={t(`skills.agent.${a}`, a)} style={{ width: 22, height: 22 }} />
                <div style={{ display: "flex", flexDirection: "column" }}>
                  <span style={{ fontSize: 18, fontWeight: 700, lineHeight: 1.1 }}>{agentCounts[a]}</span>
                  <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                    {t(`skills.agent.${a}`, a)}
                  </span>
                </div>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 11, padding: "3px 8px" }}
                  disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0 || agentCounts[a] === installed.length}
                  onClick={() => handleEnableAll(a)}
                  title={t("skills.enableAll", "全部启用")}
                >
                  {busyKey === `__enableall_${a}__` ? t("skills.enabling", "启用中…") : t("skills.enableAll", "全部启用")}
                </button>
              </div>
            ))}
          </div>
        </div>

        {/* 右: scope 筛选 (右对齐) */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8, alignItems: "flex-end" }}>
          <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
            <span className="text-secondary">{t("skills.scope", "范围")}</span>
            <select
              className="input"
              style={{ width: "auto" }}
              value={scopeKind}
              onChange={(e) => setScopeKind(e.target.value as "global" | "project")}
            >
              <option value="global">{t("skills.scopeGlobal", "用户级（全局）")}</option>
              <option value="project">{t("skills.scopeProject", "项目级")}</option>
            </select>
          </label>
          {scopeKind === "project" && (
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <input
                className="input"
                style={{ flex: 1 }}
                placeholder={t("skills.projectPathPlaceholder", "项目目录绝对路径")}
                value={projectPath}
                onChange={(e) => setProjectPath(e.target.value)}
              />
              <button className="btn" style={{ fontSize: 12 }} onClick={pickProjectDir}>
                {t("skills.browse", "浏览…")}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* 已装列表（统一一条/skill） */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {installedLoading ? (
          <div className="text-secondary" style={{ padding: 12 }}>{t("status.loading", "加载中…")}</div>
        ) : installed.length === 0 ? (
          <div className="glass-surface text-secondary" style={{ padding: "24px 16px", textAlign: "center", fontSize: 13 }}>
            {t("skills.installedEmpty", "当前范围下暂无已安装 skills")}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {groups.map((group) => {
              const groupKey = group.source ?? "__other__";
              const collapsed = collapsedGroups.has(groupKey);
              const groupLabel = group.source ?? t("skills.groupOther", "其他");
              return (
                <div key={groupKey} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {/* 组头：折叠 + owner/repo + 计数 + 组级 agent 批量图标 */}
                  <div
                    className="glass-elevated"
                    style={{ padding: "10px 16px", display: "flex", alignItems: "center", gap: 12 }}
                  >
                    <button
                      type="button"
                      onClick={() => toggleGroup(groupKey)}
                      style={{
                        flex: 1,
                        minWidth: 0,
                        display: "flex",
                        alignItems: "center",
                        gap: 10,
                        cursor: "pointer",
                        background: "transparent",
                        border: "none",
                        color: "inherit",
                      }}
                      aria-expanded={!collapsed}
                    >
                      <span
                        style={{
                          fontSize: 11,
                          display: "inline-block",
                          transform: collapsed ? "rotate(-90deg)" : "none",
                          transition: "transform 0.15s",
                          color: "var(--text-secondary)",
                        }}
                      >
                        ▾
                      </span>
                      <span style={{ fontSize: 13, fontWeight: 700, fontFamily: "var(--font-mono, monospace)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {groupLabel}
                      </span>
                      <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                        {t("skills.skillCount", "{{n}} 个", { n: group.skills.length })}
                      </span>
                    </button>
                    {/* 组级 agent 批量：全启→disable；部分/全未→enable */}
                    <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                      {AGENTS.map((a) => {
                        const enabledCount = group.skills.filter((s) => s.enabled_agents.includes(a)).length;
                        const allOn = enabledCount === group.skills.length;
                        const noneOn = enabledCount === 0;
                        const busy = busyKey === `group:${groupKey}:${a}`;
                        const label = t(`skills.agent.${a}`, a);
                        const aria = allOn
                          ? t("skills.disableAgent", "关闭 {{agent}}", { agent: label })
                          : t("skills.enableAgent", "启用 {{agent}}", { agent: label });
                        return (
                          <button
                            key={a}
                            type="button"
                            className="glass"
                            title={aria}
                            aria-label={aria}
                            disabled={!writeReady || busyKey !== null}
                            onClick={() => handleSetGroupAgent(group.source, a)}
                            style={{
                              display: "flex",
                              alignItems: "center",
                              gap: 4,
                              padding: "4px 8px",
                              cursor: writeReady && busyKey === null ? "pointer" : "default",
                              borderRadius: 8,
                              border: allOn ? "1.5px solid var(--accent)" : "1px solid var(--border)",
                              background: allOn ? "var(--accent-subtle)" : "transparent",
                              opacity: allOn ? 1 : noneOn ? 0.45 : 0.7,
                              transition: "opacity 0.15s, border-color 0.15s, background 0.15s",
                            }}
                          >
                            <img
                              src={AGENT_ICONS[a]}
                              alt={label}
                              style={{ width: 16, height: 16, filter: allOn ? "none" : "grayscale(1)" }}
                            />
                            <span style={{ fontSize: 10, fontWeight: 600 }}>
                              {busy ? "…" : `${enabledCount}/${group.skills.length}`}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                    {/* 组级卸载（破坏性，二次确认） */}
                    <button
                      className="btn btn-danger"
                      style={{ fontSize: 11, padding: "4px 10px", flexShrink: 0 }}
                      disabled={!writeReady || busyKey !== null}
                      onClick={() =>
                        setUninstallGroupTarget({
                          source: group.source,
                          label: groupLabel,
                          count: group.skills.length,
                        })
                      }
                      title={t("skills.uninstallGroup", "卸载整组")}
                    >
                      {busyKey === `__uninstall_group_${groupKey}__`
                        ? t("skills.uninstalling", "卸载中…")
                        : t("skills.uninstallGroup", "卸载整组")}
                    </button>
                  </div>
                  {/* 组内行（折叠时隐藏） */}
                  {!collapsed && (
                    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                      {group.skills.map((skill) => (
                        <div
                          key={skill.name}
                          className="glass-surface"
                          style={{ padding: "12px 16px", display: "flex", gap: 12, alignItems: "center" }}
                        >
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <div style={{ fontSize: 13, fontWeight: 600 }}>{skill.name}</div>
                            {skill.description?.trim() && (
                              <div style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>{skill.description}</div>
                            )}
                          </div>
                          {/* 右侧 agent 启用切换 */}
                          <div style={{ display: "flex", gap: 8, alignItems: "center", flexShrink: 0 }}>
                            {AGENTS.map((a) => {
                              const enabled = skill.enabled_agents.includes(a);
                              const busy = busyKey === `${skill.name}::${a}`;
                              const label = t(`skills.agent.${a}`, a);
                              const aria = enabled
                                ? t("skills.disableAgent", "关闭 {{agent}}", { agent: label })
                                : t("skills.enableAgent", "启用 {{agent}}", { agent: label });
                              return (
                                <button
                                  key={a}
                                  type="button"
                                  className="glass"
                                  title={aria}
                                  aria-label={aria}
                                  aria-pressed={enabled}
                                  disabled={!writeReady || busyKey !== null}
                                  onClick={() => handleToggle(skill, a)}
                                  style={{
                                    display: "flex",
                                    alignItems: "center",
                                    gap: 6,
                                    padding: "5px 10px",
                                    cursor: writeReady && busyKey === null ? "pointer" : "default",
                                    borderRadius: 10,
                                    border: enabled ? "1.5px solid var(--accent)" : "1px solid var(--border)",
                                    background: enabled ? "var(--accent-subtle)" : "transparent",
                                    opacity: enabled ? 1 : 0.45,
                                    transition: "opacity 0.15s, border-color 0.15s, background 0.15s",
                                  }}
                                >
                                  <img
                                    src={AGENT_ICONS[a]}
                                    alt={label}
                                    style={{ width: 18, height: 18, filter: enabled ? "none" : "grayscale(1)" }}
                                  />
                                  <span style={{ fontSize: 11, fontWeight: 600 }}>
                                    {busy ? t("skills.toggling", "…") : enabled ? t("skills.on", "启用") : t("skills.off", "未启用")}
                                  </span>
                                </button>
                              );
                            })}
                          </div>
                          {/* 单条卸载（破坏性，二次确认） */}
                          <button
                            className="btn btn-danger"
                            style={{ fontSize: 11, padding: "4px 10px", flexShrink: 0 }}
                            disabled={!writeReady || busyKey !== null}
                            onClick={() => setUninstallTarget(skill)}
                            title={t("skills.uninstall", "卸载")}
                          >
                            {busyKey === `__uninstall_single_${skill.name}__`
                              ? t("skills.uninstalling", "卸载中…")
                              : t("skills.uninstall", "卸载")}
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
      </>
      )}

      {/* 搜索安装子视图 */}
      {subView === "install" && (
        <SkillInstallView
          scope={scope}
          installedNames={new Set(installed.map((s) => s.name))}
          writeReady={writeReady}
          onBack={() => setSubView("list")}
          onInstalled={refreshInstalled}
        />
      )}

      {/* 一键卸载二次确认 modal（破坏性，禁 native confirm） */}
      {/* createPortal 到 document.body：脱离 Skills 页 transform 祖先，fixed 始终相对 viewport 居中 */}
      {confirmUninstall && createPortal(
        <div
          onClick={() => setConfirmUninstall(false)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.4)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{
              maxWidth: 380,
              padding: 24,
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.uninstallAll", "卸载全部")}</div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("skills.uninstallAllConfirm", "将删除当前范围下所有平台的全部 {{count}} 个 skills，不可恢复。确认？", { count: installed.length })}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setConfirmUninstall(false)}>
                {t("action.cancel", "取消")}
              </button>
              <button className="btn btn-danger" style={{ fontSize: 13 }} onClick={handleUninstallAll}>
                {t("skills.uninstallAll", "卸载全部")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 单条卸载二次确认 modal（破坏性，禁 native confirm） */}
      {uninstallTarget && createPortal(
        <div
          onClick={() => setUninstallTarget(null)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.4)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{
              maxWidth: 380,
              padding: 24,
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.uninstall", "卸载")}</div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("skills.uninstallConfirm", "将删除 skill {{name}} 及其在所有 agent 的启用配置，不可恢复。确认？", { name: uninstallTarget.name })}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setUninstallTarget(null)}>
                {t("action.cancel", "取消")}
              </button>
              <button className="btn btn-danger" style={{ fontSize: 13 }} onClick={handleUninstallSingle}>
                {t("skills.uninstall", "卸载")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 组级卸载二次确认 modal（破坏性，禁 native confirm） */}
      {uninstallGroupTarget && createPortal(
        <div
          onClick={() => setUninstallGroupTarget(null)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.4)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{
              maxWidth: 420,
              padding: 24,
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.uninstallGroup", "卸载整组")}</div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t(
                "skills.uninstallGroupConfirm",
                "将卸载 {{group}} 下 {{count}} 个 skill 及其在所有 agent 的启用配置，不可恢复。确认？",
                { group: uninstallGroupTarget.label, count: uninstallGroupTarget.count },
              )}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setUninstallGroupTarget(null)}>
                {t("action.cancel", "取消")}
              </button>
              <button className="btn btn-danger" style={{ fontSize: 13 }} onClick={handleUninstallGroup}>
                {t("skills.uninstallGroup", "卸载整组")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 对齐配置 modal：使 to 与 from 启用配置一致 */}
      {alignOpen && createPortal(
        <div
          onClick={() => setAlignOpen(false)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.4)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{
              maxWidth: 400,
              padding: 24,
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.alignTitle", "对齐配置")}</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                <span style={{ minWidth: 72 }} className="text-secondary">{t("skills.alignFrom", "源 agent")}</span>
                <select className="input" style={{ flex: 1 }} value={alignFrom} onChange={(e) => setAlignFrom(e.target.value as SkillAgent)}>
                  {AGENTS.map((a) => (
                    <option key={a} value={a}>{t(`skills.agent.${a}`, a)}</option>
                  ))}
                </select>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                <span style={{ minWidth: 72 }} className="text-secondary">{t("skills.alignTo", "目标 agent")}</span>
                <select className="input" style={{ flex: 1 }} value={alignTo} onChange={(e) => setAlignTo(e.target.value as SkillAgent)}>
                  {AGENTS.map((a) => (
                    <option key={a} value={a}>{t(`skills.agent.${a}`, a)}</option>
                  ))}
                </select>
              </label>
            </div>
            <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {alignFrom === alignTo
                ? t("skills.alignSameAgent", "源与目标不能相同")
                : t("skills.alignConfirm", "将使 {{to}} 的启用配置与 {{from}} 完全一致（启用 {{from}} 已启用的、关闭 {{from}} 未启用的）。", {
                    from: t(`skills.agent.${alignFrom}`, alignFrom),
                    to: t(`skills.agent.${alignTo}`, alignTo),
                  })}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setAlignOpen(false)}>
                {t("action.cancel", "取消")}
              </button>
              <button
                className="btn btn-primary"
                style={{ fontSize: 13 }}
                disabled={alignFrom === alignTo}
                onClick={handleAlign}
              >
                {t("skills.alignTitle", "对齐配置")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}
    </div>
  );
}
