// ─── 关于页 ──────────────────────────────────────────────
// 展示完整版本信息（App / Tauri / OS / 架构 / 构建）+ GitHub 静态链接。
// 版本数据单一事实源 = 后端 about_info（tauri.conf.json + build.rs 注入），禁前端硬编码。

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Update } from "@tauri-apps/plugin-updater";
import { aboutApi, cliEnvApi, type AboutInfo, type CliToolStatus, type CliConflict } from "../services/api";
import { checkForUpdateManual } from "../services/updater";
import { UpdatePromptModal } from "../components/UpdatePromptModal";
import { IconGlobe } from "../components/icons";
import { formatDateTime } from "../utils/formatters";

type UpdateState = "idle" | "checking" | "uptodate" | "error";

const GITHUB_REPO = "https://github.com/lazygophers/aidog";
const GITHUB_LINKS = {
  repo: GITHUB_REPO,
  releases: `${GITHUB_REPO}/releases`,
  issues: `${GITHUB_REPO}/issues`,
  reportIssue: `${GITHUB_REPO}/issues/new`,
};

export function About() {
  const { t } = useTranslation();
  const [info, setInfo] = useState<AboutInfo | null>(null);
  const [updState, setUpdState] = useState<UpdateState>("idle");
  const [updMsg, setUpdMsg] = useState("");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);

  // ─── 本地环境（Claude Code / Codex CLI）──────────────────────
  const [cliTools, setCliTools] = useState<CliToolStatus[]>([]);
  const [cliConflicts, setCliConflicts] = useState<CliConflict[]>([]);
  const [cliBusy, setCliBusy] = useState<"" | "check" | "install" | "upgrade" | "diagnose">("");
  const [cliMsg, setCliMsg] = useState("");
  const [cliErr, setCliErr] = useState("");
  const [cliPendingTool, setCliPendingTool] = useState<"claude" | "codex" | null>(null);

  useEffect(() => {
    aboutApi.info().then(setInfo).catch(() => setInfo(null));
    // 进入页面自动检查一次（纯手动按钮场景下也提供首屏数据，不联网拉 latest）。
    handleCliCheck();
  }, []);

  // 手动检查更新：忽略节流强制 check → 有则弹提醒 modal (立即更新/稍后)；
  // 无则提示已是最新；失败提示 (手动检查可见错误，区分静默的自动检查)。
  const handleCheckUpdate = async () => {
    setUpdState("checking");
    setUpdMsg("");
    setPendingUpdate(null);
    try {
      const upd = await checkForUpdateManual();
      if (upd) {
        setUpdState("idle");
        setPendingUpdate(upd);
      } else {
        setUpdState("uptodate");
      }
    } catch (e) {
      setUpdState("error");
      setUpdMsg(String(e));
    }
  };

  const updBusy = updState === "checking";
  const updStatusText = (() => {
    switch (updState) {
      case "checking":
        return t("about.checking", "检查中…");
      case "uptodate":
        return t("about.upToDate", "已是最新版本");
      case "error":
        return `${t("about.updateError", "检查更新失败")}: ${updMsg}`;
      default:
        return "";
    }
  })();

  const buildTimeText = (() => {
    if (!info?.build_time) return "—";
    const secs = Number(info.build_time);
    if (!Number.isFinite(secs) || secs <= 0) return info.build_time;
    return formatDateTime(secs * 1000) || info.build_time;
  })();

  // ─── 本地环境：检查版本 / 安装 / 升级 / 诊断冲突 ───────────────
  // 复用 mitm importDefaults 模式：loading 态 + toast 反馈。

  const handleCliCheck = async () => {
    setCliBusy("check");
    setCliMsg("");
    setCliErr("");
    try {
      const list = await cliEnvApi.checkVersions();
      setCliTools(list);
      // 同时检查更新可用性
      cliEnvApi.checkUpdates().then((updates) => {
        setCliTools(updates);
      }).catch((e) => {
        // 更新检测失败静默忽略，不影响主流程
        console.warn("checkUpdates failed:", e);
      });
    } catch (e) {
      setCliErr(`${t("about.localEnv.checkFailed", "检查失败")}: ${e}`);
    } finally {
      setCliBusy("");
    }
  };

  const handleCliInstall = async (tool: "claude" | "codex") => {
    setCliBusy("install");
    setCliPendingTool(tool);
    setCliMsg("");
    setCliErr("");
    try {
      await cliEnvApi.install(tool);
      setCliMsg(t("about.localEnv.installSuccess", "{{tool}} 安装成功", { tool }));
      await handleCliCheck();
    } catch (e) {
      setCliErr(`${t("about.localEnv.installFailed", "安装失败")}: ${e}`);
    } finally {
      setCliBusy("");
      setCliPendingTool(null);
    }
  };

  const handleCliUpgrade = async (tool: "claude" | "codex") => {
    setCliBusy("upgrade");
    setCliPendingTool(tool);
    setCliMsg("");
    setCliErr("");
    try {
      await cliEnvApi.upgrade(tool);
      setCliMsg(t("about.localEnv.upgradeSuccess", "{{tool}} 升级成功", { tool }));
      await handleCliCheck();
    } catch (e) {
      setCliErr(`${t("about.localEnv.upgradeFailed", "升级失败")}: ${e}`);
    } finally {
      setCliBusy("");
      setCliPendingTool(null);
    }
  };

  const handleCliDiagnose = async () => {
    setCliBusy("diagnose");
    setCliMsg("");
    setCliErr("");
    try {
      const list = await cliEnvApi.diagnoseConflicts();
      setCliConflicts(list);
      const conflictTools = list.filter((c) => c.is_conflicting);
      if (conflictTools.length === 0) {
        setCliMsg(t("about.localEnv.noConflicts", "无冲突"));
      } else {
        setCliMsg(
          t("about.localEnv.conflictFound", "发现 {{count}} 个工具存在冲突", { count: conflictTools.length })
        );
      }
    } catch (e) {
      setCliErr(`${t("about.localEnv.diagnoseFailed", "诊断失败")}: ${e}`);
    } finally {
      setCliBusy("");
    }
  };

  const cliStatusText = (s: CliToolStatus): { text: string; color: string } => {
    if (!s.installed) {
      return { text: t("about.localEnv.notInstalled", "未安装"), color: "var(--text-tertiary)" };
    }
    if (s.broken) {
      return { text: t("about.localEnv.broken", "已损坏"), color: "var(--color-danger, #ef4444)" };
    }
    if (s.conflict) {
      return { text: t("about.localEnv.conflict", "冲突"), color: "var(--color-warning, #f59e0b)" };
    }
    return { text: t("about.localEnv.installed", "已安装"), color: "var(--color-success, #22c55e)" };
  };

  const conflictFor = (tool: string) => cliConflicts.find((c) => c.tool === tool);

  const rows: { label: string; value: string; mono?: boolean }[] = info
    ? [
        { label: t("about.appVersion", "应用版本"), value: `v${info.app_version}`, mono: true },
        { label: t("about.tauriVersion", "Tauri 版本"), value: `v${info.tauri_version}`, mono: true },
        { label: t("about.os", "操作系统"), value: info.os, mono: true },
        { label: t("about.arch", "架构"), value: info.arch, mono: true },
        { label: t("about.profile", "构建配置"), value: info.profile, mono: true },
        { label: t("about.gitCommit", "Git Commit"), value: info.git_commit, mono: true },
        { label: t("about.buildTime", "构建时间"), value: buildTimeText },
      ]
    : [];

  const linkBtns: { key: keyof typeof GITHUB_LINKS; label: string }[] = [
    { key: "repo", label: t("about.repo", "代码仓库") },
    { key: "releases", label: t("about.releases", "版本发布") },
    { key: "issues", label: t("about.issues", "问题列表") },
    { key: "reportIssue", label: t("about.reportIssue", "反馈问题") },
  ];

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 20,
        width: "100%",
        maxWidth: 720,
        margin: "0 auto",
        boxSizing: "border-box",
        minWidth: 0,
      }}
    >
      <div className="section-header">
        <div style={{ flex: 1 }}>
          <div className="section-title">{t("about.title", "关于")}</div>
          <div className="section-desc">{t("about.subtitle", "版本信息与项目链接")}</div>
        </div>
      </div>

      {/* 版本信息 */}
      <div className="glass-surface" style={{ padding: "8px 20px" }}>
        {rows.length === 0 ? (
          <div style={{ padding: "16px 0", fontSize: 13, color: "var(--text-secondary)" }}>
            {t("status.loading", "加载中…")}
          </div>
        ) : (
          rows.map((r, i) => (
            <div
              key={r.label}
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                padding: "12px 0",
                borderTop: i === 0 ? "none" : "1px solid var(--border)",
                gap: 16,
                minWidth: 0,
              }}
            >
              <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)", flex: "0 0 auto" }}>{r.label}</div>
              <div
                style={{
                  fontSize: 13,
                  color: "var(--text-secondary)",
                  wordBreak: "break-all",
                  textAlign: "end",
                  flex: 1,
                  minWidth: 0,
                  fontFamily: r.mono
                    ? "ui-monospace, SFMono-Regular, Menlo, monospace"
                    : undefined,
                }}
              >
                {r.value}
              </div>
            </div>
          ))
        )}
      </div>

      {/* 检查更新 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("about.updateTitle", "软件更新")}
          </div>
          {updStatusText && (
            <div style={{ fontSize: 12, color: updState === "error" ? "var(--color-danger)" : "var(--text-secondary)", wordBreak: "break-all" }}>
              {updStatusText}
            </div>
          )}
        </div>
        <button
          className="btn btn-primary"
          style={{ fontSize: 13, padding: "6px 14px" }}
          disabled={updBusy}
          onClick={handleCheckUpdate}
        >
          {updBusy ? t("about.checking", "检查中…") : t("about.checkUpdate", "检查更新")}
        </button>
      </div>

      {/* GitHub 链接 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
          {t("about.githubTitle", "GitHub")}
        </div>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
          {linkBtns.map((b) => (
            <button
              key={b.key}
              className="btn"
              style={{ display: "inline-flex", alignItems: "center", gap: 6, fontSize: 13 }}
              onClick={() => openUrl(GITHUB_LINKS[b.key])}
            >
              <IconGlobe size={14} />
              {b.label}
            </button>
          ))}
        </div>
        <div style={{ fontSize: 12, color: "var(--text-tertiary)", fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace" }}>
          {GITHUB_REPO}
        </div>
      </div>

      {/* 本地环境：Claude Code / Codex CLI */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
              {t("about.localEnv.title", "本地环境")}
            </div>
            <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("about.localEnv.subtitle", "Claude Code / Codex CLI 安装状态")}
            </div>
          </div>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button
              className="btn"
              style={{ fontSize: 13, padding: "6px 12px" }}
              disabled={cliBusy !== ""}
              onClick={handleCliCheck}
            >
              {cliBusy === "check" ? t("about.localEnv.checking", "检查中…") : t("about.localEnv.check", "检查版本")}
            </button>
            <button
              className="btn"
              style={{ fontSize: 13, padding: "6px 12px" }}
              disabled={cliBusy !== ""}
              onClick={handleCliDiagnose}
            >
              {cliBusy === "diagnose" ? t("about.localEnv.diagnosing", "诊断中…") : t("about.localEnv.diagnose", "诊断冲突")}
            </button>
          </div>
        </div>

        {cliTools.map((s) => {
          const st = cliStatusText(s);
          const conflict = conflictFor(s.name);
          const isPending = cliPendingTool === s.name;
          return (
            <div
              key={s.name}
              style={{
                padding: "12px 0",
                borderTop: "1px solid var(--border)",
                display: "flex",
                flexDirection: "column",
                gap: 8,
                minWidth: 0,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                  <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                    {s.name === "claude"
                      ? t("about.localEnv.claudeCode", "Claude Code")
                      : t("about.localEnv.codex", "Codex CLI")}
                  </span>
                  <span
                    style={{
                      fontSize: 11,
                      padding: "2px 8px",
                      borderRadius: 4,
                      color: st.color,
                      background: "var(--bg-secondary, rgba(125,125,125,0.08))",
                    }}
                  >
                    {st.text}
                  </span>
                </div>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                  {!s.installed && (
                    <button
                      className="btn btn-primary"
                      style={{ fontSize: 12, padding: "4px 10px" }}
                      disabled={cliBusy !== "" || isPending}
                      onClick={() => handleCliInstall(s.name as "claude" | "codex")}
                    >
                      {cliBusy === "install" && isPending
                        ? t("about.localEnv.installing", "安装中…")
                        : t("about.localEnv.install", "安装")}
                    </button>
                  )}
                  {s.installed && !s.broken && s.has_update === true && (
                    <button
                      className="btn"
                      style={{ fontSize: 12, padding: "4px 10px" }}
                      disabled={cliBusy !== "" || isPending}
                      onClick={() => handleCliUpgrade(s.name as "claude" | "codex")}
                    >
                      {cliBusy === "upgrade" && isPending
                        ? t("about.localEnv.upgrading", "升级中…")
                        : t("about.localEnv.upgrade", "升级")}
                    </button>
                  )}
                  {s.broken && (
                    <button
                      className="btn btn-primary"
                      style={{ fontSize: 12, padding: "4px 10px" }}
                      disabled={cliBusy !== "" || isPending}
                      onClick={() => handleCliUpgrade(s.name as "claude" | "codex")}
                      title={t("about.localEnv.brokenHint", "已损坏，尝试重新安装以修复")}
                    >
                      {cliBusy === "upgrade" && isPending
                        ? t("about.localEnv.upgrading", "升级中…")
                        : t("about.localEnv.repair", "修复")}
                    </button>
                  )}
                </div>
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 2, fontSize: 12, color: "var(--text-secondary)" }}>
                <div>
                  <span style={{ color: "var(--text-tertiary)" }}>{t("about.localEnv.version", "版本")}：</span>
                  <span style={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace" }}>
                    {s.version || (s.installed ? t("about.localEnv.unknown", "未知") : "—")}
                  </span>
                  {s.latest_version && (
                    <span style={{ marginLeft: 8, color: "var(--color-success, #22c55e)" }}>
                      {t("about.localEnv.latest", "最新 {{version}}", { version: s.latest_version })}
                    </span>
                  )}
                </div>
                <div style={{ wordBreak: "break-all" }}>
                  <span style={{ color: "var(--text-tertiary)" }}>{t("about.localEnv.path", "路径")}：</span>
                  <span style={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace" }}>
                    {s.path || "—"}
                  </span>
                </div>
              </div>

              {/* 冲突诊断结果 */}
              {conflict && conflict.installations.length > 0 && (
                <div
                  style={{
                    marginTop: 4,
                    padding: 10,
                    borderRadius: 8,
                    border: conflict.is_conflicting
                      ? "1px solid var(--color-warning, #f59e0b)"
                      : "1px solid var(--border)",
                    background: conflict.is_conflicting
                      ? "rgba(245, 158, 11, 0.08)"
                      : "var(--bg-secondary, rgba(125,125,125,0.04))",
                    display: "flex",
                    flexDirection: "column",
                    gap: 6,
                    minWidth: 0,
                    overflowX: "auto",
                  }}
                >
                  <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
                    {t("about.localEnv.installations", "共 {{count}} 处安装", { count: conflict.installations.length })}
                    {conflict.is_conflicting && (
                      <span style={{ color: "var(--color-warning, #f59e0b)", marginLeft: 6 }}>
                        {t("about.localEnv.conflict", "冲突")}
                      </span>
                    )}
                  </div>
                  {conflict.installations.map((inst, idx) => (
                    <div
                      key={idx}
                      style={{
                        display: "flex",
                        gap: 8,
                        alignItems: "center",
                        flexWrap: "wrap",
                        fontSize: 11,
                        color: "var(--text-secondary)",
                      }}
                    >
                      <span
                        style={{
                          fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                          wordBreak: "break-all",
                        }}
                      >
                        {inst.path}
                      </span>
                      <span
                        style={{
                          padding: "1px 6px",
                          borderRadius: 3,
                          background: "var(--bg-tertiary, rgba(125,125,125,0.12))",
                          color: "var(--text-secondary)",
                        }}
                      >
                        {inst.source}
                      </span>
                      {inst.version && (
                        <span style={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace" }}>
                          v{inst.version}
                        </span>
                      )}
                      {!inst.runnable && (
                        <span style={{ color: "var(--color-danger, #ef4444)" }}>
                          {t("about.localEnv.broken", "已损坏")}
                        </span>
                      )}
                      {inst.is_path_default && (
                        <span style={{ color: "var(--color-success, #22c55e)" }}>
                          {t("about.localEnv.pathDefault", "PATH 默认")}
                        </span>
                      )}
                    </div>
                  ))}
                  {conflict.suggestion && (
                    <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>
                      <span style={{ color: "var(--text-tertiary)" }}>{t("about.localEnv.suggestion", "建议")}：</span>
                      {conflict.suggestion}
                    </div>
                  )}
                </div>
              )}
            </div>
          );
        })}

        {cliMsg && (
          <div className="toast" style={{ fontSize: 12, wordBreak: "break-all", color: "var(--color-success, #22c55e)" }}>
            {cliMsg}
          </div>
        )}
        {cliErr && (
          <div className="toast" style={{ fontSize: 12, wordBreak: "break-all", color: "var(--color-danger, #ef4444)" }}>
            {cliErr}
          </div>
        )}
      </div>

      {pendingUpdate && (
        <UpdatePromptModal
          update={pendingUpdate}
          onClose={() => setPendingUpdate(null)}
        />
      )}
    </div>
  );
}
