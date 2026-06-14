// ─── Skills 管理页 ──────────────────────────────────────────
// 顶层侧栏入口。统一已装列表（一条/skill，不分 agent）。
// 每行右侧展示 claude/codex 图标：在 enabled_agents 内=启用样式，否则=未启用样式，可点切换。
// 所有操作（list/enable/disable/update）全走后端 npx skills（无手动 fs）。
//
// scope 默认 Global（用户级全局 -g），可选 Project（选某项目目录）。
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
  const [scopeKind, setScopeKind] = useState<"global" | "project">("global");
  const [projectPath, setProjectPath] = useState("");

  const [installed, setInstalled] = useState<SkillInfo[]>([]);
  const [installedLoading, setInstalledLoading] = useState(false);

  // 切换中标识："<name>::<agent>" 或 "__update__"；非 null 时禁并发。
  const [busyKey, setBusyKey] = useState<string | null>(null);
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

  // 列已装（scope 变化时刷新）。
  // 不先清空列表：保留旧数据直到新数据到，避免整页闪烁；仅首屏（空列表）显示 loading。
  const loadInstalled = useCallback(async () => {
    if (scopeInvalid) {
      setInstalled([]);
      return;
    }
    setInstalled((prev) => {
      if (prev.length === 0) setInstalledLoading(true);
      return prev;
    });
    try {
      const list = await skillsApi.listInstalled(scope);
      setInstalled(list);
    } catch (e) {
      console.error("list installed failed", e);
    } finally {
      setInstalledLoading(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKind, projectPath]);

  useEffect(() => {
    loadInstalled();
  }, [loadInstalled]);

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
      await loadInstalled();
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

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>{t("skills.title", "Skills")}</h2>
        <button
          className="btn btn-ghost"
          style={{ fontSize: 12 }}
          disabled={!writeReady || scopeInvalid || busyKey !== null}
          onClick={handleUpdate}
        >
          {busyKey === "__update__" ? t("skills.updating", "更新中…") : t("skills.updateAll", "更新全部")}
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

      {/* scope 筛选 */}
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

      {/* 总计统计卡（醒目大数字 + 每 agent 启用数） */}
      <div
        className="glass-elevated"
        style={{
          padding: "20px 24px",
          display: "flex",
          alignItems: "center",
          gap: 28,
          flexWrap: "wrap",
        }}
      >
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
            </div>
          ))}
        </div>
      </div>

      {/* 已装列表（统一一条/skill） */}
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
                key={skill.name}
                className="glass-surface"
                style={{ padding: "12px 16px", display: "flex", gap: 12, alignItems: "center" }}
              >
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, fontWeight: 600 }}>{skill.name}</div>
                  {skill.description && (
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
                          border: enabled ? "1.5px solid var(--accent)" : "1px solid var(--border, rgba(255,255,255,0.12))",
                          background: enabled ? "var(--accent-soft, rgba(255,255,255,0.10))" : "transparent",
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
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
