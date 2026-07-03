import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import {
  skillsApi,
  type SkillAgent,
  type SkillScope,
  type SkillsEnv,
  type SkillInfo,
  type SkillsOpResult,
} from "../../services/api";
import { pinyinMatch } from "../../utils/pinyin";
import { AGENTS } from "./constants";
import { decodeSkillShare, skillCatalogId, type SkillSharePayload } from "./share";

/**
 * Skills 页全部 state + actions（自原 Skills.tsx L72-542 外迁）。
 * 15 useState + 2 useRef + filteredInstalled memo + 5 effect + 2 loader + 10 handler。
 * 无逻辑变更，纯结构外迁。
 */
export function useSkillsData() {
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

  return {
    t,
    // env / scope
    env, scopeKind, setScopeKind, projectPath, setProjectPath, scope, writeReady, scopeInvalid, pickProjectDir,
    // sub view
    subView, setSubView,
    // installed list
    installed, setInstalled, installedLoading, refreshing, refreshInstalled, filteredInstalled,
    // search
    searchQuery, setSearchQuery,
    // stats
    total, agentCounts,
    // busy / message
    busyKey, message, setMessage,
    // handlers
    handleToggle, handleUpdate, handleUninstallAll, handleUninstallSingle, handleAlign, handleEnableAll, handleShare,
    // detail modal
    detailTarget, setDetailTarget,
    // confirm uninstall
    confirmUninstall, setConfirmUninstall,
    // uninstall single
    uninstallTarget, setUninstallTarget,
    // align modal
    alignOpen, setAlignOpen, alignFrom, setAlignFrom, alignTo, setAlignTo,
    // share modal
    shareData, setShareData,
    // paste import modal
    pasteOpen, setPasteOpen, pasteText, setPasteText, handlePasteImport,
    // batch import modal
    importIds, setImportIds, importAgents, setImportAgents, importScopeKind, setImportScopeKind,
    importProjectPath, setImportProjectPath, importBusy, handleImport,
  };
}

export type SkillsData = ReturnType<typeof useSkillsData>;
