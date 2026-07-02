// ─── Skills 管理页 ──────────────────────────────────────────
// 顶层侧栏入口。统一已装列表（一条/skill，不分 agent）。
// 每行右侧展示 claude/codex 图标：在 enabled_agents 内=启用样式，否则=未启用样式，可点切换。
// 所有操作（list/enable/disable/update）全走后端 npx skills（无手动 fs）。
//
// scope 默认 Global（用户级全局 -g），可选 Project（选某项目目录）。
// npx/node 缺失 → 顶部提示条引导装 node，不阻塞整页。

import { useState, useEffect, useCallback, useMemo, useRef } from "react";
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
import { SkillDetailView } from "./SkillDetailView";
import { ShareModal } from "../components/platforms/ShareModal";
import { pinyinMatch } from "../utils/pinyin";
import { formatDateTime, formatRelativeTime } from "../utils/formatters";
import claudeIcon from "../assets/platforms/claude_code.svg";
import codexIcon from "../assets/platforms/openai.svg";

const AGENTS: SkillAgent[] = ["claude", "codex"];
const AGENT_ICONS: Record<SkillAgent, string> = { claude: claudeIcon, codex: codexIcon };

/** skill → catalog id（`owner/repo@skill`）。无 source（手动 symlink，非 catalog 来源）→ null，不可分享。 */
function skillCatalogId(s: SkillInfo): string | null {
  return s.source && s.name ? `${s.source}@${s.name}` : null;
}

/** 分享载荷：skill catalog id 列表（接收端 base64 → JSON → skills_install）。 */
interface SkillSharePayload {
  skills: string[];
}

/** 解码 base64 分享文本 → skill id 列表（接受 {skills: [...]} 包裹或裸 [...]）。
 *  返回 null = 非法格式。明文 base64(JSON) 与 ShareModal.copyUrl 产出对齐。 */
function decodeSkillShare(text: string): string[] | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  let json = trimmed;
  // 形如 base64（[A-Za-z0-9+/=] + 足够长）→ 尝试 atob；失败保持原文本走 JSON.parse。
  if (/^[A-Za-z0-9+/=\s]+$/.test(trimmed) && trimmed.length > 16) {
    try {
      json = atob(trimmed.replace(/\s/g, ""));
    } catch {
      // 非 base64，走原文本 JSON.parse。
    }
  }
  try {
    const parsed: unknown = JSON.parse(json);
    const ids = Array.isArray(parsed)
      ? parsed
      : Array.isArray((parsed as { skills?: unknown })?.skills)
        ? (parsed as { skills: unknown[] }).skills
        : null;
    if (!ids) return null;
    // 每项必须是 owner/repo@skill 形态（含 @）。
    const valid = ids.every((id) => typeof id === "string" && id.includes("@"));
    return valid ? (ids as string[]) : null;
  } catch {
    return null;
  }
}

