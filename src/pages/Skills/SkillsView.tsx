import { createPortal } from "react-dom";
import { SkillInstallView } from "../SkillInstallView";
import { formatDateTime, formatRelativeTime } from "../../utils/formatters";
import { AGENTS, AGENT_ICONS } from "./constants";
import { skillCatalogId } from "./share";
import type { SkillsData } from "./useSkillsData";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

/**
 * 主列表视图（自原 Skills.tsx L545-969 外迁）。
 * Header / 环境提示条 / 操作结果消息 portal / 统计卡 + scope 筛选 / 搜索框 / 已装列表 / install 子视图。
 */
export function SkillsView({ s }: { s: SkillsData }) {
  const {
    t, env, scopeKind, setScopeKind, projectPath, setProjectPath, scope, writeReady, scopeInvalid, pickProjectDir,
    subView, setSubView, installed, installedLoading, refreshing, refreshInstalled, filteredInstalled,
    searchQuery, setSearchQuery, total, agentCounts, busyKey, message, setMessage,
    handleToggle, handleUpdate, handleEnableAll, setConfirmUninstall, setAlignOpen,
    setUninstallTarget, setPasteOpen, setPasteText, setDetailTarget, handleShare,
  } = s;

  return (
    <>
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
          <Button
            style={{ fontSize: 12 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null}
            onClick={() => setSubView("install")}
            title={t("skills.install.addBtn", "添加 Skills")}
          >
            {t("skills.install.addBtn", "+ 添加")}
          </Button>
          <Button
            variant="outline"
            style={{ fontSize: 12 }}
            disabled={busyKey !== null}
            onClick={() => { setPasteText(""); setMessage(null); setPasteOpen(true); }}
            title={t("skills.importFromShare", "从分享导入")}
          >
            {t("skills.importFromShare", "从分享导入")}
          </Button>
          <Button
            variant="outline"
            style={{ fontSize: 12 }}
            disabled={scopeInvalid || busyKey !== null || refreshing}
            onClick={refreshInstalled}
            title={t("skills.refresh", "刷新")}
          >
            {refreshing ? t("skills.refreshing", "刷新中…") : t("skills.refresh", "刷新")}
          </Button>
          <Button
            variant="outline"
            style={{ fontSize: 12 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null}
            onClick={handleUpdate}
          >
            {busyKey === "__update__" ? t("skills.updating", "更新中…") : t("skills.updateAll", "更新全部")}
          </Button>
          <Button
            variant="destructive"
            style={{ fontSize: 12 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0}
            onClick={() => setConfirmUninstall(true)}
          >
            {busyKey === "__uninstall__" ? t("skills.uninstalling", "卸载中…") : t("skills.uninstallAll", "卸载全部")}
          </Button>
          <Button
            variant="outline"
            style={{ fontSize: 12 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0}
            onClick={() => setAlignOpen(true)}
          >
            {busyKey === "__align__" ? t("skills.aligning", "对齐中…") : t("skills.alignTitle", "对齐配置")}
          </Button>
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
            <Button
              type="button"
              variant="ghost"
              onClick={() => setMessage(null)}
              aria-label={t("action.dismiss", "关闭")}
              style={{
                background: "transparent",
                color: "var(--text-secondary)",
                fontSize: 14,
                padding: 0,
                lineHeight: 1,
                height: "auto",
              }}
            >
              ✕
            </Button>
          </div>
        </div>,
        document.body,
      )}

      {/* 统计 + scope 筛选 (合并单卡: 左统计 右筛选右对齐) */}
      <Card
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
                <Button
                  variant="outline"
                  style={{ fontSize: 11, padding: "3px 8px" }}
                  disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0 || agentCounts[a] === installed.length}
                  onClick={() => handleEnableAll(a)}
                  title={t("skills.enableAll", "全部启用")}
                >
                  {busyKey === `__enableall_${a}__` ? t("skills.enabling", "启用中…") : t("skills.enableAll", "全部启用")}
                </Button>
              </div>
            ))}
          </div>
        </div>

        {/* 右: scope 筛选 (右对齐) */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8, alignItems: "flex-end" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
            <span className="text-secondary">{t("skills.scope", "范围")}</span>
            <Select value={scopeKind} onValueChange={(v) => setScopeKind(v as "global" | "project")}>
              <SelectTrigger style={{ width: "auto", fontSize: 13 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="global">{t("skills.scopeGlobal", "用户级（全局）")}</SelectItem>
                <SelectItem value="project">{t("skills.scopeProject", "项目级")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          {scopeKind === "project" && (
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <Input
                style={{ flex: 1 }}
                placeholder={t("skills.projectPathPlaceholder", "项目目录绝对路径")}
                value={projectPath}
                onChange={(e) => setProjectPath(e.target.value)}
              />
              <Button variant="outline" style={{ fontSize: 12 }} onClick={pickProjectDir}>
                {t("skills.browse", "浏览…")}
              </Button>
            </div>
          )}
        </div>
      </Card>

      {/* 搜索框（仅有已装 skills 时显示，照 Platforms/Groups 搜索框样式） */}
      {!installedLoading && installed.length > 0 && (
        <Input
          placeholder={t("skills.searchPlaceholder", "搜索 skills...")}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          style={{ fontSize: 13 }}
        />
      )}

      {/* 已装列表（统一一条/skill） */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {installedLoading ? (
          <div className="text-secondary" style={{ padding: 12 }}>{t("status.loading", "加载中…")}</div>
        ) : installed.length === 0 ? (
          <div className="glass-surface text-secondary" style={{ padding: "24px 16px", textAlign: "center", fontSize: 13 }}>
            {t("skills.installedEmpty", "当前范围下暂无已安装 skills")}
          </div>
        ) : filteredInstalled.length === 0 ? (
          <div className="glass-surface text-secondary" style={{ padding: "24px 16px", textAlign: "center", fontSize: 13 }}>
            {t("skills.searchEmpty", "没有匹配的 skills")}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {filteredInstalled.map((skill) => (
              <Card
                key={skill.name}
                className="glass-surface"
                style={{ padding: "12px 16px", display: "flex", gap: 12, alignItems: "center" }}
              >
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                    <Button
                      type="button"
                      variant="ghost"
                      title={t("skills.detail.view", "查看详情")}
                      style={{ fontSize: 13, fontWeight: 600, padding: 0, height: "auto" }}
                      onClick={() => setDetailTarget(skill)}
                    >
                      {skill.name}
                    </Button>
                    {/* 锁文件元数据标签：source / sourceType / plugin / updatedAt 相对时间 */}
                    {skill.source && (
                      <a
                        href={skill.source_url ?? `https://github.com/${skill.source}`}
                        target="_blank"
                        rel="noreferrer"
                        onClick={(e) => {
                          // Tauri 外链走 opener 由系统处理（普通 a target=_blank 也工作，这里防误关页面）。
                          e.preventDefault();
                          window.open(skill.source_url ?? `https://github.com/${skill.source}`, "_blank");
                        }}
                        title={skill.source_url ?? skill.source}
                        style={{
                          fontSize: 11,
                          padding: "2px 8px",
                          borderRadius: 6,
                          background: "var(--accent-subtle)",
                          color: "var(--accent)",
                          textDecoration: "none",
                          cursor: "pointer",
                          border: "1px solid var(--border)",
                        }}
                      >
                        {skill.source}
                      </a>
                    )}
                    {skill.source_type && (
                      <Badge
                        variant="outline"
                        title={t("skills.sourceType", "来源类型")}
                        style={{
                          fontSize: 10,
                          padding: "2px 6px",
                          background: "var(--bg-floating)",
                          color: "var(--text-secondary)",
                          textTransform: "uppercase",
                          letterSpacing: 0.3,
                        }}
                      >
                        {skill.source_type}
                      </Badge>
                    )}
                    {skill.plugin_name && (
                      <Badge
                        variant="outline"
                        title={t("skills.pluginName", "plugin 来源")}
                        style={{
                          fontSize: 10,
                          padding: "2px 6px",
                          background: "var(--bg-floating)",
                          color: "var(--text-secondary)",
                        }}
                      >
                        plugin: {skill.plugin_name}
                      </Badge>
                    )}
                    {skill.updated_at && (
                      <span
                        title={`${t("skills.updatedAt", "更新时间")}: ${formatDateTime(skill.updated_at) ?? skill.updated_at}`}
                        style={{
                          fontSize: 11,
                          color: "var(--text-secondary)",
                        }}
                      >
                        {formatRelativeTime(skill.updated_at)}
                      </span>
                    )}
                  </div>
                  {skill.description?.trim() && (
                    <div style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>{skill.description}</div>
                  )}
                  {/* 安装时间次要行（紧凑） */}
                  {skill.installed_at && (
                    <div style={{ fontSize: 10, color: "var(--text-secondary)", marginTop: 2, opacity: 0.8 }}>
                      <span>{t("skills.installedAt", "安装于")}: </span>
                      <span title={skill.installed_at}>{formatDateTime(skill.installed_at)}</span>
                      {skill.skill_folder_hash && (
                        <>
                          <span style={{ margin: "0 6px", opacity: 0.5 }}>·</span>
                          <span title={t("skills.hash", "内容 hash")} style={{ fontFamily: "monospace" }}>
                            {skill.skill_folder_hash.slice(0, 7)}
                          </span>
                        </>
                      )}
                    </div>
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
                      <Button
                        key={a}
                        type="button"
                        variant={enabled ? "default" : "outline"}
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
                          height: "auto",
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
                      </Button>
                    );
                  })}
                </div>
                {/* 分享（仅 catalog 来源可分享：source 缺失的手动 symlink skill 隐藏按钮） */}
                {skillCatalogId(skill) && (
                  <Button
                    variant="outline"
                    style={{ fontSize: 11, padding: "4px 10px", flexShrink: 0 }}
                    onClick={() => handleShare(skill)}
                    title={t("skills.share", "分享")}
                  >
                    {t("skills.share", "分享")}
                  </Button>
                )}
                {/* 单条卸载（破坏性，二次确认） */}
                <Button
                  variant="destructive"
                  style={{ fontSize: 11, padding: "4px 10px", flexShrink: 0 }}
                  disabled={!writeReady || busyKey !== null}
                  onClick={() => setUninstallTarget(skill)}
                  title={t("skills.uninstall", "卸载")}
                >
                  {busyKey === `__uninstall_single_${skill.name}__`
                    ? t("skills.uninstalling", "卸载中…")
                    : t("skills.uninstall", "卸载")}
                </Button>
              </Card>
            ))}
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
    </>
  );
}
