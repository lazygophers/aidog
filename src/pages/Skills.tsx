// ─── Skills 管理页 ──────────────────────────────────────────
// 顶层侧栏入口。GUI 内列已装 / 卸载 / 更新 agent skills。
// 读操作走原生（skillsApi.listInstalled/checkEnv），写操作 shell out npx skills（remove/update）。
//
// scope 默认 Global（用户级全局 -g），可选 Project（选某项目目录）。
// agent 默认 Claude，可切 Codex（SVG 图标行切换，激活/未激活态可视区分，兼做统计）。
// npx/node 缺失 → 顶部提示条引导装 node，不阻塞整页。

import { useState, useEffect, useCallback } from "react";
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
import claudeIcon from "../assets/platforms/claude_code.svg";
import codexIcon from "../assets/platforms/openai.svg";

const AGENTS: SkillAgent[] = ["claude", "codex"];
const AGENT_ICONS: Record<SkillAgent, string> = { claude: claudeIcon, codex: codexIcon };

export function Skills() {
  const { t } = useTranslation();

  const [env, setEnv] = useState<SkillsEnv | null>(null);
  const [agent, setAgent] = useState<SkillAgent>("claude");
  const [scopeKind, setScopeKind] = useState<"global" | "project">("global");
  const [projectPath, setProjectPath] = useState("");

  const [installed, setInstalled] = useState<SkillInfo[]>([]);
  const [installedLoading, setInstalledLoading] = useState(false);

  // 每 agent 已装数（当前 scope）。
  const [counts, setCounts] = useState<Record<SkillAgent, number>>({ claude: 0, codex: 0 });

  const [busyId, setBusyId] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  // 当前 scope 对象（供 API 调用）。
  const scope: SkillScope =
    scopeKind === "project"
      ? { kind: "project", path: projectPath }
      : { kind: "global" };

  const writeReady = !!env?.npx_available;
  const scopeInvalid = scopeKind === "project" && projectPath.trim() === "";

  // 环境探测（进页一次）。
  useEffect(() => {
    skillsApi.checkEnv().then(setEnv).catch((e) => console.error("check env failed", e));
  }, []);

  // 列已装（scope/agent 变化时刷新）。
  const loadInstalled = useCallback(async () => {
    if (scopeInvalid) {
      setInstalled([]);
      return;
    }
    setInstalledLoading(true);
    try {
      const list = await skillsApi.listInstalled(scope, agent);
      setInstalled(list);
    } catch (e) {
      console.error("list installed failed", e);
    } finally {
      setInstalledLoading(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKind, projectPath, agent]);

  useEffect(() => {
    loadInstalled();
  }, [loadInstalled]);

  // 统计：当前 scope 下每 agent 已装数（claude + codex 并行）。
  const loadCounts = useCallback(async () => {
    if (scopeInvalid) {
      setCounts({ claude: 0, codex: 0 });
      return;
    }
    try {
      const [claudeList, codexList] = await Promise.all([
        skillsApi.listInstalled(scope, "claude"),
        skillsApi.listInstalled(scope, "codex"),
      ]);
      setCounts({ claude: claudeList.length, codex: codexList.length });
    } catch (e) {
      console.error("load counts failed", e);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKind, projectPath]);

  useEffect(() => {
    loadCounts();
  }, [loadCounts]);

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
      await loadInstalled();
      await loadCounts();
    } else {
      const err = res.stderr.trim() || res.stdout.trim() || t("skills.opFailed", "操作失败");
      setMessage(err);
    }
  };

  const handleRemove = async (name: string) => {
    if (scopeInvalid) return;
    setBusyId(name);
    setMessage(null);
    try {
      const res = await skillsApi.remove(name, agent, scope);
      await applyResult(res, "skills.removed");
    } catch (e) {
      console.error("remove failed", e);
      setMessage(String(e));
    } finally {
      setBusyId(null);
    }
  };

  const handleUpdate = async () => {
    if (!writeReady || scopeInvalid) return;
    setBusyId("__update__");
    setMessage(null);
    try {
      const res = await skillsApi.update(agent, scope);
      await applyResult(res, "skills.updated");
    } catch (e) {
      console.error("update failed", e);
      setMessage(String(e));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>{t("skills.title", "Skills")}</h2>
        <button
          className="btn btn-ghost"
          style={{ fontSize: 12 }}
          disabled={!writeReady || scopeInvalid || busyId !== null}
          onClick={handleUpdate}
        >
          {busyId === "__update__" ? t("skills.updating", "更新中…") : t("skills.updateAll", "更新全部")}
        </button>
      </div>

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

      {/* 操作结果消息 */}
      {message && (
        <div
          className="glass-surface"
          style={{ padding: "10px 14px", fontSize: 12, whiteSpace: "pre-wrap", wordBreak: "break-word" }}
        >
          {message}
        </div>
      )}

      {/* scope 筛选（仅 scope） */}
      <div className="glass-surface" style={{ padding: 16, display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", gap: 16, flexWrap: "wrap", alignItems: "center" }}>
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
        </div>

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

      {/* agent 图标 + 统计行（统计/切换区，独立于筛选） */}
      <div
        className="glass-surface"
        style={{ padding: 16, display: "flex", gap: 16, flexWrap: "wrap", alignItems: "center" }}
      >
        <div style={{ fontSize: 13, fontWeight: 600 }}>
          {t("skills.total", "已安装总计")}
          <span style={{ marginInlineStart: 6, color: "var(--accent)", fontWeight: 700 }}>
            {counts.claude + counts.codex}
          </span>
        </div>
        <div style={{ display: "flex", gap: 10 }}>
          {AGENTS.map((a) => {
            const active = agent === a;
            const label = t(`skills.agent.${a}`, a);
            return (
              <button
                key={a}
                type="button"
                className="glass"
                title={label}
                aria-label={label}
                aria-pressed={active}
                onClick={() => setAgent(a)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "6px 12px",
                  cursor: "pointer",
                  borderRadius: 10,
                  border: active ? "1.5px solid var(--accent)" : "1px solid transparent",
                  background: active ? "var(--accent-soft, rgba(255,255,255,0.08))" : "transparent",
                  opacity: active ? 1 : 0.5,
                  transition: "opacity 0.15s, border-color 0.15s, background 0.15s",
                }}
              >
                <img src={AGENT_ICONS[a]} alt={label} style={{ width: 20, height: 20 }} />
                <span style={{ fontSize: 13, fontWeight: 600 }}>{counts[a]}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* 已装列表 */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <h3 style={{ fontSize: 14, fontWeight: 700, margin: 0 }}>{t("skills.installedTitle", "已安装")}</h3>
        {installedLoading ? (
          <div className="text-secondary" style={{ padding: 12 }}>{t("status.loading", "加载中…")}</div>
        ) : installed.length === 0 ? (
          <div className="glass-surface text-secondary" style={{ padding: "24px 16px", textAlign: "center", fontSize: 13 }}>
            {t("skills.installedEmpty", "当前范围下暂无已安装 skills")}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {installed.map((skill) => (
              <div
                key={skill.installed_path}
                className="glass-surface"
                style={{ padding: "12px 16px", display: "flex", gap: 12, alignItems: "flex-start" }}
              >
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, fontWeight: 600 }}>{skill.name}</div>
                  {skill.description && (
                    <div style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>{skill.description}</div>
                  )}
                  <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4, wordBreak: "break-all" }}>
                    {skill.installed_path}
                  </div>
                </div>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 11, padding: "4px 10px", whiteSpace: "nowrap" }}
                  disabled={busyId !== null}
                  onClick={() => handleRemove(skill.name)}
                >
                  {busyId === skill.name ? t("skills.removing", "卸载中…") : t("skills.remove", "卸载")}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
