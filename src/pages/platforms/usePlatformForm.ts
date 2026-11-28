// usePlatformForm — Platforms 编辑/新建表单 state + handlers（从 usePlatformsState 抽出）。
// ponytail: 表单 state 与 handlers 是内聚单元（resetForm/handleEdit/handleSave/applyPaste 共享
//   ~30 个 form useState），独立成 hook 使 usePlatformsState 行数可控。list 侧依赖（platforms/
//   setPlatforms/quota/handleGroupsChanged 等）经 listDeps 注入，保持闭包共享正确。
//
// 边界（自包含）：仅持有 form 编辑态 + form 业务 handlers；不持有 list state（platforms/drag/quota）。
//   owner 调用 usePlatformForm(listDeps) 拿 form state + handlers，并入 PlatformsState 返回。
import { useState, useMemo, useEffect, useRef, useReducer } from "react";
import type { TFunction } from "i18next";
import {
  platformApi, groupDetailApi, parseSubscriptionStartDate, validSubscriptionStartDate,
  parseMockConfig, parseDevinConfig, parsePassthroughAuth, readRequiresValue,
  validateClientUaRule, rateLimitRuleError,
  type Platform, type Protocol, type ModelSlot, type PlatformEndpoint,
  type PlatformUsageStats, type LastTestResult,
  type SchedulingBreakerSettings, type GroupDetail, type SharePlatform,
  type FetchModelsError,
} from "../../services/api";
import { customSlotsApi, type CustomSlotDef } from "../../services/api/customSlots";
import { splitApiKeys } from "../../utils/platformPaste";
import { type SmartPasteApplyResult } from "../../components/platforms/SmartPasteModal";
import {
  PROTOCOL_LABELS, MODEL_SLOTS, DEDICATED_MODEL_SLOTS, DEFAULT_NAMES,
  getDefaultEndpoints, getDefaultModels, getDefaultQuotaScripts, type QuotaScriptVariant,
  autoCategorize,
} from "../../domains/platforms";
import { useProtocolLabelMap } from "../../domains/platforms/useProtocolMeta";
import { getPrimaryBaseUrl } from "./usePlatformQuota";
import { QUOTA_CUSTOM_VARIANT } from "./formSections";
import { slotsToForm, slotList } from "../../domains/platforms/slotValue";
import {
  draftReducer, emptyDraft, draftFromPlatform, draftToPayload,
  type PlatformDraft,
} from "../../domains/platforms/platformDraft";
import { applyPaste as applyPasteImpl, runBatchCreateFromPaste as runBatchCreateFromPasteImpl, previewBatchNames, type PlatformPasteCtx } from "./platformPasteApply";
import { notifyPlatformMembershipChanged } from "./groupsChanged";
import { getProxyHostConfigured } from "../../domains/platforms/proxyConfig";

/** owner（usePlatformsState）注入的 list 侧依赖。所有 form handler 需要的 list state/setters 走此通道。 */
export interface PlatformFormListDeps {
  t: TFunction;
  platforms: Platform[];
  setPlatforms: React.Dispatch<React.SetStateAction<Platform[]>>;
  platformsEpochRef: React.MutableRefObject<number>;
  /** quota 子系统引用（局部刷新保存/批量新建后补查余额）。 */
  quota: {
    scheduleQuotaFor: (p: Platform) => void;
  };
  groupDetails: GroupDetail[];
  setGroupDetails: React.Dispatch<React.SetStateAction<GroupDetail[]>>;
  /** 全局 toast setter（保存/批量/错误共用）。 */
  setToast: React.Dispatch<React.SetStateAction<{ text: string; ok: boolean } | null>>;
  /** 全局调度+熔断默认（展示「继承默认 N」用），owner effect 异步拉取后 setBreakerDefaults 注入。 */
  breakerDefaults: SchedulingBreakerSettings | null;
  setUsageMap: React.Dispatch<React.SetStateAction<Record<number, PlatformUsageStats>>>;
  setLastTestMap: React.Dispatch<React.SetStateAction<Record<number, LastTestResult>>>;
  /** 外部导航（分组区点编辑跳本页）。handleViewLogs 用。 */
  onNavigate?: (id: string, context?: { platformId?: number; platformName?: string; duplicate?: boolean }) => void;
  /** resetForm 复位「已消费外部编辑 pid」ref（避免同一平台二次编辑被短路）。 */
  consumedEditPidRef: React.MutableRefObject<number | null>;
}

