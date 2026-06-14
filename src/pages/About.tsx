// ─── 关于页 ──────────────────────────────────────────────
// 展示完整版本信息（App / Tauri / OS / 架构 / 构建）+ GitHub 静态链接。
// 版本数据单一事实源 = 后端 about_info（tauri.conf.json + build.rs 注入），禁前端硬编码。

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { aboutApi, type AboutInfo } from "../services/api";
import { IconGlobe } from "../components/icons";

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

  useEffect(() => {
    aboutApi.info().then(setInfo).catch(() => setInfo(null));
  }, []);

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
    </div>
  );
}
