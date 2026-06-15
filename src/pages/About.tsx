// ─── 关于页 ──────────────────────────────────────────────
// 展示完整版本信息（App / Tauri / OS / 架构 / 构建）+ GitHub 静态链接。
// 版本数据单一事实源 = 后端 about_info（tauri.conf.json + build.rs 注入），禁前端硬编码。

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Update } from "@tauri-apps/plugin-updater";
import { aboutApi, type AboutInfo } from "../services/api";
import { checkForUpdateManual } from "../services/updater";
import { UpdatePromptModal } from "../components/UpdatePromptModal";
import { IconGlobe } from "../components/icons";

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

  useEffect(() => {
    aboutApi.info().then(setInfo).catch(() => setInfo(null));
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
    return new Date(secs * 1000).toLocaleString();
  })();

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
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720 }}>
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
              }}
            >
              <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>{r.label}</div>
              <div
                style={{
                  fontSize: 13,
                  color: "var(--text-secondary)",
                  wordBreak: "break-all",
                  textAlign: "end",
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

      {pendingUpdate && (
        <UpdatePromptModal
          update={pendingUpdate}
          onClose={() => setPendingUpdate(null)}
        />
      )}
    </div>
  );
}
