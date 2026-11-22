import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { SkillInstallView } from "../SkillInstallView";
import { formatDateTime, formatRelativeTime } from "../../utils/formatters";
import { useReveal, makeRipple } from "@/utils/motion";
import { AGENTS, AGENT_ICONS } from "./constants";
import piIcon from "../../assets/platforms/pi.svg";
import { skillCatalogId } from "./share";
import type { SkillsData } from "./useSkillsData";
import type { SkillInfo, SkillAgent } from "../../services/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { PluginOriginBadge, usePluginOrigins } from "@/components/PluginOrigin";

/**
 * 主列表视图（自原 Skills.tsx L545-969 外迁）。
 * Header / 环境提示条 / 操作结果消息 portal / 统计卡 + scope 筛选 / 搜索框 / 已装列表 / install 子视图。
 */
export function SkillsView({ s }: { s: SkillsData }) {
  const {
    t, env, scopeKind, setScopeKind, projectPath, setProjectPath, scope, writeReady, scopeInvalid, pickProjectDir,
    subView, setSubView, installed, installedLoading, refreshing, refreshInstalled, filteredInstalled,
    searchQuery, setSearchQuery, enabledFilter, setEnabledFilter, total, agentCounts, busyKey, message, setMessage,
    handleToggle, handleUpdate, handleEnableAll, setConfirmUninstall, setAlignOpen,
    setUninstallTarget, setPasteOpen, setPasteText, setDetailTarget, handleShare,
    selectedNames, toggleSelected, setConfirmUninstallBatch,
  } = s;
  const origins = usePluginOrigins("skills");

  return (
    <>
      {/* 批量卸载全页 loading（portal 到 document.body，fixed 全屏遮罩） */}
      {busyKey === "__uninstall_batch__" && createPortal(
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
            {t("skills.uninstalling", "Uninstalling…")}
          </div>
        </div>,
        document.body,
      )}

      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <h2 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>{t("skills.title", "Skills")}</h2>
          {refreshing && !installedLoading && (
            <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>
              {t("skills.refreshing", "Refreshing…")}
            </span>
          )}
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <Button
            className="ripple"
            style={{ fontSize: 14 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null}
            onClick={(e) => { makeRipple(e); setSubView("install"); }}
            title={t("skills.install.addBtn", "+ Add")}
          >
            {t("skills.install.addBtn", "+ Add")}
          </Button>
          <Button
            variant="outline"
            className="ripple"
            style={{ fontSize: 14 }}
            disabled={busyKey !== null}
            onClick={(e) => { makeRipple(e); setPasteText(""); setMessage(null); setPasteOpen(true); }}
            title={t("skills.importFromShare", "Import from share")}
          >
            {t("skills.importFromShare", "Import from share")}
          </Button>
          <Button
            variant="outline"
            className="ripple"
            style={{ fontSize: 14 }}
            disabled={scopeInvalid || busyKey !== null || refreshing}
            onClick={(e) => { makeRipple(e); refreshInstalled(); }}
            title={t("skills.refresh", "Refresh")}
          >
            {refreshing ? t("skills.refreshing", "Refreshing…") : t("skills.refresh", "Refresh")}
          </Button>
          <Button
            variant="outline"
            className="ripple"
            style={{ fontSize: 14 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null}
            onClick={(e) => { makeRipple(e); handleUpdate(); }}
          >
            {busyKey === "__update__" ? t("skills.updating", "Updating…") : t("skills.updateAll", "Update All")}
          </Button>
          <Button
            variant="destructive"
            className="ripple"
            style={{ fontSize: 14 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0}
            onClick={(e) => { makeRipple(e); setConfirmUninstall(true); }}
          >
            {busyKey === "__uninstall__" ? t("skills.uninstalling", "Uninstalling…") : t("skills.uninstallAll", "Uninstall All")}
          </Button>
          <Button
            variant="destructive"
            className="ripple"
            style={{ fontSize: 14 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null || selectedNames.size === 0}
            onClick={(e) => { makeRipple(e); setConfirmUninstallBatch(true); }}
          >
            {busyKey === "__uninstall_batch__"
              ? t("skills.uninstalling", "Uninstalling…")
              : t("skills.uninstallSelected", "Uninstall selected ({{count}})", { count: selectedNames.size })}
          </Button>
          <Button
            variant="outline"
            className="ripple"
            style={{ fontSize: 14 }}
            disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0}
            onClick={(e) => { makeRipple(e); setAlignOpen(true); }}
          >
            {busyKey === "__align__" ? t("skills.aligning", "Aligning…") : t("skills.alignTitle", "Align Config")}
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
            fontSize: 14,
            color: "var(--text-secondary)",
            borderInlineStart: "3px solid var(--accent)",
          }}
        >
          {t("skills.envMissing", "npx / Node.js not detected. Install and update are unavailable. Please install Node.js first.")}
        </div>
      )}

      {/* 操作结果消息（portal 到 document.body：脱离 Skills 页 transform 祖先，fixed 始终相对 viewport 顶部居中；4s 自动消失） */}
      {message && createPortal(
        <div
          className="animate-fade-in-150"
          style={{
            position: "fixed",
            top: 16,
            left: "50%",
            transform: "translateX(-50%)",
            zIndex: 300,
            maxWidth: "calc(100vw - 32px)",
          }}
        >
          <div
            className="glass-elevated"
            style={{
              padding: "10px 16px",
              fontSize: 14,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
              display: "flex",
              alignItems: "center",
              gap: 12,
              boxShadow: "0 8px 24px rgba(var(--shadow-rgb), 0.18)",
            }}
          >
            <span style={{ flex: 1 }}>{message}</span>
            <Button
              type="button"
              variant="ghost"
              onClick={() => setMessage(null)}
              aria-label={t("action.dismiss", "Dismiss")}
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
            <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>
              {t("skills.total", "Total installed")}
            </span>
          </div>
          <div style={{ display: "flex", gap: 20 }}>
            {AGENTS.map((a) => (
              <div key={a} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <img src={AGENT_ICONS[a]} alt={t(`skills.agent.${a}`, a)} className="hover-lift" style={{ width: 22, height: 22 }} />
                <div style={{ display: "flex", flexDirection: "column" }}>
                  <span style={{ fontSize: 18, fontWeight: 700, lineHeight: 1.1 }}>{agentCounts[a]}</span>
                  <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>
                    {t(`skills.agent.${a}`, a)}
                  </span>
                </div>
                <Button
                  variant="outline"
                  className="ripple"
                  style={{ fontSize: 14, padding: "3px 8px" }}
                  disabled={!writeReady || scopeInvalid || busyKey !== null || installed.length === 0 || agentCounts[a] === installed.length}
                  onClick={(e) => { makeRipple(e); handleEnableAll(a); }}
                  title={t("skills.enableAll", "Enable All")}
                >
                  {busyKey === `__enableall_${a}__` ? t("skills.enabling", "Enabling…") : t("skills.enableAll", "Enable All")}
                </Button>
              </div>
            ))}
            {/* pi：无 per-skill 启停，全部常开，故只报总数、不给「全部启用」按钮 */}
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <img src={piIcon} alt="pi" className="hover-lift" style={{ width: 22, height: 22 }} />
              <div style={{ display: "flex", flexDirection: "column" }}>
                <span style={{ fontSize: 18, fontWeight: 700, lineHeight: 1.1 }}>{total}</span>
                <span style={{ fontSize: 14, color: "var(--text-secondary)" }}>
                  {t("skills.piAlwaysOnDesc", "pi scans the shared skill directory; no per-skill enabling needed")}
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* 右: scope 筛选 (右对齐) */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8, alignItems: "flex-end" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14 }}>
            <span className="text-secondary">{t("skills.scope", "Scope")}</span>
            <Select value={scopeKind} onValueChange={(v) => setScopeKind(v as "global" | "project")}>
              <SelectTrigger style={{ width: "auto", fontSize: 14 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="global">{t("skills.scopeGlobal", "User (global)")}</SelectItem>
                <SelectItem value="project">{t("skills.scopeProject", "Project")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          {scopeKind === "project" && (
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <Input
                style={{ flex: 1 }}
                placeholder={t("skills.projectPathPlaceholder", "Absolute project directory path")}
                value={projectPath}
                onChange={(e) => setProjectPath(e.target.value)}
              />
              <Button variant="outline" style={{ fontSize: 14 }} onClick={pickProjectDir}>
                {t("skills.browse", "Browse…")}
              </Button>
            </div>
          )}
        </div>
      </Card>

      {/* 搜索框 + 启用态筛选（仅有已装 skills 时显示，照 Platforms/Groups 搜索框样式） */}
      {!installedLoading && installed.length > 0 && (
        <div style={{ display: "flex", gap: 8 }}>
          <Input
            style={{ flex: 1, fontSize: 14 }}
            placeholder={t("skills.searchPlaceholder", "Search skills...")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          <Select value={enabledFilter} onValueChange={(v) => setEnabledFilter(v as "all" | "enabled" | "disabled")}>
            <SelectTrigger style={{ width: "auto", fontSize: 14 }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t("skills.filterAll", "All")}</SelectItem>
              <SelectItem value="enabled">{t("skills.filterEnabled", "Enabled")}</SelectItem>
              <SelectItem value="disabled">{t("skills.filterDisabled", "Disabled")}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      )}

      {/* 已装列表（统一一条/skill） */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {installedLoading ? (
          <div className="text-secondary" style={{ padding: 12 }}>{t("status.loading", "Loading...")}</div>
        ) : installed.length === 0 ? (
          <div className="glass-surface text-secondary" style={{ padding: "24px 16px", textAlign: "center", fontSize: 14 }}>
            {t("skills.installedEmpty", "No skills installed in this scope")}
          </div>
        ) : filteredInstalled.length === 0 ? (
          <div className="glass-surface text-secondary" style={{ padding: "24px 16px", textAlign: "center", fontSize: 14 }}>
            {t("skills.searchEmpty", "No matching skills")}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {filteredInstalled.map((skill, idx) => (
              <SkillRow
                key={skill.name}
                skill={skill}
                idx={idx}
                busyKey={busyKey}
                origin={origins[skill.name]}
                writeReady={writeReady}
                selected={selectedNames.has(skill.name)}
                onSelect={toggleSelected}
                onToggle={handleToggle}
                onDetail={setDetailTarget}
                onShare={handleShare}
                onUninstall={setUninstallTarget}
              />
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

// ─── 单行 Skill ───

// ponytail: 行级 reveal 包装 — 每实例独立 useReveal (React 规则禁 map 内 hook),
// stagger idx*60 错峰，hover-lift + glass-surface 萤火虫流光描边走 .glass-surface:hover。
interface SkillRowProps {
  skill: SkillInfo;
  idx: number;
  busyKey: string | null;
  /** 所属插件（由 Claude 插件捆绑包分发时）。 */
  origin?: string;
  writeReady: boolean;
  selected: boolean;
  onSelect: (name: string) => void;
  onToggle: (skill: SkillInfo, agent: SkillAgent) => void;
  onDetail: (skill: SkillInfo) => void;
  onShare: (skill: SkillInfo) => void;
  onUninstall: (skill: SkillInfo) => void;
}

function SkillRow({ skill, idx, busyKey, origin, writeReady, selected, onSelect, onToggle, onDetail, onShare, onUninstall }: SkillRowProps) {
  const { t } = useTranslation();
  const { ref, shown } = useReveal<HTMLDivElement>(idx * 60);
  return (
    <Card
      ref={ref}
      className={`glass-surface hover-lift reveal${shown ? " in" : ""}`}
      style={{ padding: "12px 16px", display: "flex", gap: 12, alignItems: "center" }}
    >
      {/* 批量卸载勾选 */}
      <input
        type="checkbox"
        aria-label={t("skills.uninstallSelected", "Uninstall selected ({{count}})", { count: 1 })}
        checked={selected}
        disabled={busyKey !== null}
        onChange={() => onSelect(skill.name)}
        style={{ width: 15, height: 15, flexShrink: 0, cursor: "pointer", accentColor: "var(--accent)" }}
      />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <Button
            type="button"
            variant="ghost"
            title={t("skills.detail.view", "View Details")}
            style={{ fontSize: 14, fontWeight: 600, padding: 0, height: "auto" }}
            onClick={() => onDetail(skill)}
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
                fontSize: 14,
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
              title={t("skills.sourceType", "Source type")}
              style={{
                fontSize: 14,
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
              title={t("skills.pluginName", "Plugin source")}
              style={{
                fontSize: 14,
                padding: "2px 6px",
                background: "var(--bg-floating)",
                color: "var(--text-secondary)",
              }}
            >
              plugin: {skill.plugin_name}
            </Badge>
          )}
          <PluginOriginBadge plugin={origin} testId={`skill-plugin-${skill.name}`} />
          {skill.updated_at && (
            <span
              title={`${t("skills.updatedAt", "Updated")}: ${formatDateTime(skill.updated_at) ?? skill.updated_at}`}
              style={{
                fontSize: 14,
                color: "var(--text-secondary)",
              }}
            >
              {formatRelativeTime(skill.updated_at)}
            </span>
          )}
        </div>
        {skill.description?.trim() && (
          <div style={{ fontSize: 14, color: "var(--text-secondary)", marginTop: 4 }}>{skill.description}</div>
        )}
        {/* 安装时间次要行（紧凑） */}
        {skill.installed_at && (
          <div style={{ fontSize: 14, color: "var(--text-secondary)", marginTop: 2, opacity: 0.8 }}>
            <span>{t("skills.installedAt", "Installed")}: </span>
            <span title={skill.installed_at}>{formatDateTime(skill.installed_at)}</span>
            {skill.skill_folder_hash && (
              <>
                <span style={{ margin: "0 6px", opacity: 0.5 }}>·</span>
                <span title={t("skills.hash", "Content hash")} style={{ fontFamily: "monospace" }}>
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
            ? t("skills.disableAgent", "Disable {{agent}}", { agent: label })
            : t("skills.enableAgent", "Enable {{agent}}", { agent: label });
          return (
            <Button
              key={a}
              type="button"
              variant={enabled ? "default" : "outline"}
              className="ripple"
              title={aria}
              aria-label={aria}
              aria-pressed={enabled}
              disabled={!writeReady || busyKey !== null}
              onClick={(e) => { makeRipple(e); onToggle(skill, a); }}
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
                className="hover-lift"
                style={{ width: 18, height: 18, filter: enabled ? "none" : "grayscale(1)" }}
              />
              <span style={{ fontSize: 14, fontWeight: 600 }}>
                {busy ? t("skills.toggling", "…") : enabled ? t("skills.on", "Enabled") : t("skills.off", "Disabled")}
              </span>
            </Button>
          );
        })}
        {/* pi 是静态徽标不是开关：pi 原生扫公共 skill 目录，没有 per-skill 启停概念，
            做成可点开关就是在骗用户。 */}
        <span
          title={t("skills.piAlwaysOnHint", "pi scans the shared skill directory; no per-skill enabling needed")}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            padding: "5px 10px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
            background: "var(--secondary)",
            color: "var(--text-secondary)",
          }}
        >
          <img src={piIcon} alt="pi" style={{ width: 18, height: 18 }} />
          <span style={{ fontSize: 14, fontWeight: 600 }}>{t("skills.piAlwaysOn", "Always on")}</span>
        </span>
      </div>
      {/* 分享（仅 catalog 来源可分享：source 缺失的手动 symlink skill 隐藏按钮） */}
      {skillCatalogId(skill) && (
        <Button
          variant="outline"
          className="ripple"
          style={{ fontSize: 14, padding: "4px 10px", flexShrink: 0 }}
          onClick={(e) => { makeRipple(e); onShare(skill); }}
          title={t("skills.share", "Share")}
        >
          {t("skills.share", "Share")}
        </Button>
      )}
      {/* 单条卸载（破坏性，二次确认） */}
      <Button
        variant="destructive"
        className="ripple"
        style={{ fontSize: 14, padding: "4px 10px", flexShrink: 0 }}
        disabled={!writeReady || busyKey !== null}
        onClick={(e) => { makeRipple(e); onUninstall(skill); }}
        title={t("skills.uninstall", "Uninstall")}
      >
        {busyKey === `__uninstall_single_${skill.name}__`
          ? t("skills.uninstalling", "Uninstalling…")
          : t("skills.uninstall", "Uninstall")}
      </Button>
    </Card>
  );
}