export function Skills() {
  const { t } = useTranslation();

  const [env, setEnv] = useState<SkillsEnv | null>(null);
  const [scopeKind, setScopeKind] = useState<"global" | "project">("global");
  const [projectPath, setProjectPath] = useState("");
  // 子视图：list = 已装列表（默认）；install = 搜索安装页（按钮切换 + 返回）。
  const [subView, setSubView] = useState<"list" | "install">("list");
  const [detailTarget, setDetailTarget] = useState<SkillInfo | null>(null);

  const [installed, setInstalled] = useState<SkillInfo[]>([]);
  // 冷启动加载态（仅无缓存命中时显整页 loading）。
  const [installedLoading, setInstalledLoading] = useState(false);
  // 后台刷新态（SWR revalidate 中，显小"刷新中"指示，不阻塞列表）。
  const [refreshing, setRefreshing] = useState(false);
  // 上次 listRefresh 成功时间戳（ms），供获焦自动 revalidate 节流，避免每次获焦狂跑 npx。
  const lastRefreshAtRef = useRef(0);

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

  // 分享 modal（复用 D3 泛化 ShareModal，3 格式切换 + aidog://skill/import 链接）。
  const [shareData, setShareData] = useState<{ share: SkillSharePayload; name: string } | null>(null);
  // 粘贴分享文本导入 modal。
  const [pasteOpen, setPasteOpen] = useState(false);
  const [pasteText, setPasteText] = useState("");
  // 批量导入确认 modal（列出将装 skill id → scope 选择 → 批量 skills_install）。
  const [importIds, setImportIds] = useState<string[] | null>(null);
  const [importAgents, setImportAgents] = useState<Set<SkillAgent>>(() => new Set(AGENTS));
  const [importScopeKind, setImportScopeKind] = useState<"global" | "project">("global");
  const [importProjectPath, setImportProjectPath] = useState("");
  const [importBusy, setImportBusy] = useState(false);

  // 当前 scope 对象（供 API 调用）。
  const scope: SkillScope =
    scopeKind === "project"
      ? { kind: "project", path: projectPath }
      : { kind: "global" };

  const writeReady = !!env?.npx_available;
  const scopeInvalid = scopeKind === "project" && projectPath.trim() === "";

  // 已装列表关键词搜索（纯前端 filter，按 name/description/source 拼音匹配）。
  const [searchQuery, setSearchQuery] = useState("");

  // 搜索过滤后的已装列表（统计/总数仍用全量 installed，搜索只影响列表展示）。
  const filteredInstalled = useMemo(() => {
    const q = searchQuery.trim();
    if (!q) return installed;
    return installed.filter(
      (s) =>
        pinyinMatch(q, s.name) ||
        pinyinMatch(q, s.description ?? "") ||
        pinyinMatch(q, s.source ?? ""),
    );
  }, [installed, searchQuery]);

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
      lastRefreshAtRef.current = Date.now();
      // F1: npx 失败/HOME 缺失时后端返 load_failed=true（保留旧缓存）。显加载失败提示让用户
      // 知道当前看到的是上次列表而非真清空（防「skills 没了」误判）。
      if (res.load_failed) {
        setMessage(t("skills.loadFailed", "列表加载失败，显示上次缓存。检查 npx/Node.js 与 HOME 配置。"));
      }
    } catch (e) {
      console.error("refresh installed failed", e);
    } finally {
      setRefreshing(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKind, projectPath]);

  // 开页/切 scope：SWR 缓存渲染 + 后台 revalidate。
  // 命中缓存 → 瞬间渲染缓存（无 spinner、无列表跳变），再后台 refreshInstalled 取最新覆盖。
  // 后台 revalidate 是 SWR 本义，也是修复「空缓存粘住」的关键：缓存的 stale 仅表示
  // 「有无缓存条目」，不表示新鲜度——一个空结果的缓存条目（如首次进页时真空 / npx 失败落空）
  // 会被永久当权威空态返回，若命中后从不 revalidate，则用户用 CLI 装了 skill 后 UI 永远显空。
  const loadInstalled = useCallback(async () => {
    if (scopeInvalid) {
      setInstalled([]);
      return;
    }
    try {
      const cached = await skillsApi.listInstalled(scope);
      if (!cached.stale) {
        // 命中缓存：瞬间渲染，再后台 revalidate 纠正过期/空缓存（不阻塞、不整页 loading）。
        setInstalled(cached.items);
        refreshInstalled();
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
      if (res.load_failed) {
        setMessage(t("skills.loadFailed", "列表加载失败，显示上次缓存。检查 npx/Node.js 与 HOME 配置。"));
      }
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

  // 窗口/标签获焦自动 revalidate：捕捉 app 外 CLI 改动（npx skills add/remove），节流 10s + 静默后台。
  // 仅列表视图 + scope 有效 + 非进行中刷新时触发，复用 refreshInstalled（不整页 loading、不跳列表）。
  useEffect(() => {
    const REVALIDATE_THROTTLE_MS = 10_000;
    const maybeRevalidate = () => {
      if (document.visibilityState !== "visible") return;
      if (subView !== "list" || scopeInvalid || refreshing) return;
      if (Date.now() - lastRefreshAtRef.current < REVALIDATE_THROTTLE_MS) return;
      refreshInstalled();
    };
    window.addEventListener("focus", maybeRevalidate);
    document.addEventListener("visibilitychange", maybeRevalidate);
    return () => {
      window.removeEventListener("focus", maybeRevalidate);
      document.removeEventListener("visibilitychange", maybeRevalidate);
    };
  }, [subView, scopeInvalid, refreshing, refreshInstalled]);

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

  // ─── 分享（catalog id 列表 → 弹泛化 ShareModal）───
  // source 缺失（手动 symlink，非 catalog 来源）→ 不可分享，按钮已隐藏；此处兜底提示。
  const handleShare = (skill: SkillInfo) => {
    const id = skillCatalogId(skill);
    if (!id) {
      setMessage(t("skills.share.noSource", "该 skill 非 catalog 来源，无法分享"));
      return;
    }
    setShareData({ share: { skills: [id] }, name: skill.name });
  };

  // ─── 批量导入（确认对话框 → skills_install per id）───
  const openImportConfirm = (ids: string[]) => {
    if (ids.length === 0) return;
    setImportIds(ids);
    setImportAgents(new Set(AGENTS));
    setImportScopeKind("global");
    setImportProjectPath("");
  };

  const importScope: SkillScope =
    importScopeKind === "project"
      ? { kind: "project", path: importProjectPath }
      : { kind: "global" };

  const handleImport = async () => {
    const ids = importIds;
    if (!ids || importAgents.size === 0) return;
    if (importScopeKind === "project" && importProjectPath.trim() === "") return;
    if (!env?.npx_available) {
      setMessage(t("skills.envMissing", "未检测到 npx / Node.js，安装与更新功能不可用。请先安装 Node.js。"));
      return;
    }
    setImportBusy(true);
    setMessage(null);
    const scope = importScope;
    const agents = Array.from(importAgents);
    let ok = 0;
    let fail = 0;
    const failed: string[] = [];
    for (const id of ids) {
      try {
        const res = await skillsApi.install(id, agents, scope);
        if (res.success) ok += 1;
        else {
          fail += 1;
          failed.push(id);
        }
      } catch (e) {
        console.error("import skill failed", id, e);
        fail += 1;
        failed.push(id);
      }
    }
    setImportIds(null);
    setImportBusy(false);
    if (fail === 0) {
      setMessage(t("skills.importOk", "已导入 {{count}} 项", { count: ok }));
    } else if (ok === 0) {
      setMessage(t("skills.importFail", "导入失败 {{count}} 项", { count: fail }));
    } else {
      setMessage(
        t("skills.importPartial", "成功 {{ok}}，失败 {{fail}}", { ok, fail }) +
          (failed.length > 0 ? `\n${failed.join(", ")}` : ""),
      );
    }
    // 导入目标 scope 可能非当前查看 scope，刷新取真实态。
    void refreshInstalled();
  };

  // ─── 粘贴分享文本导入（base64 / JSON → 解码 → 确认对话框）───
  const handlePasteImport = () => {
    const ids = decodeSkillShare(pasteText);
    if (!ids || ids.length === 0) {
      setMessage(t("skills.importInvalid", "分享文本格式无效"));
      return;
    }
    setPasteOpen(false);
    setPasteText("");
    openImportConfirm(ids);
  };

  // ─── aidog://skill/import?data=<base64> deep-link 导入 ───
  // 两路汇入（契约见 spec）：① mount 取 App.tsx 缓存（冷启动/他页唤起 setActiveNav 挂载本页后到达）；
  // ② 运行时 window 'aidog:skill' 事件（本页已 mount 热路径）。
  // data = toBase64Utf8(JSON) → decodeSkillShare → 确认对话框 → handleImport。
  const openDeepLinkImport = useCallback((data: string) => {
    if (!data) return;
    const ids = decodeSkillShare(data);
    if (!ids || ids.length === 0) {
      setMessage(t("skills.importInvalid", "分享文本格式无效"));
      return;
    }
    openImportConfirm(ids);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [t]);
  useEffect(() => {
    const w = window as unknown as { __aidogDeepLink?: Record<string, { action: string; data: string }> };
    const cached = w.__aidogDeepLink?.skill;
    if (cached?.data) {
      delete w.__aidogDeepLink!.skill; // 消费一次防重复
      openDeepLinkImport(cached.data);
    }
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ action: string; data: string }>).detail;
      if (detail?.data) {
        delete w.__aidogDeepLink!.skill; // 防重放：事件到达时也清缓存（mount/事件两路都 delete）
        openDeepLinkImport(detail.data);
      }
    };
    window.addEventListener("aidog:skill", handler);
    return () => window.removeEventListener("aidog:skill", handler);
  }, [openDeepLinkImport]);

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
            disabled={busyKey !== null}
            onClick={() => { setPasteText(""); setMessage(null); setPasteOpen(true); }}
            title={t("skills.importFromShare", "从分享导入")}
          >
            {t("skills.importFromShare", "从分享导入")}
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

      {/* 搜索框（仅有已装 skills 时显示，照 Platforms/Groups 搜索框样式） */}
      {!installedLoading && installed.length > 0 && (
        <input
          className="input"
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
              <div
                key={skill.name}
                className="glass-surface"
                style={{ padding: "12px 16px", display: "flex", gap: 12, alignItems: "center" }}
              >
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                    <button
                      type="button"
                      className="btn btn-ghost"
                      title={t("skills.detail.view", "查看详情")}
                      style={{ fontSize: 13, fontWeight: 600, padding: 0, cursor: "pointer" }}
                      onClick={() => setDetailTarget(skill)}
                    >
                      {skill.name}
                    </button>
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
                      <span
                        title={t("skills.sourceType", "来源类型")}
                        style={{
                          fontSize: 10,
                          padding: "2px 6px",
                          borderRadius: 4,
                          background: "var(--bg-floating)",
                          color: "var(--text-secondary)",
                          border: "1px solid var(--border)",
                          textTransform: "uppercase",
                          letterSpacing: 0.3,
                        }}
                      >
                        {skill.source_type}
                      </span>
                    )}
                    {skill.plugin_name && (
                      <span
                        title={t("skills.pluginName", "plugin 来源")}
                        style={{
                          fontSize: 10,
                          padding: "2px 6px",
                          borderRadius: 4,
                          background: "var(--bg-floating)",
                          color: "var(--text-secondary)",
                          border: "1px solid var(--border)",
                        }}
                      >
                        plugin: {skill.plugin_name}
                      </span>
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
                {/* 分享（仅 catalog 来源可分享：source 缺失的手动 symlink skill 隐藏按钮） */}
                {skillCatalogId(skill) && (
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 11, padding: "4px 10px", flexShrink: 0 }}
                    onClick={() => handleShare(skill)}
                    title={t("skills.share", "分享")}
                  >
                    {t("skills.share", "分享")}
                  </button>
                )}
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

      {/* 已装 skill 详情 modal（只读） */}
      {detailTarget && createPortal(
        <SkillDetailView skill={detailTarget} onClose={() => setDetailTarget(null)} />,
        document.body,
      )}

      {/* 分享 modal（泛化 ShareModal，3 格式切换 + 复制为 aidog://skill/import 链接） */}
      {shareData && (
        <ShareModal
          share={shareData.share}
          title={shareData.name}
          titleKey="skills.share.title"
          warningKey="skills.share.warning"
          urlScheme="aidog://skill/import"
          copyUrlKey="skills.share.copyUrl"
          onToast={(text) => setMessage(text)}
          onClose={() => setShareData(null)}
        />
      )}

      {/* 粘贴分享文本导入 modal */}
      {pasteOpen && createPortal(
        <div
          onClick={() => setPasteOpen(false)}
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
            style={{ width: "min(560px, 90vw)", padding: 20, display: "flex", flexDirection: "column", gap: 10 }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.pasteTitle", "从分享导入")}</div>
            <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>
              {t("skills.pasteHint", "粘贴他人分享的 skill 文本（base64 / JSON / aidog:// 链接的 data 部分）。")}
            </div>
            <textarea
              className="input"
              style={{ minHeight: 160, fontFamily: "var(--font-mono, monospace)", resize: "vertical" }}
              value={pasteText}
              placeholder="aidog://skill/import?data=... 或 base64 或 JSON 数组"
              onChange={(e) => setPasteText(e.target.value)}
            />
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setPasteOpen(false)}>
                {t("action.cancel", "取消")}
              </button>
              <button
                className="btn btn-primary"
                style={{ fontSize: 13 }}
                onClick={handlePasteImport}
                disabled={!pasteText.trim()}
              >
                {t("skills.importBtn", "导入")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 批量导入确认 modal（列出将装 skill id + scope/agents 选择） */}
      {importIds && createPortal(
        <div
          onClick={() => !importBusy && setImportIds(null)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 220,
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
            style={{ width: "min(560px, 92vw)", maxHeight: "82vh", padding: 22, display: "flex", flexDirection: "column", gap: 12 }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.importConfirmTitle", "确认导入 skills")}</div>
            <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>
              {t("skills.importConfirmDesc", "将安装以下 {{count}} 个 skill（npx skills add，需联网）：", { count: importIds.length })}
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 4, maxHeight: "30vh", overflow: "auto", padding: 8, border: "1px solid var(--border)", borderRadius: 8, background: "var(--bg-glass)" }}>
              {importIds.map((id) => (
                <div key={id} style={{ fontSize: 12, fontFamily: "var(--font-mono, monospace)" }}>{id}</div>
              ))}
            </div>
            {/* scope 选择 */}
            <div style={{ display: "flex", gap: 8, alignItems: "center", fontSize: 12 }}>
              <span className="text-secondary" style={{ minWidth: 56 }}>{t("skills.scope", "范围")}</span>
              <select
                className="input"
                style={{ width: "auto" }}
                value={importScopeKind}
                onChange={(e) => setImportScopeKind(e.target.value as "global" | "project")}
                disabled={importBusy}
              >
                <option value="global">{t("skills.scopeGlobal", "用户级（全局）")}</option>
                <option value="project">{t("skills.scopeProject", "项目级")}</option>
              </select>
              {importScopeKind === "project" && (
                <input
                  className="input"
                  style={{ flex: 1 }}
                  placeholder={t("skills.projectPathPlaceholder", "项目目录绝对路径")}
                  value={importProjectPath}
                  onChange={(e) => setImportProjectPath(e.target.value)}
                  disabled={importBusy}
                />
              )}
            </div>
            {/* agent 多选 */}
            <div style={{ display: "flex", gap: 10, alignItems: "center", fontSize: 12 }}>
              <span className="text-secondary" style={{ minWidth: 56 }}>{t("skills.importAgents", "目标 agent")}</span>
              {AGENTS.map((a) => {
                const on = importAgents.has(a);
                return (
                  <button
                    key={a}
                    type="button"
                    className="glass"
                    onClick={() =>
                      setImportAgents((prev) => {
                        const next = new Set(prev);
                        if (next.has(a)) next.delete(a);
                        else next.add(a);
                        return next;
                      })
                    }
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 6,
                      padding: "4px 10px",
                      borderRadius: 8,
                      border: on ? "1.5px solid var(--accent)" : "1px solid var(--border)",
                      background: on ? "var(--accent-subtle)" : "transparent",
                      opacity: on ? 1 : 0.5,
                      fontSize: 12,
                    }}
                  >
                    <img src={AGENT_ICONS[a]} alt={a} style={{ width: 16, height: 16 }} />
                    {t(`skills.agent.${a}`, a)}
                  </button>
                );
              })}
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setImportIds(null)} disabled={importBusy}>
                {t("action.cancel", "取消")}
              </button>
              <button
                className="btn btn-primary"
                style={{ fontSize: 13 }}
                onClick={() => void handleImport()}
                disabled={importBusy || importAgents.size === 0 || (importScopeKind === "project" && importProjectPath.trim() === "") || !env?.npx_available}
              >
                {importBusy ? t("skills.installing", "导入中…") : t("skills.importBtn", "导入")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}
    </div>
  );
}