export interface PlatformFormState {
  editing: Platform | null;
  setEditing: React.Dispatch<React.SetStateAction<Platform | null>>;
  showForm: boolean;
  setShowForm: React.Dispatch<React.SetStateAction<boolean>>;
  showPaste: boolean;
  setShowPaste: React.Dispatch<React.SetStateAction<boolean>>;
  pasteInitialText: string | undefined;
  setPasteInitialText: React.Dispatch<React.SetStateAction<string | undefined>>;
  shareData: { share: SharePlatform; name: string } | null;
  setShareData: React.Dispatch<React.SetStateAction<{ share: SharePlatform; name: string } | null>>;
  groupFullscreen: boolean;
  setGroupFullscreen: React.Dispatch<React.SetStateAction<boolean>>;
  showKey: boolean;
  setShowKey: React.Dispatch<React.SetStateAction<boolean>>;
  testingPlatform: Platform | null;
  setTestingPlatform: React.Dispatch<React.SetStateAction<Platform | null>>;
  fetching: boolean;
  setFetching: React.Dispatch<React.SetStateAction<boolean>>;
  fetchError: string;
  setFetchError: React.Dispatch<React.SetStateAction<string>>;
  /** 一键获取 info 提示（非错误）：已保留手填模型 / 候选已更新 */
  fetchInfo: string;
  saveError: string;
  setSaveError: React.Dispatch<React.SetStateAction<string>>;
  // ── 草稿单一入口（票 14 终局）：控件读 draft.xxx、写 patch({xxx})，兼容 setter 层已删 ──
  draft: PlatformDraft;
  /** 草稿部分更新（patch 只覆盖给出的键；reducer 纯函数）。 */
  patch: (p: Partial<PlatformDraft>) => void;
  /** 多 key 预览态：null = 非批量；string[] = 待确认的批量 key 列表（触发 MultiKeyPreview 渲染）。
   *  创建态 + 非 keyOptional + splitApiKeys(apiKey).length>1 时由 handleApiKeyChange 设置；
   *  编辑态 / keyOptional / 单 key 均 null（不触发预览）。 */
  batchPreviewKeys: string[] | null;
  setBatchPreviewKeys: React.Dispatch<React.SetStateAction<string[] | null>>;
  /** apiKey input onChange 包装：创建态多 key → setBatchPreviewKeys 触发实时预览（D1）。 */
  handleApiKeyChange: (v: string) => void;
  /** 预览 name 列表（batchPreviewKeys 派生，供 MultiKeyPreview 渲染；null 时空数组）。 */
  previewNames: string[];
  activeDropdown: ModelSlot | null; setActiveDropdown: React.Dispatch<React.SetStateAction<ModelSlot | null>>;
  /** 配额查询脚本（quota-scripts T6）：registry 变体列表（protocol 派生异步拉取）。 */
  quotaVariants: QuotaScriptVariant[];
  /** 变体下拉切换（切回 registry 变体时清自定义正文，保持互斥）。 */
  handleQuotaVariantChange: (v: string) => void;
  /** 配额方式显式切换器：patch quotaSource + 关 pristine（自动落位只发生在 pristine 新建态）。 */
  setQuotaSourceExplicit: (v: "auto" | "manual") => void;
  breakerDefaults: SchedulingBreakerSettings | null;
  /** windowsTz：时段档展示态（local/utc），只影响表单展示、不写 extra。 */
  windowsTz: "local" | "utc";
  setWindowsTz: React.Dispatch<React.SetStateAction<"local" | "utc">>;
  /** 「启用过期」toggle（展示态；expiresAt 在 draft）。 */
  expiryEnabled: boolean; setExpiryEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  /** 全局自定义槽位定义（null = 未加载，此时保存不清孤儿键）。 */
  customSlotDefs: CustomSlotDef[] | null;
  isMock: boolean;
  isPassthrough: boolean;
  /** 自定义透传协议（passthrough，非 claude_code 订阅透传）：专属编辑器门 + 模型槽/测试全隐藏。 */
  isPassThrough: boolean;
  keyOptional: boolean;
  apiKeyMissing: boolean;
  resetForm: () => void;
  openCreatePlatform: (presetGroupIds?: number[], lockGid?: number) => void;
  handleEdit: (p: Platform) => Promise<void>;
  handleDuplicate: (p: Platform) => Promise<void>;
  handleProtocolChange: (newProtocol: Protocol, newCodingPlan?: boolean) => void | Promise<void>;
  handleModelsChange: (slot: ModelSlot, chain: string[]) => void;
  handleFetchModels: () => Promise<void>;
  handleFillAll: () => void;
  buildModelsPayload: () => Record<string, string[] | undefined> | undefined;
  handleSave: () => Promise<void>;
  handleViewLogs: (p: Platform) => void;
  applyPaste: (r: SmartPasteApplyResult) => Promise<void>;
  runBatchCreateFromPaste: (keys: string[], baseName?: string, effectiveEndpoints?: PlatformEndpoint[], effectiveProtocol?: Protocol) => Promise<void>;
}

