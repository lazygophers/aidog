// ─── Skill 详情视图（只读浏览）──────────────────────────────
// 点击已装 skill 名触发（modal）。左文件树 + 右内容查看器。
// SKILL.md / *.md 走 react-markdown 渲染；其他文本 <pre>；二进制提示。
// 全程只读，无编辑入口。

import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  skillsApi,
  type SkillInfo,
  type SkillFile,
  type SkillFileContent,
} from "../services/api";

interface Props {
  skill: SkillInfo;
  onClose: () => void;
}

function formatSize(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export function SkillDetailView({ skill, onClose }: Props) {
  const { t } = useTranslation();
  const [files, setFiles] = useState<SkillFile[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState<SkillFileContent | null>(null);
  const [loadingList, setLoadingList] = useState(true);
  const [loadingFile, setLoadingFile] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const path = skill.installed_path;

  // 加载文件树，默认选 SKILL.md。
  useEffect(() => {
    if (!path) {
      setError(t("skills.detail.loadFailed", { defaultValue: "加载失败" }));
      setLoadingList(false);
      return;
    }
    let cancelled = false;
    (async () => {
      setLoadingList(true);
      setError(null);
      try {
        const d = await skillsApi.detail(path);
        if (cancelled) return;
        setFiles(d.files);
        const skillMd = d.files.find((f) => f.rel_path === "SKILL.md");
        setSelected(skillMd ? "SKILL.md" : d.files[0]?.rel_path ?? null);
      } catch (e: any) {
        if (!cancelled) setError(e?.toString?.() ?? String(e));
      } finally {
        if (!cancelled) setLoadingList(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [path, t]);

  // 选中文件变化 → 读内容。
  const loadFile = useCallback(
    async (rel: string) => {
      if (!path) return;
      setLoadingFile(true);
      setContent(null);
      try {
        const c = await skillsApi.readFile(path, rel);
        setContent(c);
      } catch (e: any) {
        setError(e?.toString?.() ?? String(e));
      } finally {
        setLoadingFile(false);
      }
    },
    [path]
  );

  useEffect(() => {
    if (selected) loadFile(selected);
  }, [selected, loadFile]);

  const isMd = (rel: string) => /\.md$/i.test(rel);
  const selectedFile = files.find((f) => f.rel_path === selected);

  return createPortal(
    <div
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 250,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "rgba(0,0,0,0.45)",
        animation: "fadeIn 150ms ease both",
        padding: 24,
      }}
    >
      <div
        className="glass-elevated"
        onClick={(e) => e.stopPropagation()}
        style={{
          width: "min(95vw, 1100px)",
          height: "min(88vh, 760px)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            padding: "12px 18px",
            borderBottom: "1px solid var(--border)",
            flexShrink: 0,
          }}
        >
          <div style={{ display: "flex", flexDirection: "column", gap: 2, minWidth: 0 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              <span style={{ fontSize: 15, fontWeight: 700 }}>{skill.name}</span>
              {skill.source && (
                <span
                  style={{
                    fontSize: 11,
                    padding: "1px 6px",
                    borderRadius: 4,
                    background: "var(--accent-subtle, rgba(0,0,0,0.06))",
                    color: "var(--text-secondary)",
                  }}
                >
                  {skill.source}
                </span>
              )}
              {skill.enabled_agents.map((a) => (
                <span
                  key={a}
                  style={{
                    fontSize: 10,
                    padding: "1px 5px",
                    borderRadius: 3,
                    border: "1px solid var(--border)",
                    color: "var(--text-secondary)",
                  }}
                >
                  {a}
                </span>
              ))}
            </div>
            {path && (
              <span
                style={{
                  fontSize: 10,
                  color: "var(--text-secondary)",
                  fontFamily: "var(--font-mono, monospace)",
                  wordBreak: "break-all",
                }}
              >
                {path}
              </span>
            )}
          </div>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12, flexShrink: 0 }}
            onClick={onClose}
            aria-label="close"
          >
            ✕
          </button>
        </div>

        {/* Body: 左文件树 + 右内容 */}
        <div style={{ display: "flex", flex: 1, minHeight: 0 }}>
          {/* 左：文件树 */}
          <div
            style={{
              width: 220,
              flexShrink: 0,
              borderRight: "1px solid var(--border)",
              overflowY: "auto",
              padding: "8px 0",
            }}
          >
            <div
              style={{
                fontSize: 10,
                fontWeight: 600,
                color: "var(--text-secondary)",
                padding: "0 14px 6px",
                textTransform: "uppercase",
              }}
            >
              {t("skills.detail.files", { defaultValue: "文件" })} ({files.length})
            </div>
            {loadingList && (
              <div style={{ padding: "8px 14px", fontSize: 12, color: "var(--text-secondary)" }}>
                …
              </div>
            )}
            {!loadingList && files.length === 0 && (
              <div style={{ padding: "8px 14px", fontSize: 12, color: "var(--text-secondary)" }}>
                {t("skills.detail.empty", { defaultValue: "空" })}
              </div>
            )}
            {files.map((f) => {
              const on = selected === f.rel_path;
              return (
                <button
                  key={f.rel_path}
                  onClick={() => setSelected(f.rel_path)}
                  title={`${f.rel_path} (${formatSize(f.size)})`}
                  style={{
                    display: "block",
                    width: "100%",
                    textAlign: "left",
                    padding: "5px 14px",
                    fontSize: 12,
                    background: on ? "var(--accent-subtle, rgba(0,0,0,0.06))" : "transparent",
                    borderLeft: on ? "2px solid var(--accent, #5b8def)" : "2px solid transparent",
                    color: on ? "var(--accent, #5b8def)" : "var(--text)",
                    fontFamily: "var(--font-mono, monospace)",
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    cursor: "pointer",
                  }}
                >
                  {!f.is_text && "📄 "}
                  {f.rel_path}
                </button>
              );
            })}
          </div>

          {/* 右：内容 */}
          <div style={{ flex: 1, overflow: "auto", padding: "16px 20px", minWidth: 0 }}>
            {error && (
              <div style={{ fontSize: 13, color: "var(--danger, #e06c75)" }}>
                {t("skills.detail.readFailed", { defaultValue: "读取失败" })}: {error}
              </div>
            )}
            {!error && loadingFile && (
              <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>…</div>
            )}
            {!error && !loadingFile && content && selectedFile && (
              <>
                <div
                  style={{
                    fontSize: 11,
                    color: "var(--text-secondary)",
                    marginBottom: 10,
                    display: "flex",
                    gap: 12,
                    fontFamily: "var(--font-mono, monospace)",
                  }}
                >
                  <span>{selectedFile.rel_path}</span>
                  <span>{formatSize(selectedFile.size)}</span>
                </div>
                {content.content === null ? (
                  <div
                    style={{
                      fontSize: 13,
                      color: "var(--text-secondary)",
                      fontStyle: "italic",
                    }}
                  >
                    {t("skills.detail.binary", {
                      defaultValue: "二进制文件，无法预览",
                    })}
                  </div>
                ) : content.truncated ? (
                  <div
                    style={{
                      fontSize: 11,
                      color: "var(--text-secondary)",
                      marginBottom: 8,
                    }}
                  >
                    {t("skills.detail.truncated", { defaultValue: "文件过大，已截断" })}
                  </div>
                ) : null}
                {content.content !== null &&
                  (isMd(selectedFile.rel_path) ? (
                    <div className="markdown-body" style={{ fontSize: 13, lineHeight: 1.6 }}>
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>
                        {content.content}
                      </ReactMarkdown>
                    </div>
                  ) : (
                    <pre
                      style={{
                        fontSize: 12,
                        fontFamily: "var(--font-mono, monospace)",
                        whiteSpace: "pre-wrap",
                        wordBreak: "break-word",
                        margin: 0,
                        color: "var(--text)",
                      }}
                    >
                      {content.content}
                    </pre>
                  ))}
              </>
            )}
            {!error && !loadingFile && !content && !selected && (
              <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>
                {t("skills.detail.noSkillMd", { defaultValue: "无 SKILL.md" })}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>,
    document.body
  );
}
