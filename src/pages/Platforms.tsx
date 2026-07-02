import React, { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { platformApi, settingsApi, modelTestApi, quotaApi, schedulingApi, groupDetailApi, parseMockConfig, serializeMockConfig, parseNewApiConfig, serializeNewApiConfig, parsePlatformBreaker, serializePlatformBreaker, onProxyLogUpdated, DEFAULT_MOCK_CONFIG, DEFAULT_NEWAPI_CONFIG, type Platform, type PlatformStatus, type Protocol, type ModelSlot, type PlatformEndpoint, type ClientType, type PlatformUsageStats, type PlatformQuota, type LastTestResult, type MockConfig, type NewApiConfig, type ManualBudget, type ManualBudgetKind, type ManualBudgetUnit, type WindowUnit, type SchedulingBreakerSettings, type GroupDetail, type SharePlatform } from "../services/api";
import { IconClose, IconCheck } from "../components/icons";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import { ModelTestPanel } from "./ModelTestPanel";
import { GroupsEmbedded } from "./Groups";
import { MiddlewareRulesPanel } from "../components/settings/MiddlewareRules";
import { pinyinMatch } from "../utils/pinyin";
import { splitApiKeys } from "../utils/platformPaste";
import { SmartPasteModal, type SmartPasteApplyResult } from "../components/platforms/SmartPasteModal";
import { ShareModal } from "../components/platforms/ShareModal";
import { PlatformCard, LevelPriorityControl, type PlatformCardActions } from "../components/platforms/PlatformCard";
import { useThemeMode } from "../themes/useThemeMode";

// ponytail: 平台域 SDK（constants/defaults/health/autoCategorize/SearchableProtocolSelect/MockConfigEditor）
// 已抽至 ../domains/platforms，本页主组件仅 import 实际消费的符号。阶段 4 再二次拆主组件。
import {
  PROTOCOLS, ENDPOINT_PROTOCOLS, CLIENT_TYPES, PROTOCOL_LABELS, PROTOCOL_COLORS,
  MODEL_SLOTS, DEFAULT_NAMES, QUOTA_CONCURRENCY,
  getDefaultEndpoints, getDefaultModels, getDefaultModelList, defaultClientForProtocol,
  newManualBudget, autoCategorize,
  SearchableProtocolSelect, MockConfigEditor,
} from "../domains/platforms";

/** 毫秒时间戳 → datetime-local input 值 "YYYY-MM-DDTHH:MM"（本地时区，无秒）。
 *  datetime-local 不解析 ISO Z 后缀，须手动拼本地时间分量。 */
function toDatetimeLocal(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 编辑页分区卡片：glass-surface 容器 + 标题 + 可选描述 + 内容区，统一视觉层次。 */
interface FormSectionProps {
  title: string;
  desc?: string;
  /** 标题右侧操作区（如「添加端点」「获取模型」按钮）。 */
  action?: React.ReactNode;
  children: React.ReactNode;
}

function FormSection({ title, desc, action, children }: FormSectionProps) {
  return (
    <div
      className="glass-surface"
      style={{ display: "flex", flexDirection: "column", gap: 12, padding: 16, borderRadius: "var(--radius-md)" }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
        <div style={{ minWidth: 0 }}>
          <div style={{ fontSize: 13, fontWeight: 700, color: "var(--text-primary)" }}>{title}</div>
          {desc && (
            <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.4, marginTop: 2 }}>{desc}</div>
          )}
        </div>
        {action && <div style={{ flexShrink: 0 }}>{action}</div>}
      </div>
      {children}
    </div>
  );
}

export function Platforms({ onNavigate, initialFilter }: { onNavigate?: (id: string, context?: { platformId?: number; platformName?: string; duplicate?: boolean }) => void; initialFilter?: { platformId?: number; platformName?: string; duplicate?: boolean } }) {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  // 渐进加载计数（来自 GroupsEmbedded 逐组流式回传 {total, active}），驱动页头「N / M active」增量更新。
  // null = 尚未回传 → 回退本页自身 platforms 派生值（如本页列表先于分组流加载完）。
  const [progressiveCount, setProgressiveCount] = useState<{ total: number; active: number } | null>(null);
  // ── Drag reorder for platform list ──
  const [platDrag, setPlatDrag] = useState<{ from: number; to: number } | null>(null);
  const platListRef = useRef<HTMLDivElement>(null);
  const platDragStartRef = useRef<{ y: number; index: number } | null>(null);
  const platDidDragRef = useRef(false);
  // 拖拽 geometry 计算 rAF 节流：每帧最多算一次，避免逐 pointermove 全列 getBoundingClientRect
  const platDragRafRef = useRef<number | null>(null);
  const platDragYRef = useRef(0);

  const handlePlatPointerDown = (e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    platDragStartRef.current = { y: e.clientY, index };
  };

  // rAF 内执行：基于最新 clientY 重算插入位置
  const computeDragTarget = (clientY: number) => {
    const start = platDragStartRef.current;
    if (!start) return;
    if (!platDrag) {
      if (Math.abs(clientY - start.y) < 5) return;
      setPlatDrag({ from: start.index, to: start.index });
      platDidDragRef.current = true;
    }
    if (!platListRef.current) return;
    const cards = platListRef.current.querySelectorAll<HTMLElement>("[data-platform-id]");
    let newTo = cards.length;
    for (let i = 0; i < cards.length; i++) {
      const rect = cards[i].getBoundingClientRect();
      if (clientY < rect.top + rect.height / 2) { newTo = i; break; }
    }
    setPlatDrag(d => d ? { ...d, to: newTo } : null);
  };

  const handlePlatPointerMove = (e: React.PointerEvent) => {
    if (!platDragStartRef.current) return;
    platDragYRef.current = e.clientY; // 始终记录最新位置
    if (platDragRafRef.current !== null) return; // 本帧已排程，下一帧用最新 Y
    platDragRafRef.current = requestAnimationFrame(() => {
      platDragRafRef.current = null;
      computeDragTarget(platDragYRef.current);
    });
  };

  const handlePlatPointerUp = () => {
    if (platDragRafRef.current !== null) {
      cancelAnimationFrame(platDragRafRef.current);
      platDragRafRef.current = null;
    }
    if (platDrag) {
      const effectiveTo = platDrag.from < platDrag.to ? platDrag.to - 1 : platDrag.to;
      if (platDrag.from !== effectiveTo) {
        // 仅在未分组平台子集内重排（platDrag from/to 均为 standalone 索引）。
        const reordered = [...standalonePlatforms];
        const [moved] = reordered.splice(platDrag.from, 1);
        reordered.splice(effectiveTo, 0, moved);
        // 重建 platforms：已分组平台原位，未分组按新序填回（保 sort_order 全局一致）。
        let si = 0;
        setPlatforms(platforms.map(p => platformMembership.has(p.id) ? p : reordered[si++]));
        platformApi.reorder(reordered.map(pp => pp.id)).catch(console.error);
      }
    }
    setPlatDrag(null);
    platDragStartRef.current = null;
    setTimeout(() => { platDidDragRef.current = false; }, 50);
  };
  const [usageMap, setUsageMap] = useState<Record<number, PlatformUsageStats>>({});
  const [quotaMap, setQuotaMap] = useState<Record<number, PlatformQuota>>({});
  // 手动刷新（真查校准）后的平台 id → 优先展示 quotaMap 真值而非预估
  const [quotaRealIds, setQuotaRealIds] = useState<Record<number, boolean>>({});
  const [quotaRefreshing, setQuotaRefreshing] = useState<Record<number, boolean>>({});
  // ④ 延迟档 quota 待回标志：load 时对所有需查 quota 的平台置 true，HTTP 结算（成功/失败）后置 false。
  //    余额区据此显骨架而非 est 旧值，避免闪烁回填。
  const [quotaPending, setQuotaPending] = useState<Record<number, boolean>>({});
  // ④ 渐进档 usage 批量待回标志：load 时 true，批量 usageStatsAll 到达后 false → 用量区先骨架后数据。
  const [usageLoading, setUsageLoading] = useState(false);
  const [testResults, setTestResults] = useState<Record<number, "ok" | "fail">>({});
  // 平台「最近一次测试结果」徽章数据（proxy_log source_protocol='test' 最新一条），随 load() 拉取 + 监听 aidog-platform-test-completed 单卡刷新
  const [lastTestMap, setLastTestMap] = useState<Record<number, LastTestResult>>({});
  // ③⑤ quota 调度：待领取队列（按可视优先顺序入队）、已调度去重集合、需查 quota 的平台快照。
  //    IntersectionObserver 决定入队时机/优先级，有界 worker pool 控并发上限。用 ref 不触发渲染。
  const quotaQueueRef = useRef<Platform[]>([]);
  // 局部刷新守卫：每次本地乐观写操作（保存/删除/清理）自增 epoch；在途的 load()/refreshStats
  //   captureEpoch 后异步返回时若 epoch 已变，跳过 setPlatforms(list) 整列表覆盖，防慢后端晚到回弹
  //   （mount-fetch-late-resolve-overwrites-optimistic 坑）。
  const platformsEpochRef = useRef(0);
  const quotaScheduledRef = useRef<Set<number>>(new Set());
  const quotaPoolActiveRef = useRef(0);
  const quotaWantMapRef = useRef<Map<number, Platform>>(new Map());
  const platformIObserverRef = useRef<IntersectionObserver | null>(null);
  /** favicon 加载失败的平台 ID 集合（回退到文字缩写） */
  const [faviconFailed, setFaviconFailed] = useState<Set<number>>(new Set());
  /** 列表卡片已展开（显 endpoints/模型明细）的平台 ID 集合 */
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());
  const toggleExpanded = (id: number, next: boolean) => {
    setExpandedIds(prev => {
      const s = new Set(prev);
      if (next) s.add(id); else s.delete(id);
      return s;
    });
  };
  const [testingId, setTestingId] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<Platform | null>(null);
  const [showForm, setShowForm] = useState(false);
  // GroupsEmbedded「添加分组」弹窗触发 ref（按钮上移到本页页头）。
  const openCreateGroupRef = useRef<(() => void) | null>(null);
  // GroupsEmbedded 跨组件刷新入口（全局 purge 删平台后，触发分组卡内重建）。
  const groupsReloadRef = useRef<(() => void) | null>(null);
  const [showPaste, setShowPaste] = useState(false);
  // aidog://platform/import deep-link 导入：SmartPasteModal 预填文本（来自 URL ?data=<base64>）。
  // 非空时弹窗以之初始化并跳过自动读剪贴板；null = 正常手动/剪贴板路径。
  const [pasteInitialText, setPasteInitialText] = useState<string | undefined>(undefined);
  // 平台分享弹窗：导出成功后持有 { share, name } 渲染 ShareModal（含明文 api_key + 格式切换）。
  const [shareData, setShareData] = useState<{ share: SharePlatform; name: string } | null>(null);
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState("");
  const [saveError, setSaveError] = useState("");
const [testingPlatform, setTestingPlatform] = useState<Platform | null>(null);
  const [toast, setToast] = useState<{ text: string; ok: boolean } | null>(null);
  // GroupsEmbedded 进入全屏视图态（创建/编辑分组）时为 true：隐藏下方分隔线 + 未分组平台列表，避免与全屏视图并列。
  const [groupFullscreen, setGroupFullscreen] = useState(false);
  // pointer 拖拽（未分组平台 → 分组）；HTML5 DnD 跨区域在 WKWebView 失效，改 pointer events
  const [groupDrag, setGroupDrag] = useState<{ pid: number; pname: string; x: number; y: number } | null>(null);
  const groupHighlightEl = useRef<HTMLElement | null>(null);
  const [showKey, setShowKey] = useState(false);

  // Form state
  const [name, setName] = useState("OpenAI");
  const [protocol, setProtocol] = useState<Protocol>("openai");
  const [codingPlan, setCodingPlan] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState<Record<ModelSlot, string>>({
    default: "", sonnet: "", opus: "", haiku: "", gpt: "",
  });
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [endpoints, setEndpoints] = useState<PlatformEndpoint[]>([]);
  const [activeDropdown, setActiveDropdown] = useState<ModelSlot | null>(null);
  const [showClaudeConfig, setShowClaudeConfig] = useState(false);
  const [claudeConfigJson, setClaudeConfigJson] = useState("");
  const [globalClaudeConfig, setGlobalClaudeConfig] = useState<Record<string, any>>({});
  // Mock 平台配置（持久化到 platform.extra 的 mock 子对象）
  const [extra, setExtra] = useState("");
  const [mockConfig, setMockConfig] = useState<MockConfig>({ ...DEFAULT_MOCK_CONFIG });
  const [newApiConfig, setNewApiConfig] = useState<NewApiConfig>({ ...DEFAULT_NEWAPI_CONFIG });
  // 手动预算限额（仅无上游 quota 自动支持平台可配；编辑表单态）
  const [manualBudgets, setManualBudgets] = useState<ManualBudget[]>([]);
  // 熔断阈值覆盖（0/空 = 继承全局默认；编辑表单态）。空字符串表示继承。
  const [breakerFailureThreshold, setBreakerFailureThreshold] = useState<string>("");
  const [breakerOpenSecs, setBreakerOpenSecs] = useState<string>("");
  const [breakerHalfOpenMax, setBreakerHalfOpenMax] = useState<string>("");
  // 全局调度+熔断默认（用于展示「继承默认 N」），只读消费。
  const [breakerDefaults, setBreakerDefaults] = useState<SchedulingBreakerSettings | null>(null);
  // 分组归属选项：auto_group（是否建默认分组，默认勾）+ join_group_ids（加入的已有分组）。
  // groupDetails 供 multi-select 渲染 + 编辑态反查平台当前手动组成员 + 平台归属映射构建。
  const [autoGroup, setAutoGroup] = useState(true);
  const [joinGroupIds, setJoinGroupIds] = useState<number[]>([]);
  const [groupDetails, setGroupDetails] = useState<GroupDetail[]>([]);
  // per-group level_priority 表单态（1~10，默认 5）。仅当平台归属唯一分组时可设。
  // 创建态：唯一分组 = 默认组(autoGroup) 或 joinGroupIds[0] 或 lockedGroupId。
  // 编辑态：从 groupDetails 反查该平台所属分组，唯一才显示。
  const [levelPriority, setLevelPriority] = useState(5);
  // 过期时间（毫秒 unix 时间戳，0 = 永不过期）。路由候选排除的独立维度（不改 status 三态）。
  const [expiresAt, setExpiresAt] = useState(0);
  // 「启用过期」toggle：默认 OFF（隐藏 datetime-local）。仅当用户勾选 toggle 才显示日期选择器；
  // 老平台 expires_at>0 加载时置 ON；粘贴识别填入 expiresAt 但 **保持 toggle OFF**（用户手动启用）。
  const [expiryEnabled, setExpiryEnabled] = useState(false);
  // 当前主题 mode（light/dark，订阅 documentElement data-mode 变化）。
  // 用于 datetime-local input 的 colorScheme 属性（控 WKWebView 原生日历弹出层明暗）。
  const themeMode = useThemeMode();
  // 锁定分组：从某分组 ➕ 触发创建平台时，预绑该分组且禁止修改归属。
  const [lockedGroupId, setLockedGroupId] = useState<number | null>(null);
  // 平台归属映射：platformId → groupNames[]（用于平台卡片显示所属分组 badge）
  const [platformMembership, setPlatformMembership] = useState<Map<number, string[]>>(new Map());
  // 平台管理页关键词搜索（纯前端 filter，按 name/base_url/协议拼音匹配）
  const [searchQuery, setSearchQuery] = useState("");
  // 未归属任何分组的平台（主列表独立展示）；已分组平台只在 GroupsEmbedded 内展示，避免重复。
  const standalonePlatforms = useMemo(
    () => platforms
      .filter(p => !platformMembership.has(p.id))
      .filter(p => {
        const q = searchQuery.trim();
        if (!q) return true;
        return pinyinMatch(q, p.name)
          || pinyinMatch(q, p.base_url)
          || pinyinMatch(q, p.platform_type);
      }),
    [platforms, platformMembership, searchQuery],
  );

  const isMock = protocol === "mock";
  // Claude Code 订阅纯透传：客户端自带订阅 OAuth 认证，aidog 原样转发。
  // 仅需 base_url（host 根），api_key 可空，隐藏 endpoints/models 编辑。
  const isPassthrough = protocol === "claude_code";
  // OpenCode Zen：免费匿名访问（api_key 留空时 proxy 兜底 $opencode），全程不校验 key 存在。
  const keyOptional = protocol === "opencode_zen";
  // 需要 api_key 但未填（keyOptional 平台不要求）—— fetch/列模型按钮共用的禁用判定。
  const apiKeyMissing = !keyOptional && !apiKey;
  // 唯一分组判定：平台最终归属恰好一个分组时，表单提供 level_priority 设置。
  // 创建态 count = (autoGroup?1:0) + joinGroupIds.length + (lockedGroupId?1:0 互斥)；
  // 编辑态 = auto 组 + 用户改后的 joinGroupIds。
  const uniqueGroupInfo = useMemo(() => {
    if (isPassthrough) return { show: false, groupId: null as number | null, isAuto: false };
    if (editing) {
      const autoGd = groupDetails.find(gd => gd.group.auto_from_platform === String(editing.id));
      const total = (autoGd ? 1 : 0) + joinGroupIds.length;
      if (total === 1) return { show: true, groupId: autoGd ? autoGd.group.id : joinGroupIds[0], isAuto: false };
      return { show: false, groupId: null, isAuto: false };
    }
    if (lockedGroupId != null) return { show: true, groupId: lockedGroupId, isAuto: false };
    const joinCount = joinGroupIds.length;
    if (autoGroup && joinCount === 0) return { show: true, groupId: null, isAuto: true };
    if (!autoGroup && joinCount === 1) return { show: true, groupId: joinGroupIds[0], isAuto: false };
    return { show: false, groupId: null, isAuto: false };
  }, [isPassthrough, editing, groupDetails, lockedGroupId, autoGroup, joinGroupIds]);

  /** 从 endpoints 中推导主 base_url（匹配主协议的 endpoint，否则取第一个） */
  const getPrimaryBaseUrl = (proto: Protocol, eps: PlatformEndpoint[]): string => {
    const primary = eps.find(ep => ep.protocol === proto);
    if (primary) return primary.base_url;
    return eps[0]?.base_url || "";
  };

  const handleProtocolChange = (newProtocol: Protocol, newCodingPlan?: boolean) => {
    const cp = !!newCodingPlan;
    // Auto-fill name with protocol label if empty or still at a default name
    if (!name.trim() || DEFAULT_NAMES.has(name)) {
      setName(cp ? `${PROTOCOL_LABELS[newProtocol]} Coding Plan` : PROTOCOL_LABELS[newProtocol]);
    }
    // Auto-fill endpoints from defaults（mock 无真实上游，返回空）
    const defaultEps = getDefaultEndpoints(newProtocol, cp);
    if (defaultEps.length > 0) {
      setEndpoints(defaultEps);
    } else {
      setEndpoints([]);
    }
    // Auto-fill 默认模型预设（与 endpoints 同步随协议切换）。
    // 仅填预设有值的槽位，其余保持空；未覆盖平台返回空对象 = 不改动。
    const defaultModels = getDefaultModels(newProtocol, cp);
    setModels({
      default: "", sonnet: "", opus: "", haiku: "", gpt: "",
      ...defaultModels,
    });
    // 切到 mock 时用当前 extra 初始化 mock 配置编辑器
    if (newProtocol === "mock") {
      setMockConfig(parseMockConfig(extra));
    }
    // 切到 newapi 时用当前 extra 初始化 newapi 配置
    if (newProtocol === "newapi") {
      setNewApiConfig(parseNewApiConfig(extra));
    }
    setProtocol(newProtocol);
    setCodingPlan(cp);
  };

  /** 智能识别弹窗确认后，将解析结果填入添加表单。 */
  const applyPaste = (r: SmartPasteApplyResult) => {
    // 命中 aidog 平台分享串 → 整体灌表单（含 api_key / models / endpoints / extra / 手动预算）。
    // 以「新建态」打开（editing=null）：保存才新建平台。优先于零散杂乱解析。
    if (r.fullShare) {
      const s = r.fullShare;
      setName(s.name);
      setProtocol(s.platform_type);
      setApiKey(s.api_key);  // fullShare 路径仍是单 key（分享串只含 1 个 api_key），保留单平台行为
      setCodingPlan((s.endpoints || []).some(ep => ep.coding_plan));
      setModels({
        default: s.models.default ?? "",
        sonnet: s.models.sonnet ?? "",
        opus: s.models.opus ?? "",
        haiku: s.models.haiku ?? "",
        gpt: s.models.gpt ?? "",
      });
      setAvailableModels(s.available_models ?? []);
      setEndpoints(s.endpoints ?? []);
      setManualBudgets(s.manual_budgets ?? []);
      const extra = s.extra ?? "";
      setExtra(extra);
      setMockConfig(parseMockConfig(extra));
      setNewApiConfig(parseNewApiConfig(extra));
      {
        const brk = parsePlatformBreaker(extra);
        setBreakerFailureThreshold(brk.failure_threshold > 0 ? String(brk.failure_threshold) : "");
        setBreakerOpenSecs(brk.open_secs > 0 ? String(brk.open_secs) : "");
        setBreakerHalfOpenMax(brk.half_open_max > 0 ? String(brk.half_open_max) : "");
      }
      setEditing(null);
      setLockedGroupId(null);
      setJoinGroupIds([]);
      setShowClaudeConfig(false);
      setClaudeConfigJson("");
      setFetchError("");
      setSaveError("");
      setShowPaste(false);
      setShowForm(true);
      return;
    }
    // 匹配到内置平台 → 走协议切换（设置 name + 默认 endpoints + client_type）。
    // 未匹配 → 不改平台选择（保持当前 protocol/endpoints），仅填 base_url/apiKey。
    // codingPlan flag 必传：同 value 的普通/coding 两 preset（如 xiaomi_mimo）命中后，
    // 不传 flag 则 getDefaultEndpoints 拿普通 endpoints（base_url 取错）。
    if (r.platform) {
      handleProtocolChange(r.platform.value as Protocol, r.platform.codingPlan);
    }
    // 同步计算出本批 pasted 应落入的有效 endpoints（供 setEndpoints + 批量分支共用）。
    // ponytail: 把原 setEndpoints(prev=>...) 回调提取为纯函数 computeEndpoints(prev)，
    // 既写表单态又把同值喂给批量创建，避免批量分支读到 setState 未提交的旧 endpoints。
    const computeEndpoints = (prev: PlatformEndpoint[]): PlatformEndpoint[] => {
      const eps = prev.map((e) => ({ ...e }));
      if (r.baseUrls.length === 0) return eps;
      // 命中内置平台：prev 已是该平台默认 endpoints（handleProtocolChange 填入）。
      // 按 host+path 最长子串把每条 pasted base_url 映射到对应默认 endpoint 覆盖其 base_url，
      // 保留该 endpoint 的 protocol/client_type。这样火山双端点（/api/coding→anthropic、
      // /api/coding/v3→openai/openai_responses）各落各位、不塌缩，且不依赖 guessProtocol
      // （v3/openai_responses 无法靠协议猜测区分）。同 base_url 多 endpoint（如 v3 同时映射
      // openai + openai_responses）全部一并覆盖，保持双协议端点。
      if (r.platform) {
        const norm = (s: string) => {
          try {
            const u = new URL(s);
            const host = u.host.replace(/^www\./, "").toLowerCase();
            const path = u.pathname.replace(/\/+$/, "").toLowerCase();
            return path && path !== "/" ? host + path : host;
          } catch { return s.toLowerCase(); }
        };
        for (const b of r.baseUrls) {
          const bn = norm(b.url);
          // 每条 url 选「与之最长公共前缀子串」的默认 endpoint：endpoint host+path 是 url 的前缀
          // （url 更具体，如 .../api/coding/v3 命中 endpoint .../api/coding/v3），取最长命中。
          let bestLen = -1;
          const targets: number[] = [];
          eps.forEach((e, i) => {
            const en = norm(e.base_url);
            // en 须是 bn 的路径边界前缀（url 比默认 endpoint 更具体或相等），避免 codingX 误命中 coding。
            if (bn === en || bn.startsWith(en + "/")) {
              if (en.length > bestLen) { bestLen = en.length; targets.length = 0; targets.push(i); }
              else if (en.length === bestLen) targets.push(i);
            }
          });
          if (targets.length) {
            for (const i of targets) eps[i] = { ...eps[i], base_url: b.url };
          } else {
            // host+path 无匹配（如粘贴裸 host 无版本段，或 preset 与分享 host 不一致）→
            // 退回按协议去重覆盖：同协议 endpoint 存在则覆盖 base_url，否则新增。
            const epProto: Protocol = b.protocol === "unknown" ? "openai" : b.protocol;
            const idx = eps.findIndex((e) => e.protocol === epProto);
            if (idx >= 0) eps[idx] = { ...eps[idx], base_url: b.url };
            else eps.push({ protocol: epProto, base_url: b.url, client_type: defaultClientForProtocol(epProto) });
          }
        }
        return eps;
      }
      // 未命中平台：按协议去重（每协议最多一个），同协议覆盖 base_url，否则新增。
      // 支持 anthropic + openai 双端点平台（如 glm）的零散粘贴。
      for (const b of r.baseUrls) {
        const epProto: Protocol = b.protocol === "unknown" ? "openai" : b.protocol;
        const idx = eps.findIndex((e) => e.protocol === epProto);
        if (idx >= 0) {
          eps[idx] = { ...eps[idx], base_url: b.url };
        } else {
          eps.push({ protocol: epProto, base_url: b.url, client_type: defaultClientForProtocol(epProto) });
        }
      }
      return eps;
    };
    // 单 key → 灌表单走旧路径；多 key → 灌表单后立刻批量创建 N 平台。
    // apiKeys 可能为空（用户只粘贴了 base_url 无 key），保留 setApiKey("") 旧行为。
    const keys = r.apiKeys ?? [];
    if (keys.length > 1) {
      // 多 key 批量：同步计算有效 endpoints / 协议（避免读到 setState 未提交的旧表单态）。
      // 命中平台 → endpoints 取平台默认 + pasted 覆盖；协议取平台 value。
      // 未命中平台 → 沿用当前表单 endpoints（用户已选），协议沿用当前 protocol。
      const basePrev = r.platform
        ? getDefaultEndpoints(r.platform.value as Protocol, r.platform.codingPlan)
        : endpoints;
      const effectiveEndpoints = computeEndpoints(basePrev);
      const effectiveProtocol: Protocol = r.platform
        ? (r.platform.value as Protocol)
        : protocol;
      // 灌表单态（让用户可见将批量化创建的配置），再异步触发批量循环。
      setApiKey(keys[0]);
      setEndpoints(effectiveEndpoints);
      if (r.expiresAt && r.expiresAt > 0) {
        setExpiresAt(r.expiresAt);
        setExpiryEnabled(true);
      }
      setShowPaste(false);
      setShowForm(true);
      void runBatchCreateFromPaste(keys, r.platform?.label, effectiveEndpoints, effectiveProtocol);
      return;
    }
    if (r.baseUrls.length > 0) {
      setEndpoints(computeEndpoints);
    }
    if (keys.length === 1) setApiKey(keys[0]);
    // 智能粘贴识别到的过期时间（社区分享帖常见「即将过期 06-28 23:59」）。0/未识别 = 不动。
    // 识别到则自动启用 expiry toggle，使过期字段在表单可见（与 coding_plan 自动识别对齐），
    // 否则 toggle 默认 OFF → datetime-local 隐藏 → 用户误判「没识别到过期时间」。
    if (r.expiresAt && r.expiresAt > 0) {
      setExpiresAt(r.expiresAt);
      setExpiryEnabled(true);
    }
    setShowPaste(false);
    // 弹窗可能从主列表「添加平台」直达（表单尚未挂载），apply 后显式拉起表单展示已填字段。
    setShowForm(true);
  };

  /**
   * 批量创建 N 平台（智能识别多 key 或手动表单多 key 共用）。
   * 共用当前表单的 protocol / endpoints / 分组（autoGroup / joinGroupIds / lockedGroupId），
   * 每平台挂自己的 api_key，name = `{baseName}-{key 尾4位}`，撞名（同尾4位）自动追号 `-2`。
   * enabled=true、不调 model_test。失败项不中断整批，末尾 toast 汇总「成功 X / 失败 Y + 失败 key」。
   *
   * @param keys N 个 apikey
   * @param baseName 平台名前缀（默认取当前 name 表单态）
   * @param effectiveEndpoints 批量创建使用的 endpoints（智能识别路径传入同步计算值，
   *   避免读到 setState 未提交的旧表单态；手动表单路径省略 → 用当前 endpoints 闭包值）
   * @param effectiveProtocol 批量创建使用的协议（同上）
   */
  const runBatchCreateFromPaste = async (
    keys: string[],
    baseName?: string,
    effectiveEndpoints?: PlatformEndpoint[],
    effectiveProtocol?: Protocol,
  ) => {
    const prefix = (baseName || name || "Platform").trim();
    // 智能识别路径显式传入同步计算值；手动表单路径沿用当前闭包值（用户已手填并点击保存）。
    const eps = effectiveEndpoints ?? endpoints;
    const proto = effectiveProtocol ?? protocol;
    const baseUrl = getPrimaryBaseUrl(proto, eps);
    if (!baseUrl && eps.length === 0) {
      setToast({ text: t("platform.batch.noBaseUrl", "批量创建失败：未设置 Base URL"), ok: false });
      setTimeout(() => setToast(null), 4000);
      return;
    }
    // 撞名追号：每次创建后即时把成功 name 纳入 used 集合，避免同尾4位连发撞名。
    const usedNames = new Set(platforms.map(p => p.name));
    const joinIds = lockedGroupId != null ? [lockedGroupId] : joinGroupIds;
    const auto = lockedGroupId != null ? false : autoGroup;
    let okCount = 0;
    const failures: { key: string; err: string }[] = [];
    // 批量进行中即时反馈（避免 N 次串行 invoke 时用户以为卡死）。
    setToast({ text: t("platform.batch.progress", "批量创建中… {{done}}/{{total}}", { done: 0, total: keys.length }), ok: true });
    for (let i = 0; i < keys.length; i++) {
      const k = keys[i];
      const tail = k.length >= 4 ? k.slice(-4) : k;
      let pname = `${prefix}-${tail}`;
      // 撞名（含本次批量已建）追号 -2 -3 …
      if (usedNames.has(pname)) {
        let seq = 2;
        while (usedNames.has(`${pname}-${seq}`)) seq++;
        pname = `${pname}-${seq}`;
      }
      try {
        const saved = await platformApi.create({
          name: pname, platform_type: proto, base_url: baseUrl, api_key: k,
          endpoints: eps.length > 0 ? eps : undefined,
          auto_group: auto,
          join_group_ids: joinIds,
          expires_at: expiresAt,
        });
        usedNames.add(pname);
        okCount++;
        // 局部刷新：append 单项（epoch guard 防晚到 resolve 覆盖）。
        platformsEpochRef.current++;
        setPlatforms(prev => prev.some(x => x.id === saved.id) ? prev : [...prev, saved]);
        scheduleQuotaFor(saved);
        // 进度 toast（每条更新）
        setToast({ text: t("platform.batch.progress", "批量创建中… {{done}}/{{total}}", { done: i + 1, total: keys.length }), ok: true });
      } catch (e: any) {
        failures.push({ key: k, err: e?.toString() || "Unknown error" });
        console.error("batch create failed", k, e);
      }
    }
    // 末尾汇总：成功 X / 失败 Y + 失败 key 列表（失败不静默吞）。
    handleGroupsChanged();
    groupsReloadRef.current?.();
    window.dispatchEvent(new Event("aidog-groups-changed"));
    resetForm();
    if (failures.length === 0) {
      setToast({ text: t("platform.batch.allOk", "批量创建完成：成功 {{n}} 个", { n: okCount }), ok: true });
    } else {
      const failList = failures.map(f => `${f.key.slice(-4)}: ${f.err}`).join("; ");
      setToast({
        text: `${t("platform.batch.summary", "批量创建：成功 {{ok}} / 失败 {{fail}}", { ok: okCount, fail: failures.length })} — ${failList}`,
        ok: okCount > 0,
      });
    }
    setTimeout(() => setToast(null), 6000);
  };

  /** 纯函数：从 groupDetails 构建 platformId → groupNames[] */
  function buildMembership(gds: GroupDetail[]): Map<number, string[]> {
    const m = new Map<number, string[]>();
    for (const g of gds) {
      for (const gp of g.platforms) {
        const arr = m.get(gp.platform.id) ?? [];
        arr.push(g.group.name);
        m.set(gp.platform.id, arr);
      }
    }
    return m;
  }

  /** 分组变更：refetch groupDetails，effect 自动重建 membership */
  const handleGroupsChanged = async () => {
    try {
      setGroupDetails(await groupDetailApi.list());
    } catch { /* ignore */ }
  };

  // ── pointer 拖拽未分组平台到分组（绕开 WKWebView HTML5 跨区域 DnD 失效）──
  const clearGroupHighlight = () => {
    if (groupHighlightEl.current) {
      groupHighlightEl.current.style.outline = "";
      groupHighlightEl.current.style.outlineOffset = "";
      groupHighlightEl.current = null;
    }
  };
  const findGroupAt = (x: number, y: number): { el: HTMLElement; gid: number } | null => {
    const el = document.elementFromPoint(x, y) as HTMLElement | null;
    const groupEl = el?.closest("[data-group-id]") as HTMLElement | null;
    if (!groupEl) return null;
    const gid = Number(groupEl.getAttribute("data-group-id"));
    return Number.isFinite(gid) && gid > 0 ? { el: groupEl, gid } : null;
  };
  const onStandaloneGroupPointerDown = (e: React.PointerEvent, p: Platform) => {
    if (e.button !== 0) return;
    const tgt = e.target as HTMLElement;
    // 让位：reorder handle（pointer 排序）+ 交互元素（按钮/输入）
    if (tgt.closest(".drag-handle-inline, button, a, input, [role=button]")) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setGroupDrag({ pid: p.id, pname: p.name, x: e.clientX, y: e.clientY });
  };
  const onStandaloneGroupPointerMove = (e: React.PointerEvent) => {
    setGroupDrag(d => d ? { ...d, x: e.clientX, y: e.clientY } : d);
    if (!groupDrag) return;
    clearGroupHighlight();
    const found = findGroupAt(e.clientX, e.clientY);
    if (found) {
      found.el.style.outline = "2px solid var(--accent)";
      found.el.style.outlineOffset = "2px";
      groupHighlightEl.current = found.el;
    }
  };
  const onStandaloneGroupPointerUp = (e: React.PointerEvent) => {
    if (!groupDrag) return;
    const pid = groupDrag.pid;
    const found = findGroupAt(e.clientX, e.clientY);
    clearGroupHighlight();
    setGroupDrag(null);
    if (!found) return;
    groupDetailApi.movePlatform(pid, 0, found.gid)
      .then(() => {
        setToast({ text: "已加入分组", ok: true });
        // 拖到分组只改归属：平台行本身不变，仅刷 groupDetails 重建 membership（卡片即移到目标组），
        // 无需整页 load()。保留事件广播供 GroupsEmbedded 等跨组件同步。
        handleGroupsChanged();
        window.dispatchEvent(new Event("aidog-groups-changed"));
      })
      .catch(err => setToast({ text: `加入分组失败: ${err}`, ok: false }));
  };

  /** 该平台是否需要外部 quota 查询（mock/claude_code 无配额；无 key / 无 base_url 不可查）。 */
  const platformWantsQuota = useCallback((p: Platform): boolean => {
    if (p.platform_type === "mock" || p.platform_type === "claude_code") return false;
    if (!p.api_key) return false;
    return !!getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []);
  }, []);

  /** 单平台 quota 查询（成功填 quotaMap），结束后清 pending。供有界并发池 worker 调用。 */
  const fetchQuotaForPlatform = useCallback(async (p: Platform) => {
    const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []);
    try {
      const q = p.platform_type === "newapi"
        ? await quotaApi.queryNewapi(baseUrl, p.api_key, p.extra ?? "", p.id)
        : await quotaApi.query(baseUrl, p.api_key, p.id);
      if (q.success) setQuotaMap(prev => ({ ...prev, [p.id]: q }));
    } catch { /* ignore */ }
    finally {
      setQuotaPending(prev => { const n = { ...prev }; delete n[p.id]; return n; });
    }
  }, []);

  // ③ 有界并发池：共享队列 quotaQueueRef + 至多 QUOTA_CONCURRENCY 个 worker 循环领取。
  //    入队由 ⑤ IntersectionObserver（可视/未折叠优先）+ 兜底全量补齐触发；scheduled 去重防重复拉。
  const pumpQuotaPool = useCallback(() => {
    const spawn = async () => {
      quotaPoolActiveRef.current++;
      try {
        for (;;) {
          const p = quotaQueueRef.current.shift();
          if (!p) break;
          await fetchQuotaForPlatform(p);
        }
      } finally {
        quotaPoolActiveRef.current--;
      }
    };
    while (quotaPoolActiveRef.current < QUOTA_CONCURRENCY && quotaQueueRef.current.length > 0) {
      void spawn();
    }
  }, [fetchQuotaForPlatform]);

  /** 把平台入 quota 队列（去重），并尝试启动 worker 领取。 */
  const enqueueQuota = useCallback((p: Platform) => {
    if (quotaScheduledRef.current.has(p.id)) return;
    if (!quotaWantMapRef.current.has(p.id)) return; // 非本轮需查平台（已结算/不需查）忽略
    quotaScheduledRef.current.add(p.id);
    quotaQueueRef.current.push(p);
    pumpQuotaPool();
  }, [pumpQuotaPool]);

  /** 局部刷新（新建/编辑平台）专用 quota 调度：不走 load() 重置 wantMap 路径，
   *  故新平台不在 quotaWantMapRef，无法经 enqueueQuota 入队。这里把单平台注入 wantMap + pending
   *  后入队，确保不走整页 load 的平台余额仍会被查（风险④：load 重置 quota 状态耦合）。 */
  const scheduleQuotaFor = useCallback((p: Platform) => {
    if (!platformWantsQuota(p)) return;
    quotaWantMapRef.current.set(p.id, p);
    setQuotaPending(prev => ({ ...prev, [p.id]: true }));
    // 已调度过则先放行重查（编辑可能改了 key/base_url）。
    quotaScheduledRef.current.delete(p.id);
    enqueueQuota(p);
  }, [platformWantsQuota, enqueueQuota]);

  const load = async () => {
    setLoading(true);
    const epoch = platformsEpochRef.current;
    let list: Platform[] = [];
    try {
      list = (await platformApi.list()) || [];
    } catch (e) { console.error(e); }
    // 在途期间发生本地乐观写（删除/保存/清理）则放弃整列表覆盖，避免晚到 resolve 回弹。
    if (epoch !== platformsEpochRef.current) { setLoading(false); return; }

    // ③⑤ quota 调度状态必须在 setPlatforms（→ DOM 提交 → IntersectionObserver 初次回调）之前同步就绪，
    //     否则 observer 初次 fire 时 quotaWantMapRef 仍为空 → enqueueQuota 早退 → 首屏卡片 quota 永不查
    //     （cards 已 intersecting，无后续 isIntersecting 跳变可再触发）。这是「余额/coding plan 全不展示」根因。
    quotaQueueRef.current = [];
    quotaScheduledRef.current = new Set();
    const wantMap = new Map<number, Platform>();
    const pending: Record<number, boolean> = {};
    for (const p of list) {
      if (platformWantsQuota(p)) { wantMap.set(p.id, p); pending[p.id] = true; }
    }
    quotaWantMapRef.current = wantMap;
    setQuotaPending(pending);

    setPlatforms(list);
    // 平台列表到手即渲染，余额/用量改后台渐进填充，禁止外部 quota HTTP 阻塞整页
    setLoading(false);

    // ① 渐进档：usage stats 单次批量（GROUP BY platform_id，含 platform_id=0 回溯），替换逐平台 N+1。
    setUsageLoading(true);
    try {
      const all = await platformApi.usageStatsAll();
      setUsageMap(all || {});
    } catch { /* ignore */ }
    finally {
      setUsageLoading(false);
    }

    // 平台「最近一次测试」徽章数据：并行拉取每平台最新 test 日志，有值才填（null 不填 = 不渲染徽章）
    Promise.all(list.map(p => platformApi.lastTestResult(p.id).catch(() => null)))
      .then(results => {
        const map: Record<number, LastTestResult> = {};
        results.forEach((r, i) => {
          if (r && list[i]) map[list[i].id] = r;
        });
        setLastTestMap(map);
      })
      .catch(() => { /* ignore */ });
  };

  /** 轻量刷新：按 id 局部 merge 派生统计字段（est_balance/est_coding_plan 等）+ usage stats 批量，
   *  不拉 quota HTTP、不整列表替换。高频被动触发（proxy log 订阅），整列表替换会打断 memo / 拖拽态
   *  并与乐观操作竞争回弹，故改为：仅更新已存在平台的字段，新增/删除的行交由显式写操作或 load() 处理。 */
  const refreshStats = async () => {
    const epoch = platformsEpochRef.current;
    try {
      const list = await platformApi.list();
      if (list && epoch === platformsEpochRef.current) {
        const byId = new Map(list.map(p => [p.id, p]));
        setPlatforms(prev => {
          let changed = false;
          const next = prev.map(p => {
            const fresh = byId.get(p.id);
            // 只 merge 后台派生的统计字段，保留前端排序/乐观态；字段相同则保引用（利于 memo）。
            if (!fresh) return p;
            if (
              fresh.est_balance_remaining === p.est_balance_remaining &&
              fresh.est_coding_plan === p.est_coding_plan &&
              fresh.last_real_query_at === p.last_real_query_at &&
              fresh.estimate_count === p.estimate_count &&
              fresh.last_error === p.last_error &&
              fresh.last_error_at === p.last_error_at
            ) return p;
            changed = true;
            return {
              ...p,
              est_balance_remaining: fresh.est_balance_remaining,
              est_coding_plan: fresh.est_coding_plan,
              last_real_query_at: fresh.last_real_query_at,
              estimate_count: fresh.estimate_count,
              last_error: fresh.last_error,
              last_error_at: fresh.last_error_at,
            };
          });
          return changed ? next : prev;
        });
      }
      const all = await platformApi.usageStatsAll();
      setUsageMap(all || {});
    } catch { /* ignore */ }
  };

  useEffect(() => { load(); }, []);

  // aidog://platform/import?data=<base64> deep-link 导入入口。
  // 两路汇入同一 helper：① mount 时取 App.tsx 缓存（冷启动 / 他页唤起 → setActiveNav 挂载本页后到达）；
  // ② 运行时 window 'aidog:platform' 事件（本页已 mount 的热路径）。base64 → SmartPasteModal 预填 + 自动识别
  // → applyPaste 流程（用户确认才创建）。data 为空 / 非 base64 → 忽略（SmartPasteModal 内识别兜底）。
  const openDeepLinkImport = useCallback((data: string) => {
    if (!data) return;
    // SmartPasteModal 挂在 `if (showForm)` 分支内，需先开 form 再开 paste 弹窗。
    // applyPaste(fullShare) 路径整体覆盖所有字段（setEditing(null) 等），故不调 resetForm。
    setPasteInitialText(data);
    setShowForm(true);
    setShowPaste(true);
  }, []);
  useEffect(() => {
    const w = window as unknown as { __aidogDeepLink?: Record<string, { action: string; data: string }> };
    const cached = w.__aidogDeepLink?.platform;
    if (cached?.data) {
      delete w.__aidogDeepLink!.platform; // 消费一次防重复
      openDeepLinkImport(cached.data);
    }
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ action: string; data: string }>).detail;
      if (detail?.data) {
        // 热路径（本页已 mount）也清缓存，否则离开再回（key={effectiveNav} 重挂载）会重放。
        delete w.__aidogDeepLink!.platform;
        openDeepLinkImport(detail.data);
      }
    };
    window.addEventListener("aidog:platform", handler);
    return () => window.removeEventListener("aidog:platform", handler);
  }, [openDeepLinkImport]);

  // ⑤ 可视区优先 quota 调度：IntersectionObserver 观察每张卡片（data-platform-id），
  //    进入视口即入队（enqueueQuota 去重 + 池控并发）；滚动到更多平台时触发其余。
  //    折叠/隐藏卡片不进视口→不触发；卡片复用（DnD/重排）由 platforms 依赖重建 observer 兜底。
  useEffect(() => {
    if (platforms.length === 0) return;
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const idAttr = (entry.target as HTMLElement).dataset.platformId;
        if (!idAttr) continue;
        const pid = Number(idAttr);
        const p = quotaWantMapRef.current.get(pid);
        if (p) enqueueQuota(p);
      }
    }, { root: null, rootMargin: "200px", threshold: 0 });
    platformIObserverRef.current = observer;
    const el = platListRef.current;
    if (el) el.querySelectorAll<HTMLElement>("[data-platform-id]").forEach(card => observer.observe(card));
    return () => { observer.disconnect(); platformIObserverRef.current = null; };
  }, [platforms, enqueueQuota]);

  // 外部导航上下文（如分组展开区点「编辑」→ onNavigate("platforms",{platformId})）打开对应平台编辑页。
  // 用 ref 记录已消费的 platformId，避免后续 load/reload 重复触发；平台列表到手后再匹配，否则等下一次列表更新。
  const consumedEditPidRef = useRef<number | null>(null);
  useEffect(() => {
    const pid = initialFilter?.platformId;
    if (!pid || consumedEditPidRef.current === pid) return;
    const target = platforms.find(p => p.id === pid);
    if (!target) return;  // 列表尚未加载到该平台，待 platforms 更新后重试
    consumedEditPidRef.current = pid;
    if (initialFilter?.duplicate) handleDuplicate(target);
    else handleEdit(target);
  }, [initialFilter?.platformId, initialFilter?.duplicate, platforms]);

  // 分组列表（multi-select 数据源 + 编辑态反查手动组归属 + 平台归属映射）。本地查询，失败不阻断编辑。
  useEffect(() => {
    groupDetailApi.list().then(setGroupDetails).catch(() => {});
  }, []);

  // groupDetails 变化时重建 membership（初始加载 + 所有 setGroupDetails 路径都覆盖）
  useEffect(() => { setPlatformMembership(buildMembership(groupDetails)); }, [groupDetails]);

  // 全局调度+熔断默认（展示「继承默认 N」用），读失败不阻断编辑。
  useEffect(() => {
    (async () => {
      try {
        setBreakerDefaults(await schedulingApi.getSettings());
      } catch (e) {
        console.error("get scheduling settings failed", e);
      }
    })();
  }, []);

  // 请求完成后轻量刷新统计（仅本地 DB 查询，不拉 quota HTTP）
  useEffect(() => onProxyLogUpdated(() => { refreshStats(); }), []);

  /** 刷新单个平台 quota（合查 balance + coding_plan） */
  const refreshQuota = async (p: Platform) => {
    if (!p.api_key) {
      setToast({ text: `${p.name}: ${t("platform.quotaNoKey", "缺少 API Key")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
      return;
    }
    // 手动刷新接管该平台 quota：清初始 pending（避免与 refreshing 旋转图标骨架重叠），显式调度去重也标记。
    setQuotaPending(prev => { const n = { ...prev }; delete n[p.id]; return n; });
    quotaScheduledRef.current.add(p.id);
    setQuotaRefreshing((s) => ({ ...s, [p.id]: true }));
    try {
      const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url;
      const q = p.platform_type === "newapi"
        ? await quotaApi.queryNewapi(baseUrl, p.api_key, p.extra ?? "", p.id)
        : await quotaApi.query(baseUrl, p.api_key, p.id);
      if (q.success) {
        setQuotaMap((s) => ({ ...s, [p.id]: q }));
        setQuotaRealIds((s) => ({ ...s, [p.id]: true }));
        // New API: 自动回填 user_id
        if (p.platform_type === "newapi" && q.newapi_user_id && editing?.id === p.id) {
          setNewApiConfig(prev => prev.user_id ? prev : { ...prev, user_id: q.newapi_user_id! });
        }
      } else {
        setToast({ text: `${p.name}: ${q.error || t("platform.quotaRefreshFail", "刷新额度失败")}`, ok: false });
        setTimeout(() => setToast(null), 3000);
      }
    } catch (e) {
      console.error(e);
      setToast({ text: `${p.name}: ${t("platform.quotaRefreshFail", "刷新额度失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
    setQuotaRefreshing((s) => ({ ...s, [p.id]: false }));
  };

  const resetForm = () => {
    setName(""); setProtocol("openai"); setCodingPlan(false); setApiKey("");
    setModels({ default: "", sonnet: "", opus: "", haiku: "", gpt: "" });
    setAvailableModels([]); setEndpoints([]);
    setEditing(null); setShowForm(false); setFetchError(""); setSaveError("");
    setShowClaudeConfig(false); setClaudeConfigJson("");
    setExtra(""); setMockConfig({ ...DEFAULT_MOCK_CONFIG });
    setNewApiConfig({ ...DEFAULT_NEWAPI_CONFIG });
    setManualBudgets([]);
    setBreakerFailureThreshold(""); setBreakerOpenSecs(""); setBreakerHalfOpenMax("");
    setAutoGroup(true); setJoinGroupIds([]); setLockedGroupId(null); setLevelPriority(5); setExpiresAt(0); setExpiryEnabled(false);
    // 关闭表单时复位「已消费的外部编辑导航 platformId」一次性 ref：否则经 onNavigate 进来的同一
    // 平台第二次编辑会被 consumedEditPidRef 短路（initialFilter.platformId 值不变，effect 亦不重跑）。
    consumedEditPidRef.current = null;
  };

  /** 打开平台创建表单（顶部「添加平台」或分组卡片 ➕ 触发）。
   *  lockGid 提供时预绑该分组并锁定、关闭 auto_group；否则用默认（建默认分组）。 */
  const openCreatePlatform = (presetGroupIds?: number[], lockGid?: number) => {
    resetForm();
    if (lockGid != null) {
      setAutoGroup(false);
      setJoinGroupIds(presetGroupIds && presetGroupIds.length > 0 ? presetGroupIds : [lockGid]);
      setLockedGroupId(lockGid);
    }
    // chips 渲染依赖 groupDetails，确保已加载。
    groupDetailApi.list().then(setGroupDetails).catch(() => {});
    setShowForm(true);
  };

  // 跳转该平台的日志（带 platformId 筛选上下文）。
  const handleViewLogs = (p: Platform) => {
    onNavigate?.("logs", { platformId: p.id, platformName: p.name });
  };

  const handleEdit = async (p: Platform) => {
    setName(p.name); setProtocol(p.platform_type); setApiKey(p.api_key);
    // 检测 endpoints 中是否有 coding_plan
    const hasCodingPlan = (p.endpoints || []).some(ep => ep.coding_plan);
    setCodingPlan(hasCodingPlan);
    setModels({
      default: p.models.default ?? "",
      sonnet: p.models.sonnet ?? "",
      opus: p.models.opus ?? "",
      haiku: p.models.haiku ?? "",
      gpt: p.models.gpt ?? "",
    });
    setAvailableModels(p.available_models ?? []);
    setEndpoints(p.endpoints ?? []);
    setEditing(p); setShowForm(true); setFetchError(""); setSaveError("");
    setShowClaudeConfig(false); setClaudeConfigJson("");
    setExtra(p.extra ?? "");
    setMockConfig(parseMockConfig(p.extra ?? ""));
    setNewApiConfig(parseNewApiConfig(p.extra ?? ""));
    setManualBudgets(p.manual_budgets ?? []);
    // 老平台 expires_at>0 → toggle 默认 ON；=0/未设 → OFF。
    setExpiresAt(p.expires_at ?? 0);
    setExpiryEnabled((p.expires_at ?? 0) > 0);
    // 熔断覆盖现存于 extra.breaker：0 = 继承 → 显示空
    {
      const brk = parsePlatformBreaker(p.extra ?? "");
      setBreakerFailureThreshold(brk.failure_threshold > 0 ? String(brk.failure_threshold) : "");
      setBreakerOpenSecs(brk.open_secs > 0 ? String(brk.open_secs) : "");
      setBreakerHalfOpenMax(brk.half_open_max > 0 ? String(brk.half_open_max) : "");
    }
    setLockedGroupId(null);
    // 反查该平台当前手动组成员（排除其 auto 分组），作为「加入已有分组」初始值。
    try {
      const gds = await groupDetailApi.list();
      setGroupDetails(gds);
      const manualIds = gds
        .filter(gd => gd.group.auto_from_platform !== String(p.id)
          && gd.platforms.some(gp => gp.platform.id === p.id))
        .map(gd => gd.group.id);
      setJoinGroupIds(manualIds);
      // 唯一分组回填 level_priority（auto 组 + 手动组总数==1 才显示控件）。
      const autoGd = gds.find(gd => gd.group.auto_from_platform === String(p.id));
      const total = (autoGd ? 1 : 0) + manualIds.length;
      if (total === 1) {
        const uniqGd = autoGd ?? gds.find(gd => gd.group.id === manualIds[0]);
        const lp = uniqGd?.platforms.find(gp => gp.platform.id === p.id)?.level_priority;
        setLevelPriority(lp ?? 5);
      } else {
        setLevelPriority(5);
      }
    } catch {
      setJoinGroupIds([]);
    }

    // Load global + platform Claude Code config
    try {
      const [globalResult, platformResult] = await Promise.all([
        settingsApi.get("global", "claude_code"),
        settingsApi.get(`platform:${p.id}`, "claude_code"),
      ]);
      const gv = (globalResult as Record<string, any>) ?? {};
      const pv = (platformResult as Record<string, any>) ?? {};
      setGlobalClaudeConfig(gv);
      setClaudeConfigJson(JSON.stringify({ ...gv, ...pv }, null, 2));
    } catch (e) { console.error(e); }
  };

  /** 分享平台：拉取可分享配置对象（含明文 api_key）→ 打开 ShareModal（弹窗内自动复制 + 格式切换）。 */
  const handleShare = async (p: Platform) => {
    try {
      const share = await platformApi.shareExport(p.id);
      setShareData({ share, name: p.name });
    } catch (err) {
      console.error("platform share export failed", err);
      setToast({ text: `${p.name}: ${t("platform.share.exportFail", "生成分享内容失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  /** 复制平台：复用源平台全部配置灌入表单，但以「新建态」打开（editing=null），保存才新建。
   *  与 handleEdit 唯一差异：setEditing(null)（不绑定源平台 id）+ Claude 配置仅在存在非空 override diff 时展开。 */
  const handleDuplicate = async (p: Platform) => {
    setName(p.name); setProtocol(p.platform_type); setApiKey(p.api_key);
    const hasCodingPlan = (p.endpoints || []).some(ep => ep.coding_plan);
    setCodingPlan(hasCodingPlan);
    setModels({
      default: p.models.default ?? "",
      sonnet: p.models.sonnet ?? "",
      opus: p.models.opus ?? "",
      haiku: p.models.haiku ?? "",
      gpt: p.models.gpt ?? "",
    });
    setAvailableModels(p.available_models ?? []);
    setEndpoints(p.endpoints ?? []);
    setEditing(null); setShowForm(true); setFetchError(""); setSaveError("");
    setShowClaudeConfig(false); setClaudeConfigJson("");
    setExtra(p.extra ?? "");
    setMockConfig(parseMockConfig(p.extra ?? ""));
    setNewApiConfig(parseNewApiConfig(p.extra ?? ""));
    setManualBudgets(p.manual_budgets ?? []);
    // 老平台 expires_at>0 → toggle 默认 ON；=0/未设 → OFF。
    setExpiresAt(p.expires_at ?? 0);
    setExpiryEnabled((p.expires_at ?? 0) > 0);
    {
      const brk = parsePlatformBreaker(p.extra ?? "");
      setBreakerFailureThreshold(brk.failure_threshold > 0 ? String(brk.failure_threshold) : "");
      setBreakerOpenSecs(brk.open_secs > 0 ? String(brk.open_secs) : "");
      setBreakerHalfOpenMax(brk.half_open_max > 0 ? String(brk.half_open_max) : "");
    }
    setLockedGroupId(null);
    // 反查源平台当前手动组成员（排除其 auto 分组），作为「加入已有分组」初始值。
    try {
      const gds = await groupDetailApi.list();
      setGroupDetails(gds);
      setJoinGroupIds(gds
        .filter(gd => gd.group.auto_from_platform !== String(p.id)
          && gd.platforms.some(gp => gp.platform.id === p.id))
        .map(gd => gd.group.id));
    } catch {
      setJoinGroupIds([]);
    }

    // 加载 global + 源平台 Claude Code 配置，合并填入；仅当源平台存在非空 override diff 时展开面板。
    try {
      const [globalResult, platformResult] = await Promise.all([
        settingsApi.get("global", "claude_code"),
        settingsApi.get(`platform:${p.id}`, "claude_code"),
      ]);
      const gv = (globalResult as Record<string, any>) ?? {};
      const pv = (platformResult as Record<string, any>) ?? {};
      setGlobalClaudeConfig(gv);
      setClaudeConfigJson(JSON.stringify({ ...gv, ...pv }, null, 2));
      if (Object.keys(pv).length > 0) setShowClaudeConfig(true);
    } catch (e) { console.error(e); }
  };

  const handleModelChange = (slot: ModelSlot, value: string) => {
    setModels(prev => ({ ...prev, [slot]: value }));
  };

  /** 从下拉选择一个模型填入指定槽位 */
  const handleModelSelect = (slot: ModelSlot, value: string) => {
    setModels(prev => ({ ...prev, [slot]: value }));
  };

  /** 一键获取：获取模型列表 + 自动分类 + 持久化
   *  默认使用 OpenAI 协议 endpoint，回退到主协议 endpoint */
  const handleFetchModels = async () => {
    const openaiEp = endpoints.find(ep => ep.protocol === "openai");
    const fetchUrl = openaiEp?.base_url || getPrimaryBaseUrl(protocol, endpoints);
    // opencode_zen /v1/models 无 auth 可列模型，api_key 可留空（后端兜底 $opencode）。
    if (!fetchUrl || apiKeyMissing) return;
    setFetching(true); setFetchError("");
    try {
      const fetchProtocol: Protocol = openaiEp ? "openai" : protocol;
      const modelIds = await platformApi.fetchModels(fetchProtocol, fetchUrl, apiKey);
      if (modelIds.length === 0) {
        setFetchError(t("platform.fetchEmpty"));
      } else {
        setAvailableModels(modelIds);
        const categorized = autoCategorize(modelIds);
        setModels(categorized);
      }
    } catch (e: any) {
      setFetchError(e.toString());
    }
    setFetching(false);
  };

  /** 一键填充：把 default 模型填到所有槽位（覆盖已有值） */
  const handleFillAll = () => {
    const defaultModel = models.default.trim();
    if (!defaultModel) return;
    setModels(prev => {
      const next = { ...prev };
      for (const slot of MODEL_SLOTS) {
        if (slot.key !== "default") {
          next[slot.key] = defaultModel;
        }
      }
      return next;
    });
  };

  const buildModelsPayload = () => {
    const result: Record<string, string | undefined> = {};
    let hasAny = false;
    for (const slot of MODEL_SLOTS) {
      const v = models[slot.key].trim();
      if (v) { result[slot.key] = v; hasAny = true; }
      else { result[slot.key] = undefined; }
    }
    return hasAny ? result : undefined;
  };

  const handleSave = async () => {
    setSaveError("");
    // 手动表单批量：apikey 字段粘入多 key（多行/逗号/空白/分号）→ 批量创建 N 平台。
    // 仅创建态触发；编辑态（editing != null）apiKey 是已存在平台的单值，不进入批量。
    // keyOptional 平台（透传/opencode_zen）apiKey 留空走原路径。
    if (!editing && !keyOptional) {
      const keys = splitApiKeys(apiKey);
      if (keys.length > 1) {
        await runBatchCreateFromPaste(keys);
        return;
      }
    }
    try {
      const modelsPayload = buildModelsPayload() as Platform["models"] | undefined;
      const availablePayload = availableModels.length > 0 ? availableModels : undefined;
      const baseUrl = getPrimaryBaseUrl(protocol, endpoints);
      // mock 平台：把配置写回 extra；newapi 平台写回 newapi 配置；其余原样保留
      let extraPayload = extra;
      if (isMock) extraPayload = serializeMockConfig(extra, mockConfig);
      if (protocol === "newapi") extraPayload = serializeNewApiConfig(extraPayload, newApiConfig);
      // 熔断覆盖现写入 extra.breaker：空 = 继承（写 0 → 移除 breaker 键）；负值钳为 0。
      const toBreakerNum = (s: string) => Math.max(0, Math.floor(Number(s) || 0));
      extraPayload = serializePlatformBreaker(extraPayload, {
        failure_threshold: toBreakerNum(breakerFailureThreshold),
        open_secs: toBreakerNum(breakerOpenSecs),
        half_open_max: toBreakerNum(breakerHalfOpenMax),
      });
      const extraArg = extraPayload ? extraPayload : undefined;
      // 手动预算：所有平台可设（含 mock / 有上游配额支持的平台），仅透传订阅强制清空。
      const manualBudgetsPayload: ManualBudget[] = isPassthrough ? [] : manualBudgets;
      let savedId: number | undefined;
      let saved: Platform | undefined;
      const wasEditing = !!editing;
      if (editing) {
        saved = await platformApi.update({
          id: editing.id, name, platform_type: protocol, base_url: baseUrl, api_key: apiKey,
          extra: extraArg,
          models: modelsPayload, available_models: availablePayload,
          endpoints: endpoints.length > 0 ? endpoints : undefined,
          manual_budgets: manualBudgetsPayload,
          join_group_ids: joinGroupIds,
          expires_at: expiresAt,
        });
        savedId = editing.id;
      } else {
        saved = await platformApi.create({
          name, platform_type: protocol, base_url: baseUrl, api_key: apiKey,
          extra: extraArg,
          models: modelsPayload, available_models: availablePayload,
          endpoints: endpoints.length > 0 ? endpoints : undefined,
          manual_budgets: manualBudgetsPayload.length > 0 ? manualBudgetsPayload : undefined,
          auto_group: autoGroup,
          join_group_ids: joinGroupIds,
          // 唯一分组为默认组(autoGroup)时, 后端建组直接用此值, 免前端回查。
          default_level_priority: uniqueGroupInfo.isAuto ? levelPriority : undefined,
          expires_at: expiresAt,
        });
        savedId = saved.id;
      }

      // 唯一分组为已有组(locked/join/editing 唯一关联)时, 平台落库后设其 level_priority。
      // (autoGroup 默认组路径走后端 default_level_priority, 不在此重复设。)
      if (savedId && uniqueGroupInfo.show && uniqueGroupInfo.groupId != null && !uniqueGroupInfo.isAuto) {
        try {
          await groupDetailApi.setPlatformLevelPriority(uniqueGroupInfo.groupId, savedId, levelPriority);
        } catch (e) { /* level_priority 非关键路径, 失败不阻塞保存 */ console.warn("set level_priority failed", e); }
      }

      // Save Claude Code config overrides for this platform
      if (savedId && showClaudeConfig && claudeConfigJson.trim()) {
        try {
          const merged = JSON.parse(claudeConfigJson);
          const diff: Record<string, any> = {};
          for (const [k, v] of Object.entries(merged)) {
            if (JSON.stringify(v) !== JSON.stringify(globalClaudeConfig[k])) {
              diff[k] = v;
            }
          }
          if (Object.keys(diff).length > 0) {
            await settingsApi.set(`platform:${savedId}`, "claude_code", diff);
          } else {
            await settingsApi.delete(`platform:${savedId}`, "claude_code");
          }
        } catch (e) { /* ignore JSON parse errors for config */ }
      }

      resetForm();
      // 局部刷新：用返回的完整 Platform 单项 setState（编辑=replace / 新建=append），不整页 load()。
      // 自增 epoch 让任何在途的 load()/refreshStats 放弃覆盖，防晚到 resolve 回弹乐观结果。
      if (saved) {
        const savedPlatform = saved;
        platformsEpochRef.current++;
        if (wasEditing) {
          setPlatforms(prev => prev.map(x => x.id === savedPlatform.id ? savedPlatform : x));
        } else {
          setPlatforms(prev => prev.some(x => x.id === savedPlatform.id) ? prev : [...prev, savedPlatform]);
        }
        // 不走 load() → 不会重置 quota 调度，须显式为该平台补 quota 查询（风险④）。
        scheduleQuotaFor(savedPlatform);
        // 单项变更后补刷该平台 usage / 最近测试徽章（load() 原本顺带刷，局部路径须自补）。
        platformApi.usageStats(savedPlatform.id)
          .then(u => setUsageMap(prev => ({ ...prev, [savedPlatform.id]: u })))
          .catch(() => {});
        platformApi.lastTestResult(savedPlatform.id)
          .then(r => { if (r) setLastTestMap(prev => ({ ...prev, [savedPlatform.id]: r })); })
          .catch(() => {});
      }
      // 保存可能改变分组归属（join_group_ids / auto_group 建默认组），
      // 必须刷新 groupDetails 重建 membership，否则已分组平台漏判为未分组、误现于底部未分配区。
      handleGroupsChanged();
      // 已分组平台（join_group_ids / auto_group）只在 <GroupsEmbedded> 的分组卡内渲染，而该组件渲染门控在
      // 其**自身** platforms state（Groups.tsx 分组卡 platforms.find），父级乐观 setPlatforms 不会注入。
      // 仅靠 window 事件依赖 mount 期绑定的 load() 闭包 + 多阶段异步重载，存在「保存成功但分组卡不刷新」窗口
      // （根因：新平台落入已有分组后既不进父级未分组列表、分组卡又未确定性重载 → UI 无反应、用户重复创建）。
      // 故与 purge 路径一致，显式调用专用命令式重载入口，确定性重建 GroupsEmbedded 平台态。
      groupsReloadRef.current?.();
      window.dispatchEvent(new Event("aidog-groups-changed"));
    } catch (e: any) {
      const msg = e?.toString() || "Unknown error";
      console.error(msg);
      setSaveError(msg);
      // saveError 渲染在长表单底部易被滚出视口（用户感知「无反应」）；额外用全局 toast 即时反馈，禁静默失败。
      setToast({ text: `${t("platform.saveFail", "保存失败")}: ${msg}`, ok: false });
      setTimeout(() => setToast(null), 4000);
    }
  };

  const handleDelete = async (id: number) => {
    // 删平台后端会清理 group_platform 关联并可能删孤儿 auto 组，
    // 故须刷新 groupDetails（重建 membership chips + 已分组/未分组归属），仅刷平台列表会留陈旧分组态。
    // 局部刷新：乐观从列表按 id 移除（不整页 load），失败回滚。
    let removed: Platform | undefined;
    let removedIndex = -1;
    platformsEpochRef.current++;
    setPlatforms(prev => {
      removedIndex = prev.findIndex(x => x.id === id);
      if (removedIndex >= 0) removed = prev[removedIndex];
      return prev.filter(x => x.id !== id);
    });
    try {
      await platformApi.delete(id);
      handleGroupsChanged();
      window.dispatchEvent(new Event("aidog-groups-changed"));
    } catch (e) {
      console.error(e);
      // 回滚：把被删平台插回原位。
      if (removed) {
        const r = removed; const idx = removedIndex;
        setPlatforms(prev => {
          if (prev.some(x => x.id === r.id)) return prev;
          const next = [...prev];
          next.splice(idx >= 0 && idx <= next.length ? idx : next.length, 0, r);
          return next;
        });
      }
      setToast({ text: `${t("platform.deleteFail", "删除失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  const handleToggle = async (p: Platform) => {
    // 三态切换：enabled → disabled；disabled / auto_disabled → enabled（恢复并清退避）。
    const nextStatus: PlatformStatus = p.status === "enabled" ? "disabled" : "enabled";
    // 乐观更新：立即本地置换该平台 status，UI 即时响应、不调 load() 全量重拉（避免整页 loading 闪烁）。
    // status 切换不改分组归属（membership 由 groupDetails 决定），故无需广播 aidog-groups-changed。
    setPlatforms(prev => prev.map(x =>
      x.id === p.id ? { ...x, status: nextStatus, enabled: nextStatus === "enabled" } : x));
    try {
      const updated = await platformApi.update({ id: p.id, status: nextStatus });
      // 用后端返回值校正单个 item（含清退避后的派生字段），仍不动其他平台、不重拉列表。
      setPlatforms(prev => prev.map(x => x.id === p.id ? updated : x));
    } catch (e) {
      console.error(e);
      // 失败回滚该 item 到原状态 + 报错。
      setPlatforms(prev => prev.map(x => x.id === p.id ? p : x));
      setToast({ text: `${p.name}: ${t("platform.toggleFail", "切换失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  const handleQuickTest = async (p: Platform) => {
    setTestingId(p.id);
    let success = false;
    try {
      const defaultModel = p.models.default || p.available_models[0] || "";
      const r = await modelTestApi.test({ platform_id: p.id, model: defaultModel });
      success = r.success;
      setTestResults(prev => ({ ...prev, [p.id]: r.success ? "ok" : "fail" }));
      setToast({ text: r.success
        ? `${p.name}: ${t("platform.testOk", "测试成功")}${r.duration_ms > 0 ? ` (${r.duration_ms}ms)` : ""}`
        : `${p.name}: ${r.error || t("platform.testFail", "测试失败")}`,
        ok: r.success });
    } catch (err: any) {
      setTestResults(prev => ({ ...prev, [p.id]: "fail" }));
      setToast({ text: `${p.name}: ${err?.message || t("platform.testFail", "测试失败")}`, ok: false });
    }
    setTestingId(null);
    setTimeout(() => setToast(null), 3000);
    // 派发全局事件：跨页（Groups 批量测 / ModelTestPanel 自定义）跑测后切到本页，本页卡片徽章 + health 据此即时刷新
    window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: p.id, success } }));
  };

  // 拉取某平台最近一次 test 日志，刷新 lastTestMap 对应项（供 aidog-platform-test-completed 监听后调用）
  const refreshLastTest = useCallback(async (platformId: number) => {
    try {
      const r = await platformApi.lastTestResult(platformId);
      setLastTestMap(prev => {
        const next = { ...prev };
        if (r) next[platformId] = r; else delete next[platformId];
        return next;
      });
    } catch { /* ignore */ }
  }, []);

  // 监听全局测试完成事件：单卡刷新「最近测试」徽章 + 写 testResults（驱动 health 走 manual 分支，
  // Groups 批量测 / ModelTestPanel 的成功失败信号即时反映到本页健康点）（事件来自本页快速测 / Groups 批量测 / ModelTestPanel）
  useEffect(() => {
    const handler = (e: Event) => {
      const ce = e as CustomEvent<{ platformId: number; success?: boolean }>;
      const pid = ce.detail?.platformId;
      if (pid == null) return;
      refreshLastTest(pid);
      if (ce.detail.success != null) {
        setTestResults(prev => ({ ...prev, [pid]: ce.detail.success ? "ok" : "fail" }));
      }
    };
    window.addEventListener("aidog-platform-test-completed", handler);
    return () => window.removeEventListener("aidog-platform-test-completed", handler);
  }, [refreshLastTest]);

  // 卡片操作集合：用 latest-ref 持有最新闭包，对外暴露稳定引用，保证 PlatformCard memo 生效
  const actionsRef = useRef({
    handlePlatPointerDown, handlePlatPointerMove, handlePlatPointerUp,
    toggleExpanded, refreshQuota, handleToggle, handleEdit, handleShare, handleDuplicate, handleDelete, handleViewLogs,
    handleQuickTest, setTestingPlatform, setFaviconFailed,
  });
  actionsRef.current = {
    handlePlatPointerDown, handlePlatPointerMove, handlePlatPointerUp,
    toggleExpanded, refreshQuota, handleToggle, handleEdit, handleShare, handleDuplicate, handleDelete, handleViewLogs,
    handleQuickTest, setTestingPlatform, setFaviconFailed,
  };
  const cardActions = useMemo<PlatformCardActions>(() => ({
    onPointerDown: (e, index) => actionsRef.current.handlePlatPointerDown(e, index),
    onPointerMove: (e) => actionsRef.current.handlePlatPointerMove(e),
    onPointerUp: () => actionsRef.current.handlePlatPointerUp(),
    onToggleExpanded: (id, next) => actionsRef.current.toggleExpanded(id, next),
    onRefreshQuota: (p) => actionsRef.current.refreshQuota(p),
    onToggleEnabled: (p) => actionsRef.current.handleToggle(p),
    onEdit: (p) => actionsRef.current.handleEdit(p),
    onShare: (p) => actionsRef.current.handleShare(p),
    onDuplicate: (p) => actionsRef.current.handleDuplicate(p),
    onDelete: (id) => actionsRef.current.handleDelete(id),
    onViewLogs: (p) => actionsRef.current.handleViewLogs(p),
    onQuickTest: (p) => actionsRef.current.handleQuickTest(p),
    onCustomTest: (p) => actionsRef.current.setTestingPlatform(p),
    onFaviconFailed: (id) => actionsRef.current.setFaviconFailed(prev => new Set(prev).add(id)),
  }), []);

  // 列表头部「启用 / 总数」派生值：仅随 platforms 变化，避免每次轮询/拖拽重渲染时重扫全列表
  const enabledCount = useMemo(() => platforms.filter(p => p.enabled).length, [platforms]);
  // 页头徽章计数：优先用 GroupsEmbedded 渐进回传值（随各组平台逐组流入增量更新），
  // 回退本页自身 platforms 派生值（progressiveCount 尚未回传 / 被重置时）。
  const headerActive = progressiveCount ? progressiveCount.active : enabledCount;
  const headerTotal = progressiveCount ? progressiveCount.total : platforms.length;

  // ── Edit / Add form (full page, no list) ──
  if (showForm) {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
        {/* Edit page header */}
        <div className="section-header" style={{ gap: 10 }}>
          <button className="btn btn-ghost" style={{ padding: "4px 8px", fontSize: 14 }} onClick={resetForm}>
            ← {t("action.back", "Back")}
          </button>
          <div style={{ flex: 1 }}>
            <div className="section-title">
              {editing ? editing.name : t("platform.add")}
            </div>
            {editing && (
              <div className="section-desc">{editing.platform_type.toUpperCase()} · {getPrimaryBaseUrl(editing.platform_type, editing.endpoints ?? []) || editing.base_url}</div>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            {!editing && (
              <button className="btn" onClick={() => setShowPaste(true)}>
                {t("platform.paste.title", "智能识别")}
              </button>
            )}
            <button className="btn" onClick={resetForm}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleSave}
              disabled={!name || (isPassthrough ? endpoints.length === 0 : (!isMock && !keyOptional && (endpoints.length === 0 || !apiKey)))}>
              {editing ? t("action.save") : t("action.create")}
            </button>
          </div>
        </div>

        {showPaste && (
          <SmartPasteModal
            presets={PROTOCOLS}
            onApply={applyPaste}
            initialText={pasteInitialText}
            onClose={() => { setShowPaste(false); setPasteInitialText(undefined); }}
          />
        )}

        <div className="animate-fade-in" style={{
          display: "flex",
          flexDirection: "column",
          gap: 16,
        }}>
          {/* 基础信息：名称 + 协议 */}
          <FormSection title={t("platform.sectionBasic", "基础信息")}>
            <input className="input" placeholder={t("platform.name")} value={name}
              onChange={(e) => setName(e.target.value)} />
          {editing ? (
            <div style={{
              display: "flex", alignItems: "center", gap: 8,
              padding: "10px 14px", borderRadius: "var(--radius-sm)",
              background: "var(--bg-glass)", border: "1px solid var(--border)",
              fontSize: 14,
            }}>
              <span style={{
                display: "inline-block", padding: "2px 8px", borderRadius: "var(--radius-sm)",
                background: `${PROTOCOL_COLORS[protocol] || "var(--accent)"}20`,
                color: PROTOCOL_COLORS[protocol] || "var(--accent)",
                fontSize: 11, fontWeight: 700,
              }}>
                {protocol.toUpperCase()}
              </span>
              <span style={{ color: "var(--text-tertiary)", fontSize: 12 }}>
                {t("platform.protocolLocked", "Protocol cannot be changed after creation")}
              </span>
            </div>
          ) : (
            <SearchableProtocolSelect
              value={protocol}
              codingPlan={codingPlan}
              onChange={handleProtocolChange}
            />
          )}
          </FormSection>

          {/* Mock 平台配置编辑器（仅 mock 平台显示，替代 endpoints / API Key / 模型） */}
          {isMock && (
            <FormSection title={t("platform.sectionSpecial", "特例配置")}>
              <MockConfigEditor config={mockConfig} onChange={setMockConfig} />
            </FormSection>
          )}

          {/* New API 余额查询配置（仅 newapi 平台显示） */}
          {protocol === "newapi" && (
            <FormSection
              title={t("platform.newapiBalanceConfig", "余额查询配置")}
              desc={t("platform.newapiBalanceHint", "查询余额需要独立的地址和 Token（从控制台获取），与 API Key 不同")}
            >
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t("platform.newapiBalanceUrl", "余额查询地址")}
                </div>
                <input
                  className="input"
                  placeholder={t("platform.newapiBalanceUrlPlaceholder", "https://your-newapi-instance.com")}
                  value={newApiConfig.balance_base_url}
                  onChange={(e) => setNewApiConfig(prev => ({ ...prev, balance_base_url: e.target.value }))}
                />
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t("platform.newapiBalanceKey", "余额查询 Token")}
                </div>
                <input
                  className="input"
                  type="text"
                  placeholder={t("platform.newapiBalanceKeyPlaceholder", "sess-xxxx 或 access token")}
                  value={newApiConfig.balance_api_key}
                  onChange={(e) => setNewApiConfig(prev => ({ ...prev, balance_api_key: e.target.value }))}
                />
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t("platform.newapiUserId", "用户 ID")}
                </div>
                <input
                  className="input"
                  placeholder={t("platform.newapiUserIdPlaceholder", "数字 ID（可选）")}
                  value={newApiConfig.user_id}
                  onChange={(e) => setNewApiConfig(prev => ({ ...prev, user_id: e.target.value }))}
                />
              </div>
            </FormSection>
          )}

          {/* Claude Code 订阅（透传）配置：仅 base_url（host 根）+ 可空 api_key */}
          {isPassthrough && (
            <FormSection title={t("platform.sectionPassthrough", "透传配置")}>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-secondary)" }}>
                  {t("platform.passthroughBaseUrl", "上游地址（Base URL）")}
                </div>
                <input
                  className="input"
                  placeholder="https://api.anthropic.com"
                  value={endpoints[0]?.base_url ?? ""}
                  onChange={(e) => {
                    const next = [...endpoints];
                    if (next.length === 0) {
                      next.push({ protocol: "anthropic" as Protocol, base_url: e.target.value, client_type: "default" });
                    } else {
                      next[0] = { ...next[0], base_url: e.target.value };
                    }
                    setEndpoints(next);
                  }}
                />
                <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
                  {t("platform.passthroughBaseUrlHint", "填 host 根（如 https://api.anthropic.com）。纯透传会拼接客户端原始 path/query 直接转发，请勿带版本前缀。")}
                </div>
              </div>
              {/* 可空 API Key（透传模式客户端自带认证，留空即可） */}
              <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <input
                  className="input"
                  type={showKey ? "text" : "password"}
                  placeholder={t("platform.apiKeyOptional", "API Key（可选，透传可留空）")}
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  style={{ flex: 1 }}
                />
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  title={showKey ? "Hide key" : "Show key"}
                  onClick={() => setShowKey(!showKey)}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    {showKey ? (
                      <>
                        <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
                        <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
                        <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
                        <line x1="1" y1="1" x2="23" y2="23" />
                      </>
                    ) : (
                      <>
                        <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                        <circle cx="12" cy="12" r="3" />
                      </>
                    )}
                  </svg>
                </button>
              </div>
              <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
                {t("platform.passthroughNote", "纯透传：客户端请求的 header（含订阅 OAuth 认证）与 body 原样转发，aidog 不做任何转换或认证注入。上方 API Key 可留空。")}
              </div>
            </FormSection>
          )}

          {/* Protocol Endpoints（mock / 透传平台隐藏，无可编辑上游） */}
          {!isMock && !isPassthrough && (
          <>
          <FormSection
            title={t("platform.endpoints", "Protocol Endpoints")}
            desc={t("platform.endpointsHint", "Additional protocols this platform supports with different base URLs")}
            action={(
              <button
                type="button"
                className="btn btn-ghost"
                style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
                onClick={() => setEndpoints([...endpoints, { protocol: "openai" as Protocol, base_url: "", client_type: defaultClientForProtocol("openai"), coding_plan: false }])}
              >
                + {t("platform.addEndpoint", "Add Endpoint")}
              </button>
            )}
          >
            {endpoints.length === 0 && (
              <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "4px 0", fontStyle: "italic" }}>
                {t("platform.noEndpoints", "No additional endpoints")}
              </div>
            )}
            {endpoints.map((ep, idx) => (
              <div key={idx} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <select
                  className="input"
                  style={{ width: 120, flexShrink: 0 }}
                  value={ep.protocol}
                  onChange={(e) => {
                    const newProto = e.target.value as Protocol;
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], protocol: newProto, client_type: defaultClientForProtocol(newProto) };
                    setEndpoints(next);
                  }}
                >
                  {ENDPOINT_PROTOCOLS.map((p) => (
                    <option key={p.value} value={p.value}>{p.label}</option>
                  ))}
                </select>
                <input
                  className="input"
                  style={{ flex: 1 }}
                  placeholder="Endpoint Base URL"
                  value={ep.base_url}
                  onChange={(e) => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], base_url: e.target.value };
                    setEndpoints(next);
                  }}
                />
                <select
                  className="input"
                  style={{ width: 140, flexShrink: 0 }}
                  value={ep.client_type || "default"}
                  onChange={(e) => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], client_type: e.target.value as ClientType };
                    setEndpoints(next);
                  }}
                  title={t("platform.clientType", "客户端模拟")}
                >
                  <option value="default">{t(CLIENT_TYPES[0].labelKey!)}</option>
                  {["Claude Code", "Codex", "IDE"].map(group => (
                    <optgroup key={group} label={group}>
                      {CLIENT_TYPES.filter(c => c.group === group).map(c => (
                        <option key={c.value} value={c.value}>{c.label}</option>
                      ))}
                    </optgroup>
                  ))}
                </select>
                {/* Coding Plan 开关 */}
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  style={{
                    flexShrink: 0,
                    width: 28, height: 28, minWidth: 28,
                    padding: 0,
                    fontSize: 11, fontWeight: 700,
                    color: ep.coding_plan ? "var(--color-success, var(--color-success))" : "var(--text-tertiary)",
                    background: ep.coding_plan ? "var(--color-success, var(--color-success))15" : "transparent",
                    border: `1px solid ${ep.coding_plan ? "var(--color-success, var(--color-success))40" : "var(--border)"}`,
                    borderRadius: "var(--radius-sm)",
                  }}
                  title={ep.coding_plan ? "Coding Plan ON" : "Coding Plan"}
                  onClick={() => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], coding_plan: !next[idx].coding_plan };
                    setEndpoints(next);
                  }}
                >
                  C
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-icon btn-danger"
                  style={{ flexShrink: 0 }}
                  onClick={() => setEndpoints(endpoints.filter((_, i) => i !== idx))}
                >
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                  </svg>
                </button>
              </div>
            ))}
          </FormSection>

          {/* API Key with show/copy */}
          <FormSection title={t("platform.sectionAuth", "认证")}>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <input
              className="input"
              type={showKey ? "text" : "password"}
              placeholder="API Key"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              style={{ flex: 1 }}
            />
            <button
              type="button"
              className="btn btn-ghost btn-icon"
              title={showKey ? "Hide key" : "Show key"}
              onClick={() => setShowKey(!showKey)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                {showKey ? (
                  <>
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
                    <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
                    <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
                    <line x1="1" y1="1" x2="23" y2="23" />
                  </>
                ) : (
                  <>
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                    <circle cx="12" cy="12" r="3" />
                  </>
                )}
              </svg>
            </button>
            {editing && apiKey && (
              <button
                type="button"
                className="btn btn-ghost btn-icon"
                title="Copy key"
                onClick={() => void writeText(apiKey)}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              </button>
            )}
          </div>
          </FormSection>

          {/* Models Configuration */}
          <FormSection
            title={t("platform.models")}
            action={(
              <div style={{ display: "flex", gap: 6 }}>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--text-secondary)" }}
                  onClick={handleFillAll}
                  disabled={!models.default.trim()}
                  title={t("platform.fillAllHint")}
                >
                  {t("platform.fillAll")}
                </button>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
                  onClick={handleFetchModels}
                  disabled={apiKeyMissing || endpoints.length === 0 || fetching}
                >
                  {fetching ? t("status.loading") : t("platform.fetchModels")}
                </button>
              </div>
            )}
          >
            {fetchError && (
              <div style={{ fontSize: 12, color: "var(--danger, #e55)", padding: "2px 0" }}>
                {fetchError}
              </div>
            )}
            {MODEL_SLOTS.map(({ key, labelKey }) => {
              const query = models[key].trim().toLowerCase();
              // 下拉源：fetchModels 成功用 available_models，否则用内置候选列表（冷启动兜底）
              const dropdownSource = availableModels.length > 0
                ? availableModels
                : getDefaultModelList(protocol, codingPlan);
              const hasDropdown = dropdownSource.length > 0;
              const filtered = hasDropdown
                ? (query
                  ? dropdownSource.filter(m => pinyinMatch(query, m))
                  : dropdownSource)
                : [];
              return (
              <div key={key} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{
                  fontSize: 12, fontWeight: 500, color: "var(--text-tertiary)",
                  width: 56, textAlign: "right", flexShrink: 0,
                }}>
                  {t(labelKey)}
                </span>
                <div style={{ position: "relative", flex: 1 }}>
                  <input
                    className="input"
                    style={{ width: "100%", paddingRight: hasDropdown ? 28 : undefined }}
                    placeholder={t(labelKey)}
                    value={models[key]}
                    onChange={(e) => {
                      handleModelChange(key, e.target.value);
                      if (hasDropdown) setActiveDropdown(key);
                    }}
                    onFocus={() => {
                      if (hasDropdown) setActiveDropdown(key);
                    }}
                  />
                  {hasDropdown && (
                    <button
                      type="button"
                      className="btn btn-ghost btn-icon"
                      style={{
                        position: "absolute", right: 2, top: "50%", transform: "translateY(-50%)",
                        width: 24, height: 24, minWidth: 24, padding: 0,
                        color: "var(--text-tertiary)", cursor: "pointer",
                      }}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        setActiveDropdown(activeDropdown === key ? null : key);
                      }}
                      title={t("platform.selectModel")}
                    >
                      ▾
                    </button>
                  )}
                  {/* 可搜索下拉列表 — 主题化 */}
                  {activeDropdown === key && filtered.length > 0 && (
                    <>
                      <div
                        style={{ position: "fixed", inset: 0, zIndex: 99 }}
                        onMouseDown={() => setActiveDropdown(null)}
                      />
                      <div
                        className="glass-elevated"
                        style={{
                          position: "absolute",
                          top: "100%",
                          left: 0,
                          right: 0,
                          marginTop: 4,
                          maxHeight: 200,
                          overflowY: "auto",
                          zIndex: 100,
                          padding: 4,
                          animation: "fadeIn 150ms ease both",
                        }}
                      >
                        {filtered.map((m) => (
                          <button
                            key={m}
                            type="button"
                            className="btn btn-ghost"
                            style={{
                              width: "100%",
                              justifyContent: "flex-start",
                              padding: "6px 10px",
                              fontSize: 12,
                              fontWeight: models[key] === m ? 600 : 400,
                              color: models[key] === m ? "var(--accent)" : "var(--text-primary)",
                              background: models[key] === m ? "var(--accent-subtle)" : "transparent",
                              borderRadius: "var(--radius-sm)",
                            }}
                            onMouseDown={(e) => {
                              e.preventDefault();
                              handleModelSelect(key, m);
                              setActiveDropdown(null);
                            }}
                          >
                            {m}
                          </button>
                        ))}
                      </div>
                    </>
                  )}
                </div>
              </div>
              );
            })}
          </FormSection>
          </>
          )}

          {/* Manual Budgets — 所有平台可设（含 mock / 有上游配额支持的平台），仅透传订阅不需要 */}
          {!isPassthrough && (
            <FormSection
              title={t("platform.manualBudgetTitle", "手动预算")}
              desc={t("platform.manualBudgetDesc", "该平台无上游额度自动查询，可手动设置一个或多个预算限额，按用量预估扣减；任一耗尽时停止转发（返回 402），窗口/次日恢复后自动放行。")}
              action={(
                <button
                  type="button"
                  className="btn btn-ghost"
                  style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
                  onClick={() => setManualBudgets([...manualBudgets, newManualBudget()])}
                >
                  {t("platform.manualBudgetAdd", "添加限额")}
                </button>
              )}
            >
              {manualBudgets.length === 0 && (
                <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "2px 0" }}>
                  {t("platform.manualBudgetEmpty", "暂无限额，点击「添加限额」开始配置。")}
                </div>
              )}
              {manualBudgets.map((b, idx) => {
                const update = (patch: Partial<ManualBudget>) =>
                  setManualBudgets(manualBudgets.map((x, i) => i === idx ? { ...x, ...patch } : x));
                const needsWindow = b.kind === "rolling" || b.kind === "fixed";
                const onKindChange = (kind: ManualBudgetKind) => {
                  const willNeedWindow = kind === "rolling" || kind === "fixed";
                  // 切到 rolling/fixed 且尚无窗口配置 → 给合理默认（7 天）
                  if (willNeedWindow && (b.window_hours == null || b.window_hours <= 0)) {
                    update({ kind, window_hours: 7, window_unit: "day" });
                  } else {
                    update({ kind });
                  }
                };
                return (
                  <div key={b.id} style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
                    <select
                      className="input"
                      style={{ width: 110, flexShrink: 0 }}
                      value={b.kind}
                      onChange={e => onKindChange(e.target.value as ManualBudgetKind)}
                    >
                      <option value="total">{t("platform.manualBudgetKindTotal", "总额")}</option>
                      <option value="rolling">{t("platform.manualBudgetKindRolling", "滑动窗口")}</option>
                      <option value="fixed">{t("platform.manualBudgetKindFixed", "固定窗口")}</option>
                      <option value="daily">{t("platform.manualBudgetKindDaily", "每日")}</option>
                    </select>
                    <select
                      className="input"
                      style={{ width: 90, flexShrink: 0 }}
                      value={b.unit}
                      onChange={e => update({ unit: e.target.value as ManualBudgetUnit })}
                    >
                      <option value="usd">$ USD</option>
                      <option value="token">{t("platform.manualBudgetUnitToken", "Token")}</option>
                    </select>
                    <input
                      className="input"
                      type="number"
                      min={0}
                      step="any"
                      style={{ width: 100, flexShrink: 0 }}
                      placeholder={t("platform.manualBudgetAmount", "额度")}
                      value={b.amount || ""}
                      onChange={e => update({ amount: parseFloat(e.target.value) || 0 })}
                    />
                    {needsWindow && (
                      <>
                        <input
                          className="input"
                          type="number"
                          min={0}
                          step="any"
                          style={{ width: 80, flexShrink: 0 }}
                          placeholder={t("platform.manualBudgetWindow", "窗口")}
                          value={b.window_hours ?? ""}
                          onChange={e => update({ window_hours: e.target.value === "" ? null : (parseFloat(e.target.value) || 0) })}
                        />
                        <select
                          className="input"
                          style={{ width: 90, flexShrink: 0 }}
                          value={b.window_unit ?? "hour"}
                          onChange={e => update({ window_unit: e.target.value as WindowUnit })}
                        >
                          <option value="minute">{t("platform.windowUnitMinute", "分钟")}</option>
                          <option value="hour">{t("platform.windowUnitHour", "小时")}</option>
                          <option value="day">{t("platform.windowUnitDay", "天")}</option>
                          <option value="week">{t("platform.windowUnitWeek", "周")}</option>
                          <option value="month">{t("platform.windowUnitMonth", "月")}</option>
                        </select>
                      </>
                    )}
                    <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
                      <input
                        type="checkbox"
                        checked={b.enabled}
                        onChange={e => update({ enabled: e.target.checked })}
                      />
                      {t("platform.manualBudgetEnabled", "启用")}
                    </label>
                    <button
                      type="button"
                      className="btn btn-ghost btn-icon btn-danger"
                      style={{ flexShrink: 0 }}
                      title={t("action.delete", "删除")}
                      onClick={() => setManualBudgets(manualBudgets.filter((_, i) => i !== idx))}
                    >
                      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                      </svg>
                    </button>
                  </div>
                );
              })}
            </FormSection>
          )}

          {/* Circuit Breaker 熔断覆盖（仅编辑态可配；空 = 继承全局默认） */}
          {editing && !isPassthrough && (
            <FormSection
              title={t("platform.breakerTitle", "熔断阈值")}
              desc={t("platform.breakerDesc", "连续失败达阈值后临时摘除该平台，冷却后半开探测恢复。留空 = 继承系统设置的全局默认值。")}
            >
              <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: "10px 12px" }}>
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerFailureThreshold", "失败阈值")}</span>
                <input
                  className="input" type="number" min={0} style={{ width: 140 }}
                  placeholder={breakerDefaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(breakerDefaults.breaker_failure_threshold)) : t("platform.breakerInheritGeneric", "继承默认")}
                  value={breakerFailureThreshold}
                  onChange={e => setBreakerFailureThreshold(e.target.value)}
                />
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerOpenSecs", "熔断时长(秒)")}</span>
                <input
                  className="input" type="number" min={0} style={{ width: 140 }}
                  placeholder={breakerDefaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(breakerDefaults.breaker_open_secs)) : t("platform.breakerInheritGeneric", "继承默认")}
                  value={breakerOpenSecs}
                  onChange={e => setBreakerOpenSecs(e.target.value)}
                />
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{t("platform.breakerHalfOpenMax", "半开探测数")}</span>
                <input
                  className="input" type="number" min={0} style={{ width: 140 }}
                  placeholder={breakerDefaults ? t("platform.breakerInherit", "继承默认 {{n}}").replace("{{n}}", String(breakerDefaults.breaker_half_open_max)) : t("platform.breakerInheritGeneric", "继承默认")}
                  value={breakerHalfOpenMax}
                  onChange={e => setBreakerHalfOpenMax(e.target.value)}
                />
              </div>
            </FormSection>
          )}

          {/* 分组归属：是否建默认分组 + 加入已有分组（多选 chips）。
              可同时勾；都不选 = 平台不在任何分组（游离，ensure 永不补建）。 */}
          {!isPassthrough && (
            <FormSection
              title={t("platform.groupAssignTitle", "分组归属")}
              desc={t("platform.groupAssignDesc", "可同时创建默认分组并加入其他已有分组；都不选则该平台不在任何分组。")}
            >
              {lockedGroupId != null ? (
                <div style={{ fontSize: 12, color: "var(--text-secondary)", display: "flex", alignItems: "center", gap: 6 }}>
                  <span className="badge badge-muted" style={{ padding: "0 6px" }}>
                    {groupDetails.find(g => g.group.id === lockedGroupId)?.group.name ?? `#${lockedGroupId}`}
                  </span>
                  {t("platform.groupLocked", "已锁定到此分组")}
                </div>
              ) : !editing ? (
                // 创建默认分组是「创建时一次性判断」，仅创建表单显示；编辑表单不再判断建组。
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
                  <span style={{ fontSize: 13 }}>{t("platform.groupAssignAuto", "创建默认分组")}</span>
                  <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
                    <input type="checkbox" checked={autoGroup} onChange={e => setAutoGroup(e.target.checked)} style={{ display: "none" }} />
                    <span className={`toggle ${autoGroup ? "active" : ""}`} />
                  </label>
                </div>
              ) : null}
              {lockedGroupId == null && groupDetails.length > 0 && (
                <>
                  <div style={{ fontSize: 12, color: "var(--text-secondary)", margin: "10px 0 6px" }}>
                    {t("platform.groupAssignJoin", "加入已有分组")}
                  </div>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                    {groupDetails
                      // 编辑态隐藏该平台自己的 auto 分组（由上方 toggle 管理）。
                      .filter(gd => !editing || gd.group.auto_from_platform !== String(editing.id))
                      .map(gd => {
                        const checked = joinGroupIds.includes(gd.group.id);
                        return (
                          <button
                            key={gd.group.id}
                            type="button"
                            onClick={() => setJoinGroupIds(prev => checked
                              ? prev.filter(id => id !== gd.group.id)
                              : [...prev, gd.group.id])}
                            style={{
                              display: "inline-flex", alignItems: "center",
                              padding: "4px 12px", borderRadius: 999, fontSize: 12, fontWeight: 500,
                              cursor: "pointer",
                              border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                              background: checked ? "var(--accent-subtle)" : "var(--bg-glass)",
                              color: checked ? "var(--accent)" : "var(--text-secondary)",
                              transition: "all 200ms cubic-bezier(0.4, 0, 0.2, 1)",
                            }}
                          >
                            {gd.group.name}
                          </button>
                        );
                      })}
                  </div>
                </>
              )}
              {/* 唯一分组时提供 per-group 优先级设置（复用 Groups 页同款控件）。
                  多分组/零分组不显示——语义上 level_priority 属 group×platform 关联。 */}
              {uniqueGroupInfo.show && (
                <div style={{ marginTop: 12 }}>
                  <LevelPriorityControl value={levelPriority} onChange={setLevelPriority} />
                </div>
              )}
            </FormSection>
          )}

          {/* 过期时间（可选）：设过期后路由自动排除（等效禁用），独立于 status 三态。
              「启用过期」toggle OFF → 隐藏 datetime-local（即便 expiresAt 有识别值也不显示）；
              toggle ON → 显示 datetime-local；ON→OFF 清零 expiresAt（不生效）。
              智能粘贴识别到过期时间时 applyPaste 自动置 expiryEnabled=true，使识别值在表单可见。 */}
          <FormSection
            title={t("platform.expiresAt", "过期时间")}
            desc={t("platform.expiresAtHint", "可选。到期后该平台自动从路由候选排除（等效禁用），改值或清空即恢复。")}
          >
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: expiryEnabled ? 8 : 0 }}>
              <span style={{ fontSize: 13 }}>{t("platform.expiresAtEnable", "启用过期")}</span>
              <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
                <input
                  type="checkbox"
                  checked={expiryEnabled}
                  onChange={e => {
                    const v = e.target.checked;
                    setExpiryEnabled(v);
                    // ON→OFF：清零 expiresAt（不生效）；OFF→ON：保留 expiresAt 若有粘贴识别值（预填）。
                    if (!v) setExpiresAt(0);
                  }}
                  style={{ display: "none" }}
                />
                <span className={`toggle ${expiryEnabled ? "active" : ""}`} />
              </label>
            </div>
            {expiryEnabled && (
              <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <input
                  className="input"
                  type="datetime-local"
                  // colorScheme 控 WKWebView 原生日历弹出层明暗；input 本体 color/bg/border 走 .input 的 CSS 变量。
                  style={{ flex: 1, minWidth: 200, colorScheme: themeMode }}
                  // datetime-local 值为 "YYYY-MM-DDTHH:MM" 本地时间；expiresAt=0 → 空串（未设）。
                  value={expiresAt > 0 ? toDatetimeLocal(expiresAt) : ""}
                  onChange={(e) => {
                    const v = e.target.value;
                    if (!v) { setExpiresAt(0); return; }
                    // 本地时间 "YYYY-MM-DDTHH:MM" → 毫秒时间戳（new Date 按本地时区解析）。
                    const ms = new Date(v).getTime();
                    if (Number.isFinite(ms) && ms > 0) setExpiresAt(ms);
                  }}
                />
                {expiresAt > 0 && (
                  <button
                    type="button"
                    className="btn btn-ghost"
                    style={{ fontSize: 12, padding: "4px 10px" }}
                    onClick={() => setExpiresAt(0)}
                  >
                    {t("platform.expiresAtClear", "清空")}
                  </button>
                )}
                {expiresAt > 0 && (
                  <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                    {(() => {
                      const nowMs = Date.now();
                      if (nowMs >= expiresAt) {
                        return t("platform.expired", "已过期");
                      }
                      const inDay = expiresAt - nowMs < 86_400_000;
                      const txt = new Date(expiresAt).toLocaleString();
                      return inDay
                        ? t("platform.expiresAtSoon", "临近过期：{{time}}", { time: txt })
                        : txt;
                    })()}
                  </span>
                )}
              </div>
            )}
          </FormSection>

          {/* Claude Code Config */}
          {editing && (
            <FormSection title={t("settings.claudeCodeConfig")}>
              <button
                type="button"
                className="btn btn-ghost"
                style={{
                  width: "100%",
                  justifyContent: "space-between",
                  fontSize: 12,
                  padding: "6px 4px",
                  color: "var(--text-secondary)",
                }}
                onClick={() => setShowClaudeConfig(!showClaudeConfig)}
              >
                <span style={{ fontWeight: 600 }}>{t("settings.claudeConfigToggle", "Config Override")}</span>
                <span style={{ opacity: 0.5 }}>{showClaudeConfig ? "▾" : "▸"}</span>
              </button>
              {showClaudeConfig && (
                <div className="animate-fade-in" style={{ marginTop: 6 }}>
                  <textarea
                    className="input"
                    style={{
                      fontFamily: '"SF Mono", "Fira Code", monospace',
                      fontSize: 12,
                      lineHeight: 1.6,
                      minHeight: 180,
                      resize: "vertical",
                      whiteSpace: "pre",
                    }}
                    value={claudeConfigJson}
                    onChange={(e) => setClaudeConfigJson(e.target.value)}
                    spellCheck={false}
                  />
                  <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4, lineHeight: 1.5 }}>
                    {t("settings.platformConfigHint")}
                  </div>
                  {(() => {
                    try {
                      const merged = JSON.parse(claudeConfigJson);
                      const overridden = Object.keys(merged).filter(
                        k => JSON.stringify(merged[k]) !== JSON.stringify(globalClaudeConfig[k]),
                      );
                      return overridden.length > 0 ? (
                        <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
                          {overridden.map(k => (
                            <span key={k} className="badge badge-accent" style={{ fontSize: 10 }}>
                              {k}
                            </span>
                          ))}
                        </div>
                      ) : (
                        <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>
                          {t("settings.allAligned")}
                        </div>
                      );
                    } catch { return null; }
                  })()}
                </div>
              )}
            </FormSection>
          )}

          {/* Middleware rules (platform scope) — 需已有 platform_id */}
          {editing && (
            <FormSection
              title={t("middleware.platformRules", "平台中间件规则")}
              desc={t("middleware.platformRulesHint", "仅本平台生效，就近覆盖分组 / 全局同类型规则")}
            >
              <MiddlewareRulesPanel scope="platform" scopeRef={String(editing.id)} embedded />
            </FormSection>
          )}

          {saveError && (
            <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>
              {saveError}
            </div>
          )}
        </div>
      </div>
    );
  }

  // ── List view ──
  return (
    <>
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Header */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("page.platforms")}</div>
          <div className="section-desc">
            {headerTotal > 0 ? `${headerActive} / ${headerTotal} active` : t("platform.empty")}
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <input
            className="input"
            placeholder={t("platform.searchPlaceholder", "搜索平台...")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            style={{ width: 180, fontSize: 13 }}
          />
          <button className="btn btn-primary" onClick={() => openCreateGroupRef.current?.()}>
            + {t("group.add", "添加分组")}
          </button>
          <button className="btn btn-primary" onClick={() => { resetForm(); setShowForm(true); }}>
            + {t("platform.add")}
          </button>
          <button
            className="btn btn-ghost"
            onClick={async () => {
              if (!window.confirm(t("platform.purgeDisabledConfirm", "将永久删除全库失效(自动禁用)平台，不可恢复，确定？"))) return;
              try {
                const r = await platformApi.purgeDisabled();
                if (r.deletedIds.length === 0) {
                  setToast({ text: t("platform.purgeDisabledNone", "暂无失效平台"), ok: true });
                } else {
                  setToast({ text: t("platform.purgeDisabledDone", "已删除 {{count}} 个失效平台", { count: r.deletedIds.length }), ok: true });
                }
                setTimeout(() => setToast(null), 3000);
                // 局部刷新：按 deletedIds 批量移除被永久删除的平台（不整页 load）；
                // unassignedIds（仅移除分组关联，平台行保留）的归属变化由 handleGroupsChanged 重建 membership。
                if (r.deletedIds.length > 0) {
                  const del = new Set(r.deletedIds);
                  platformsEpochRef.current++;
                  setPlatforms(prev => prev.filter(x => !del.has(x.id)));
                }
                handleGroupsChanged();
                groupsReloadRef.current?.();
              } catch (err) {
                setToast({ text: `${t("platform.purgeDisabled", "清理失效平台")}: ${err}`, ok: false });
                setTimeout(() => setToast(null), 3000);
              }
            }}
            title={t("platform.purgeDisabled", "清理失效平台")}
          >
            {t("platform.purgeDisabled", "清理失效平台")}
          </button>
        </div>
      </div>

      {/* 分组段（内嵌） */}
      <GroupsEmbedded onNavigate={onNavigate} onGroupsChanged={handleGroupsChanged} onCreatePlatform={openCreatePlatform} onEditPlatform={handleEdit} onDuplicatePlatform={handleDuplicate} onToast={setToast} onViewModeChange={setGroupFullscreen} openCreateGroupRef={openCreateGroupRef} reloadRef={groupsReloadRef} onCountChange={setProgressiveCount} searchQuery={searchQuery} />

      {/* 全屏视图态（创建/编辑分组）时隐藏分隔线 + 未分组平台列表，避免与全屏视图并列 */}
      {!groupFullscreen && (<>
      {/* 分隔线 */}
      <div style={{ height: 1, background: "var(--border)", margin: "0 0 10px 0" }} />

      {/* Platform List */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : (
        <div ref={platListRef} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {platforms.length === 0 && (
            <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
              <div className="text-tertiary" style={{ fontSize: 13 }}>{t("platform.empty")}</div>
            </div>
          )}
          {standalonePlatforms.map((p, i) => {
            const isDragging = platDrag?.from === i;
            const draggedPlat = platDrag ? standalonePlatforms[platDrag.from] : null;
            const draggedColor = draggedPlat ? (PROTOCOL_COLORS[draggedPlat.platform_type] || "var(--accent)") : "";
            return (
              <React.Fragment key={p.id}>
                {/* Ghost card at insertion point */}
                {platDrag && platDrag.to === i && draggedPlat && (
                  <div style={{
                    display: "flex", alignItems: "center", gap: 14, paddingLeft: 44,
                    padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                    background: "var(--glass-bg, rgba(255,255,255,0.06))",
                    border: "1.5px dashed var(--accent)",
                    opacity: 0.5, filter: "grayscale(0.8)",
                    pointerEvents: "none", transition: "all 150ms ease",
                  }}>
                    <div style={{ width: 10, height: 10, borderRadius: "50%", background: draggedColor, flexShrink: 0 }} />
                    <span style={{ fontSize: 13, fontWeight: 600 }}>{draggedPlat.name}</span>
                    <span className="badge badge-muted" style={{ fontSize: 10 }}>{PROTOCOL_LABELS[draggedPlat.platform_type] || draggedPlat.platform_type}</span>
                  </div>
                )}
                {/* 未分组平台 pointer 拖拽加入分组（按住卡片空白区拖到分组）；HTML5 DnD 跨区域在 WKWebView 失效故用 pointer events */}
                <div
                  onPointerDown={(e) => onStandaloneGroupPointerDown(e, p)}
                  onPointerMove={onStandaloneGroupPointerMove}
                  onPointerUp={onStandaloneGroupPointerUp}
                  style={{ cursor: groupDrag?.pid === p.id ? "grabbing" : undefined }}
                >
                <PlatformCard
                  platform={p}
                  index={i}
                  isDragging={isDragging}
                  dragActive={!!platDrag}
                  quotaRaw={quotaMap[p.id]}
                  quotaPreferReal={!!quotaRealIds[p.id]}
                  refreshing={!!quotaRefreshing[p.id]}
                  quotaPending={!!quotaPending[p.id]}
                  usagePending={usageLoading && !usageMap[p.id]}
                  usage={usageMap[p.id]}
                  expanded={expandedIds.has(p.id)}
                  manualResult={testResults[p.id]}
                  testing={testingId === p.id}
                  faviconFailed={faviconFailed.has(p.id)}
                  actions={cardActions}
                  platformMembership={platformMembership.get(p.id)}
                  lastTest={lastTestMap[p.id]}
                />
                </div>
              </React.Fragment>
            );
          })}
          {platDrag && (() => {
            if (platDrag.to !== standalonePlatforms.length) return null;
            const dp = standalonePlatforms[platDrag.from];
            const dc = PROTOCOL_COLORS[dp.platform_type] || "var(--accent)";
            return (
              <div style={{
                display: "flex", alignItems: "center", gap: 14, paddingLeft: 44,
                padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                background: "var(--glass-bg, rgba(255,255,255,0.06))",
                border: "1.5px dashed var(--accent)",
                opacity: 0.5, filter: "grayscale(0.8)",
                pointerEvents: "none", transition: "all 150ms ease",
              }}>
                <div style={{ width: 10, height: 10, borderRadius: "50%", background: dc, flexShrink: 0 }} />
                <span style={{ fontSize: 13, fontWeight: 600 }}>{dp.name}</span>
                <span className="badge badge-muted" style={{ fontSize: 10 }}>{PROTOCOL_LABELS[dp.platform_type] || dp.platform_type}</span>
              </div>
            );
          })()}
        </div>
      )}
      </>)}
    </div>

      {/* Custom test overlay — ModelTestPanel 自带 overlay 且经 createPortal 挂 body, 此处不再包外层遮罩。 */}
      {testingPlatform !== null && (
        <ModelTestPanel
          platform={testingPlatform as Platform}
          onClose={() => setTestingPlatform(null)}
          onResult={(success) => { if (testingPlatform) setTestResults(prev => ({ ...prev, [testingPlatform.id]: success ? "ok" : "fail" })); }}
        />
      )}

      {/* Test result toast — Portal 到 body, 脱离页面 transform 祖先(animate-fade-in 等)确保 fixed 相对窗口顶部 */}
      {groupDrag && createPortal(
        <div style={{
          position: "fixed", left: groupDrag.x + 14, top: groupDrag.y + 14,
          pointerEvents: "none", zIndex: 3000,
          padding: "6px 12px", borderRadius: 8,
          background: "var(--accent)", color: "#fff",
          fontSize: 12, fontWeight: 600,
          boxShadow: "0 4px 12px rgba(0,0,0,0.35)", opacity: 0.92,
        }}>
          {groupDrag.pname}
        </div>,
        document.body,
      )}
      {shareData && (
        <ShareModal
          share={shareData.share}
          title={shareData.name}
          onToast={(text, ok) => { setToast({ text, ok }); setTimeout(() => setToast(null), 3000); }}
          onClose={() => setShareData(null)}
        />
      )}
      {toast && createPortal(
        <div style={{
          position: "fixed", top: 24, left: "50%", transform: "translateX(-50%)",
          zIndex: 2000, pointerEvents: "none",
          padding: "10px 20px", borderRadius: 10,
          background: toast.ok ? "var(--color-success, #22c55e)" : "var(--color-danger, #ef4444)",
          color: "#fff", fontSize: 13, fontWeight: 600,
          boxShadow: "0 4px 20px rgba(0,0,0,0.25)",
          opacity: 0.95,
          transition: "opacity 0.3s",
        }}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>{toast.ok ? <IconCheck size={14} color="#fff" /> : <IconClose size={14} color="#fff" />} {toast.text}</span>
        </div>,
        document.body,
      )}
    </>
  );
}