export function usePlatformForm(listDeps: PlatformFormListDeps): PlatformFormState {
  const {
    t, platforms, setPlatforms, platformsEpochRef, quota,
    setGroupDetails,
    setToast, breakerDefaults, setUsageMap, setLastTestMap,
    onNavigate, consumedEditPidRef,
  } = listDeps;

  const [editing, setEditing] = useState<Platform | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [showPaste, setShowPaste] = useState(false);
  // aidog://platform/import deep-link 导入：SmartPasteModal 预填文本（来自 URL ?data=<base64>）。
  // 非空时弹窗以之初始化并跳过自动读剪贴板；null = 正常手动/剪贴板路径。
  const [pasteInitialText, setPasteInitialText] = useState<string | undefined>(undefined);
  // 平台分享弹窗：导出成功后持有 { share, name } 渲染 ShareModal（含明文 api_key + 格式切换）。
  const [shareData, setShareData] = useState<{ share: SharePlatform; name: string } | null>(null);
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState("");
  const [fetchInfo, setFetchInfo] = useState("");
  const [saveError, setSaveError] = useState("");
  // GroupsEmbedded 进入全屏视图态（创建/编辑分组）时为 true：隐藏下方分隔线 + 未分组平台列表，避免与全屏视图并列。
  const [groupFullscreen, setGroupFullscreen] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [testingPlatform, setTestingPlatform] = useState<Platform | null>(null);

  // ── 表单纯数据 state：单一 draft + reducer（票 12 形状 B；票 14 终局对外 draft + patch） ──
  const [draft, dispatch] = useReducer(draftReducer, undefined, emptyDraft);
  const draftRef = useRef(draft);
  draftRef.current = draft;
  /** 草稿部分更新：s.form 对草稿的唯一写入口（patch 只覆盖给出的键）。 */
  const patch = (p: Partial<PlatformDraft>) => dispatch({ type: "patch", patch: p });
  // hook 内部 handler 的单字段函数式更新助手（不对外暴露；控件一律走 patch）。
  const patchOf = <K extends keyof PlatformDraft>(k: K) => (v: React.SetStateAction<PlatformDraft[K]>) =>
    dispatch({ type: "patch", patch: { [k]: typeof v === "function" ? (v as (prev: PlatformDraft[K]) => PlatformDraft[K])(draftRef.current[k]) : v } });
  const setName = patchOf("name");
  const setProtocol = patchOf("protocol");
  const setCodingPlan = patchOf("codingPlan");
  const setApiKey = patchOf("apiKey");
  const setModels = patchOf("models");
  const setAvailableModels = patchOf("availableModels");
  const setEndpoints = patchOf("endpoints");
  const setMockConfig = patchOf("mockConfig");
  const setDevinConfig = patchOf("devinConfig");
  const setPassAuth = patchOf("passAuth");
  const setQuotaVariantId = patchOf("quotaVariantId");
  const setQuotaCustomScript = patchOf("quotaCustomScript");
  const setQuotaRequires = patchOf("quotaRequires");
  const setQuotaSource = patchOf("quotaSource");
  const setJoinGroupIds = patchOf("joinGroupIds");
  const setAutoGroup = patchOf("autoGroup");
  const setLockedGroupId = patchOf("lockedGroupId");
  const setExpiresAt = patchOf("expiresAt");
  const setProxyEnabled = patchOf("proxyEnabled");
  // handler 内读取的草稿值（解构自当前 draft；handler 每轮渲染重建，闭包即最新值）。
  const {
    name, protocol, apiKey, models, endpoints, extra,
    quotaVariantId,
    proxyEnabled, autoGroup, joinGroupIds, lockedGroupId, expiresAt,
  } = draft;
  // 协议 label 全表（共享 hook）；首帧 {} 用 key fallback。lang 省略 = 沿用原非本地化
  // 行为（表单挂载即定型，无 locale 重拉）。DEFAULT_NAMES 运行时并入此 map 的 value 集合。
  const protocolLabelMap = useProtocolLabelMap();
  // 多 key 预览态：null = 非批量；string[] = 待确认的批量 key 列表。
  // 创建态 + 非 keyOptional + splitApiKeys.length>1 → setBatchPreviewKeys(keys) 触发 MultiKeyPreview。
  const [batchPreviewKeys, setBatchPreviewKeys] = useState<string[] | null>(null);
  const [activeDropdown, setActiveDropdown] = useState<ModelSlot | null>(null);
  // 配额查询脚本（quota-scripts T6）：变体列表随 protocol 异步拉取（docPromise 单次 RPC）；
  //   选择 id / 自定义正文 / requires 值序列化见 buildSharedCreateFields。
  const [quotaVariants, setQuotaVariants] = useState<QuotaScriptVariant[]>([]);
  /** 配额方式 pristine：未被用户显式切换过 → 新建态随协议能力自动落位（有变体→auto，无→manual）。 */
  const [quotaSourcePristine, setQuotaSourcePristine] = useState(true);
  /** 显式切换（UI tab）：一经用户选择即停止自动落位。 */
  const setQuotaSourceExplicit = (v: "auto" | "manual") => {
    dispatch({ type: "patch", patch: { quotaSource: v } });
    setQuotaSourcePristine(false);
  };
  const [customSlotDefs, setCustomSlotDefs] = useState<CustomSlotDef[] | null>(null);
  // 表单挂载与每次打开（新建 / 编辑）时重读全局定义：设置页增删槽位后平台页随之出现 / 消失。
  useEffect(() => {
    if (!showForm) return;
    let alive = true;
    customSlotsApi.get().then((d) => { if (alive) setCustomSlotDefs(d); }).catch((e) => console.error("custom slots definitions load failed:", e));
    return () => { alive = false; };
  }, [showForm]);
  // windowsTz：时段档展示态（local/utc），只影响表单展示、不写 extra（不进 draft）。
  const [windowsTz, setWindowsTz] = useState<"local" | "utc">("local");
  // 「启用过期」toggle：默认 OFF（隐藏 datetime-local）。仅当用户勾选 toggle 才显示日期选择器；
  // 老平台 expires_at>0 加载时置 ON；粘贴识别填入 expiresAt 但 **保持 toggle OFF**（用户手动启用）。
  const [expiryEnabled, setExpiryEnabled] = useState(false);

  const isMock = protocol === "mock";
  // Claude Code 订阅纯透传：客户端自带订阅 OAuth 认证，aidog 原样转发。
  // 仅需 base_url（host 根），api_key 可空，隐藏 endpoints/models 编辑。
  const isPassthrough = protocol === "claude_code";
  // 自定义透传：用户定义 base_url + apikey 注入 header，其余原样转发（共识稿 2026-10-08）。
  const isPassThrough = protocol === "passthrough";
  // OpenCode Zen：免费匿名访问（api_key 留空时 proxy 兜底 $opencode），全程不校验 key 存在。
  const keyOptional = protocol === "opencode_zen";
  // 需要 api_key 但未填（keyOptional 平台不要求）—— fetch/列模型按钮共用的禁用判定。
  const apiKeyMissing = !keyOptional && !apiKey;
  // 协议的 quota_scripts 变体列表（registry → getDefaultsJson；docPromise 单次 RPC 共享）。
  useEffect(() => {
    let cancelled = false;
    getDefaultQuotaScripts(protocol).then(list => {
      if (cancelled) return;
      setQuotaVariants(list);
      // 新建态 + 用户未显式切换过 → 按协议能力落位（票 01：有变体→auto 保持开箱即查，无→manual）。
      if (quotaSourcePristine && !editing) setQuotaSource(list.length > 0 ? "auto" : "manual");
    });
    return () => { cancelled = true; };
  }, [protocol, quotaSourcePristine, editing]);

  // requires 初值回填：变体列表 / 选择 / extra 任一变化后，为选中变体缺失的 requires key
  //   从 extra 读初值（嵌套优先，同脚本取值语义 —— 见 readRequiresValue）。用户已输入的键不覆盖。
  useEffect(() => {
    const sel = quotaVariants.find(v => v.id === quotaVariantId) ?? quotaVariants[0] ?? null;
    const keys = [...(sel?.requires ?? []).map(r => r.key)];
    if (protocol === "newapi" && !keys.includes("user_id")) keys.push("user_id");
    setQuotaRequires(prev => {
      let changed = false;
      const next = { ...prev };
      for (const k of keys) {
        if (!(k in next)) { next[k] = readRequiresValue(extra, k); changed = true; }
      }
      return changed ? next : prev;
    });
  }, [quotaVariants, quotaVariantId, protocol, extra]);

  /** 变体下拉切换：切回 registry 变体时清自定义正文（互斥，序列化规则见 serializeQuotaScriptConfig）。 */
  const handleQuotaVariantChange = (v: string) => {
    setQuotaVariantId(v);
    if (v !== QUOTA_CUSTOM_VARIANT) setQuotaCustomScript("");
  };

  const handleProtocolChange = async (newProtocol: Protocol, newCodingPlan?: boolean) => {
    const cp = !!newCodingPlan;
    const repairingUnknown = editing?.platform_type === "unknown";
    // 未知协议修复只替换协议，保留用户已有配置；正常切换仍按原逻辑填充默认值。
    if (repairingUnknown) {
      setProtocol(newProtocol);
      setCodingPlan(cp);
      return;
    }
    // DEFAULT_NAMES 运行时并入 protocolLabelMap value 集合（覆盖 60+ platform JSON name）
    // + 静态 5 请求格式协议 label；任一命中视为「name 仍是协议默认名 → 切协议时自动覆盖」。
    const label = protocolLabelMap[newProtocol] || PROTOCOL_LABELS[newProtocol] || newProtocol;
    const isDefaultName = !name.trim() || DEFAULT_NAMES.has(name)
      || Object.values(protocolLabelMap).includes(name);
    if (isDefaultName) {
      setName(label);
    }
    // Auto-fill endpoints from defaults（mock 无真实上游，返回空）
    const defaultEps = await getDefaultEndpoints(newProtocol);
    if (defaultEps.length > 0) {
      setEndpoints(defaultEps);
    } else {
      setEndpoints([]);
    }
    // 保留已填写的模型链，仅用预设填充空槽位。
    const defaultModels = slotsToForm(await getDefaultModels(newProtocol));
    setModels(prev => {
      const next = { ...prev };
      for (const slot of MODEL_SLOTS) {
        if (next[slot.key].length === 0) next[slot.key] = defaultModels[slot.key];
      }
      return next;
    });
    // 切到 mock 时用当前 extra 初始化 mock 配置编辑器
    if (newProtocol === "mock") {
      setMockConfig(parseMockConfig(extra));
    }
    // 切到 devin 时用当前 extra 初始化 devin 配置
    if (newProtocol === "devin") {
      setDevinConfig(parseDevinConfig(extra));
    }
    // 切到 passthrough 时用当前 extra 初始化透传认证配置
    if (newProtocol === "passthrough") {
      setPassAuth(parsePassthroughAuth(extra));
    }
    // quota 脚本选择随协议重置（变体列表由上面 protocol effect 重拉；
    //   requires 初值由 requires effect 从 extra 回填）。
    setQuotaVariantId("");
    setQuotaCustomScript("");
    setQuotaRequires({});
    setProtocol(newProtocol);
    setCodingPlan(cp);
  };

  const resetForm = () => {
    // 数据面：draft 整体复位（= load(emptyDraft())）；windowsTz 是展示态，单独复位。
    dispatch({ type: "reset" });
    setWindowsTz("local");
    setBatchPreviewKeys(null);
    setEditing(null); setShowForm(false); setFetchError(""); setSaveError("");
    setQuotaSourcePristine(true);
    setExpiryEnabled(false);
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
    // 使用代理预填（issue #64 T03）：全局代理地址非空默认开、为空默认关；一次性写入 state，
    // 不做渲染期回落（禁 value={x || DEFAULT} 模式）。
    getProxyHostConfigured().then(on => setProxyEnabled(on)).catch(() => setProxyEnabled(false));
    setShowForm(true);
  };

  const handleViewLogs = (p: Platform) => {
    onNavigate?.("logs", { platformId: p.id, platformName: p.name });
  };

  /** 反查平台当前手动组成员（排除其 auto 分组），作为「加入已有分组」初始值。 */
  const loadJoinGroups = async (p: Platform) => {
    try {
      const gds = await groupDetailApi.list();
      setGroupDetails(gds);
      const manualIds = gds
        .filter(gd => gd.group.auto_from_platform !== String(p.id)
          && gd.platforms.some(gp => gp.platform.id === p.id))
        .map(gd => gd.group.id);
      dispatch({ type: "patch", patch: { joinGroupIds: manualIds } });
    } catch {
      dispatch({ type: "patch", patch: { joinGroupIds: [] } });
    }
  };

  const handleEdit = async (p: Platform) => {
    // 三入口水合单一实现 draftFromPlatform（票 12；edit 与 duplicate 字段水合完全一致，
    // spec 5.1 不变量④；透传 endpoints 回填 / 熔断 / 时段 6 键 / extra 派生字段全部在内）。
    dispatch({ type: "load", draft: draftFromPlatform(p, "edit") });
    setEditing(p); setShowForm(true); setFetchError(""); setSaveError("");
    // 配额方式：编辑即视为已定位，不再随协议能力漂移。
    setQuotaSourcePristine(false);
    // 老平台 expires_at>0 → toggle 默认 ON；=0/未设 → OFF。
    setExpiryEnabled((p.expires_at ?? 0) > 0);
    await loadJoinGroups(p);
  };

  /** 复制平台：复用源平台全部配置灌入表单，但以「新建态」打开（editing=null），保存才新建。
   *  与 handleEdit 唯一差异：setEditing(null)（不绑定源平台 id）。 */
  const handleDuplicate = async (p: Platform) => {
    dispatch({ type: "load", draft: draftFromPlatform(p, "duplicate") });
    setEditing(null); setShowForm(true); setFetchError(""); setSaveError("");
    // 配额方式与 handleEdit 一致（票 03 D2）：取源平台值并停用自动落位。
    setQuotaSourcePristine(false);
    setExpiryEnabled((p.expires_at ?? 0) > 0);
    await loadJoinGroups(p);
  };

  const handleModelsChange = (slot: ModelSlot, chain: string[]) => {
    setModels(prev => ({ ...prev, [slot]: chain }));
  };

  /** 一键获取：获取模型列表 + 自动分类 + 持久化
   *  多协议回退链（通用，非 longcat 专属）：
   *  - 按优先级 try 各 endpoint：openai 优先 → 主协议 → 其余已配 endpoint
   *  - 401/403 (Auth) → 鉴权问题，立即 break 回退链 + 鉴权专用文案（鉴权错回退无意义）
   *  - 404 (NotFound) / 其他错 → continue 试下一协议
   *  - 首个成功（返非空 models）即 setAvailableModels + break
   *  - 全部 endpoint 试完仍无结果 → 报最后一条错 */
  const handleFetchModels = async () => {
    // 回退顺序：openai endpoint 在前 → 主协议 endpoint → 其余 endpoint 去重。
    const primaryBase = getPrimaryBaseUrl(protocol, endpoints);
    const openaiEp = endpoints.find(ep => ep.protocol === "openai");
    // opencode_zen /v1/models 无 auth 可列模型，api_key 可留空（后端兜底 $opencode）。
    if (apiKeyMissing) return;
    // 构造有序去重 try 列表：(protocol, base_url)
    const seen = new Set<string>();
    const tryList: { proto: Protocol; url: string }[] = [];
    const push = (proto: Protocol, url: string) => {
      if (!url) return;
      const key = `${proto}|${url}`;
      if (seen.has(key)) return;
      seen.add(key);
      tryList.push({ proto, url });
    };
    if (openaiEp) push("openai", openaiEp.base_url);
    if (primaryBase) push(protocol, primaryBase);
    for (const ep of endpoints) {
      if (ep.protocol === "openai" || ep.protocol === protocol) continue;
      push(ep.protocol as Protocol, ep.base_url);
    }
    if (tryList.length === 0) return;
    setFetching(true); setFetchError(""); setFetchInfo("");
    let lastError: FetchModelsError | null = null;
    let fetched = false;
    try {
      for (const { proto, url } of tryList) {
        try {
          const modelIds = await platformApi.fetchModels(proto, url, apiKey, proxyEnabled);
          if (modelIds.length === 0) {
            // 空列表（200 但无 data）不 break，继续试下一 endpoint，但记录为「可能拉不到」
            continue;
          }
          setAvailableModels(modelIds);
          // 任意槽位已填（用户手填过）→ 不覆写模型，只更新候选并提示；全空才自动填充。
          const hasFilled = MODEL_SLOTS.some(s => models[s.key].length > 0);
          if (hasFilled) {
            setFetchInfo(t("platform.fetchKeptExisting"));
          } else {
            const categorized = autoCategorize(modelIds);
            setModels(slotsToForm(categorized));
          }
          lastError = null;
          fetched = true;
          break;
        } catch (e: any) {
          const err: FetchModelsError | null = e && typeof e === "object" && "kind" in e ? (e as FetchModelsError) : null;
          if (err && err.kind === "Auth") {
            // 401/403 鉴权失败：回退无意义，立即 break 报鉴权专用文案
            lastError = err;
            break;
          }
          // NotFound (404) / Other (网络错 / 5xx) → 记录后 continue 试下一协议
          if (err) lastError = err;
          else lastError = { kind: "Other", code: 0, message: e?.toString?.() ?? String(e) };
        }
      }
      if (lastError) {
        if (lastError.kind === "Auth") {
          setFetchError(t("platform.fetchAuthError", { code: lastError.code }));
        } else {
          setFetchError(lastError.message || t("platform.fetchEmpty"));
        }
      } else if (!fetched) {
        // 回退链全跑完但没成功（所有 endpoint 返空列表）
        setFetchError(t("platform.fetchEmpty"));
      }
    } catch (e: any) {
      setFetchError(e?.toString?.() ?? String(e));
    }
    setFetching(false);
  };

  /** 一键填充：把 default 模型填到所有槽位（覆盖已有值） */
  const handleFillAll = () => {
    const defaultChain = models.default;
    if (defaultChain.length === 0) return;
    setModels(prev => {
      const next = { ...prev };
      for (const slot of MODEL_SLOTS) {
        // jev 是决策槽（/v1/systemone 专用）、embedding / rerank 是专用槽，default 是聊天模型，一键填充不进专用槽
        if (slot.key !== "default" && slot.key !== "jev" && !DEDICATED_MODEL_SLOTS.includes(slot.key)) {
          next[slot.key] = [...defaultChain];
        }
      }
      return next;
    });
  };

  const buildModelsPayload = () => {
    const result: Record<string, string[] | undefined> = {};
    for (const slot of MODEL_SLOTS) {
      const v = slotList(models[slot.key]);
      result[slot.key] = v.length > 0 ? v : undefined;
    }
    // 全空也返回对象（JSON 序列化为 {}）：后端 update 对 models 缺省=保留旧值，undefined 会让清空保存静默失效。
    return result;
  };

  /** 构建 applyPaste/runBatchCreateFromPaste 的 ctx（每次调用刷新引用最新闭包值）。
   *  ponytail: 字段虽多但全是直传，无逻辑；保持与抽前闭包语义一致。
   *  票 13：fullShare 专用 setter 随 draftFromShare + load 落地删除（44 → 26 字段）。 */
  const buildPasteCtx = (): PlatformPasteCtx => ({
    t,
    buildSharedCreateFields,
    name, protocol, endpoints, lockedGroupId, joinGroupIds, autoGroup, expiresAt,
    setApiKey, setModels, setEndpoints,
    setEditing, setFetchError, setSaveError,
    setShowPaste, setShowForm, setExpiresAt, setExpiryEnabled, // setExpiresAt = patchOf("expiresAt") 私有助手
    dispatch,
    setBatchPreviewKeys,
    handleProtocolChange, resetForm,
    platforms, setPlatforms, platformsEpochRef, quota,
    setToast,
  });

  /** apiKey input onChange 包装：创建态 + 非 keyOptional + 多 key → setBatchPreviewKeys 触发实时预览（D1）。
   *  编辑态 / keyOptional / 单 key → null（走原保存路径）。复用 splitApiKeys 拆分。 */
  const handleApiKeyChange = (v: string) => {
    setApiKey(v);
    // passthrough 单 key（共识稿 §2 隐藏多 key），不触发批量创建预览。
    if (!editing && !keyOptional && !isPassThrough) {
      const keys = splitApiKeys(v);
      setBatchPreviewKeys(keys.length > 1 ? keys : null);
    } else {
      setBatchPreviewKeys(null);
    }
  };

  /** 派生预览 name 列表（batchPreviewKeys → previewBatchNames，供 MultiKeyPreview 渲染）。
   *  baseName 取 name（手动表单）或 preset label（智能粘贴路径已 setName）；撞名基准 = 当前 platforms。 */
  const previewNames = useMemo(() => {
    if (!batchPreviewKeys || batchPreviewKeys.length === 0) return [];
    const used = new Set(platforms.map(p => p.name));
    return previewBatchNames(batchPreviewKeys, name, used);
  }, [batchPreviewKeys, name, platforms]);

  /** 构建创建 payload 的共享字段（除 name/api_key 外全部）：单平台保存与多 key 批量创建共用，
   *  保证批量创建的每个平台与表单配置完全一致（仅 token 不同），而非各用默认配置。
   *  update 路径也复用（api_key 由 caller 覆盖；auto_group 仅 create 语义）。
   *  序列化单一实现在 platformDraft.draftToPayload（票 12）；此处只保留交互校验
   *  （unknown 协议）与本地化错误文案预检（draftToPayload 内同校验抛英文串，文案走这里）。 */
  const buildSharedCreateFields = () => {
    if (protocol === "unknown") {
      throw new Error(t("platform.unknownProtocolSave", "Select a valid protocol before saving"));
    }
    if (protocol === "qianfan_coding" && !validSubscriptionStartDate(parseSubscriptionStartDate(extra))) {
      throw new Error(t("platform.subscriptionDateInvalid", "Enter a valid date no later than today in Beijing."));
    }
    if (draft.uaRules.some(r => validateClientUaRule(r))) {
      throw new Error(t("platform.client_ua_rules_invalid", "Invalid UA rule: pattern must not be empty and regex must be valid"));
    }
    if (draft.rateLimits.rate_limits?.rules?.some(r => rateLimitRuleError(r))) {
      throw new Error(t("platform.rateLimits.rulesInvalid", "Invalid rate-limit rule: each rule needs at least one model or a time window; wildcard must be a single trailing *"));
    }
    return draftToPayload(draft, { quotaVariants, customSlotDefs });
  };

  /** 确认批量创建（MultiKeyPreview 确认按钮）：复用 runBatchCreateFromPaste → 成功后 resetForm + 关表单。
   *  cancel 由 runBatchCreateFromPaste 内部 toast 反馈；本函数仅负责派发，不重复 toast。 */
  const confirmBatchCreate = async () => {
    if (!batchPreviewKeys || batchPreviewKeys.length === 0) return;
    const keys = batchPreviewKeys;
    setBatchPreviewKeys(null);
    await runBatchCreateFromPaste(keys);
  };

  /** 智能识别弹窗确认后，将解析结果填入添加表单。
   *  ponytail: 实现抽到 platformPasteApply.ts（控制本文件行数），经 ctx 传 form state/setters +
   *    list 依赖保持闭包语义；本 wrapper 在每次调用时刷新 ctx 引用最新闭包值。 */
  const applyPaste = async (r: SmartPasteApplyResult) => {
    const ctx: PlatformPasteCtx = buildPasteCtx();
    await applyPasteImpl(r, ctx);
  };

  /** 批量创建 N 平台（智能识别多 key 或手动表单多 key 共用）。
   *  ponytail: 实现抽到 platformPasteApply.ts；ctx 每次调用刷新。 */
  const runBatchCreateFromPaste = async (
    keys: string[],
    baseName?: string,
    effectiveEndpoints?: PlatformEndpoint[],
    effectiveProtocol?: Protocol,
  ) => {
    const ctx: PlatformPasteCtx = buildPasteCtx();
    await runBatchCreateFromPasteImpl(keys, ctx, baseName, effectiveEndpoints, effectiveProtocol);
  };

  const handleSave = async () => {
    setSaveError("");
    if (protocol === "unknown") {
      const message = t("platform.unknownProtocolSave", "Select a valid protocol before saving");
      setSaveError(message);
      setToast({ text: message, ok: false });
      return;
    }
    // 多 key 预览态：右上角创建按钮直接批量创建（预览卡只读展示，无独立确认按钮）。
    if (batchPreviewKeys && batchPreviewKeys.length > 1) {
      await confirmBatchCreate();
      return;
    }
    try {
      const shared = buildSharedCreateFields();
      let saved: Platform | undefined;
      const wasEditing = !!editing;
      const { auto_group: _auto, manual_budgets: _mb, ...sharedCore } = shared;
      if (editing) {
        saved = await platformApi.update({
          id: editing.id, name, api_key: apiKey, ...sharedCore,
          // update 语义：手动预算空也全量清空；auto_group 属 create 专属
          manual_budgets: shared.manual_budgets ?? [],
        });
      } else {
        saved = await platformApi.create({
          name, ...shared, api_key: apiKey,
        });
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
        quota.scheduleQuotaFor(savedPlatform);
        // 单项变更后补刷该平台 usage / 最近测试徽章（load() 原本顺带刷，局部路径须自补）。
        platformApi.usageStats(savedPlatform.id)
          .then(u => setUsageMap(prev => ({ ...prev, [savedPlatform.id]: u })))
          .catch(() => {});
        platformApi.lastTestResult(savedPlatform.id)
          .then(r => { if (r) setLastTestMap(prev => ({ ...prev, [savedPlatform.id]: r })); })
          .catch(() => {});
      }
      // 保存可能改变分组归属（join_group_ids / auto_group 建默认组）：单一入口广播一次，
      // 分组页监听端统一重载行 + 重建 membership（票 09，取代旧「三连」）。
      notifyPlatformMembershipChanged();
    } catch (e: any) {
      const msg = e?.toString() || "Unknown error";
      console.error(msg);
      setSaveError(msg);
      // saveError 渲染在长表单底部易被滚出视口（用户感知「无反应」）；额外用全局 toast 即时反馈，禁静默失败。
      setToast({ text: `${t("platform.saveFail", "Save failed")}: ${msg}`, ok: false });
      setTimeout(() => setToast(null), 4000);
    }
  };

  return {
    editing, setEditing, showForm, setShowForm, showPaste, setShowPaste,
    pasteInitialText, setPasteInitialText, shareData, setShareData,
    groupFullscreen, setGroupFullscreen, showKey, setShowKey,
    testingPlatform, setTestingPlatform,
    fetching, setFetching, fetchError, setFetchError, fetchInfo, saveError, setSaveError,
    // 草稿单一入口（票 14 终局）：控件读 draft.xxx、写 patch({xxx})，兼容 setter 层已删。
    draft, patch,
    batchPreviewKeys, setBatchPreviewKeys,
    handleApiKeyChange, previewNames,
    activeDropdown, setActiveDropdown,
    quotaVariants, handleQuotaVariantChange, setQuotaSourceExplicit,
    breakerDefaults,
    windowsTz, setWindowsTz,
    expiryEnabled, setExpiryEnabled,
    customSlotDefs,
    isMock, isPassthrough, isPassThrough, keyOptional, apiKeyMissing,
    resetForm, openCreatePlatform, handleEdit, handleDuplicate, handleProtocolChange,
    handleModelsChange, handleFetchModels, handleFillAll, buildModelsPayload,
    handleSave, handleViewLogs, applyPaste, runBatchCreateFromPaste,
  };
}
