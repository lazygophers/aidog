// usePlatformForm — Platforms 编辑/新建表单 state + handlers（从 usePlatformsState 抽出）。
// ponytail: 表单 state 与 handlers 是内聚单元（resetForm/handleEdit/handleSave/applyPaste 共享
//   ~30 个 form useState），独立成 hook 使 usePlatformsState 行数可控。list 侧依赖（platforms/
//   setPlatforms/quota/handleGroupsChanged 等）经 listDeps 注入，保持闭包共享正确。
//
// 边界（自包含）：仅持有 form 编辑态 + form 业务 handlers；不持有 list state（platforms/drag/quota）。
//   owner 调用 usePlatformForm(listDeps) 拿 form state + handlers，并入 PlatformsState 返回。
import { useState, useMemo } from "react";
import type { TFunction } from "i18next";
import {
  platformApi, settingsApi, groupDetailApi,
  parseMockConfig, serializeMockConfig, parseNewApiConfig, serializeNewApiConfig,
  parsePlatformBreaker, serializePlatformBreaker,
  parsePlatformPeakHours, serializePlatformPeakHours,
  parseDisableDuringPeak, serializeDisableDuringPeak,
  DEFAULT_MOCK_CONFIG, DEFAULT_NEWAPI_CONFIG,
  type Platform, type Protocol, type ModelSlot, type PlatformEndpoint,
  type PlatformUsageStats, type LastTestResult, type MockConfig, type NewApiConfig,
  type ManualBudget, type SchedulingBreakerSettings, type GroupDetail, type SharePlatform,
  type FetchModelsError,
} from "../../services/api";
import { splitApiKeys } from "../../utils/platformPaste";
import { type SmartPasteApplyResult } from "../../components/platforms/SmartPasteModal";
import {
  PROTOCOL_LABELS, MODEL_SLOTS, DEFAULT_NAMES,
  getDefaultEndpoints, getDefaultModels,
  autoCategorize, type PeakWindow,
} from "../../domains/platforms";
import { getPrimaryBaseUrl } from "./usePlatformQuota";
import { applyPaste as applyPasteImpl, runBatchCreateFromPaste as runBatchCreateFromPasteImpl, previewBatchNames, type PlatformPasteCtx } from "./platformPasteApply";

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
  handleGroupsChanged: () => Promise<void>;
  groupsReloadRef: React.MutableRefObject<(() => void) | null>;
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
  saveError: string;
  setSaveError: React.Dispatch<React.SetStateAction<string>>;
  name: string; setName: React.Dispatch<React.SetStateAction<string>>;
  protocol: Protocol; setProtocol: React.Dispatch<React.SetStateAction<Protocol>>;
  codingPlan: boolean; setCodingPlan: React.Dispatch<React.SetStateAction<boolean>>;
  apiKey: string; setApiKey: React.Dispatch<React.SetStateAction<string>>;
  /** 多 key 预览态：null = 非批量；string[] = 待确认的批量 key 列表（触发 MultiKeyPreview 渲染）。
   *  创建态 + 非 keyOptional + splitApiKeys(apiKey).length>1 时由 handleApiKeyChange 设置；
   *  编辑态 / keyOptional / 单 key 均 null（不触发预览）。 */
  batchPreviewKeys: string[] | null;
  setBatchPreviewKeys: React.Dispatch<React.SetStateAction<string[] | null>>;
  /** apiKey input onChange 包装：创建态多 key → setBatchPreviewKeys 触发实时预览（D1）。 */
  handleApiKeyChange: (v: string) => void;
  /** 确认批量创建（MultiKeyPreview 确认按钮）：调 runBatchCreateFromPaste → 成功后 resetForm + 关表单。 */
  confirmBatchCreate: () => Promise<void>;
  /** 取消批量预览：清 batchPreviewKeys 并把 apiKey 清回单值（用户可重新输入）。 */
  cancelBatchPreview: () => void;
  /** 预览 name 列表（batchPreviewKeys 派生，供 MultiKeyPreview 渲染；null 时空数组）。 */
  previewNames: string[];
  models: Record<ModelSlot, string>; setModels: React.Dispatch<React.SetStateAction<Record<ModelSlot, string>>>;
  availableModels: string[]; setAvailableModels: React.Dispatch<React.SetStateAction<string[]>>;
  endpoints: PlatformEndpoint[]; setEndpoints: React.Dispatch<React.SetStateAction<PlatformEndpoint[]>>;
  activeDropdown: ModelSlot | null; setActiveDropdown: React.Dispatch<React.SetStateAction<ModelSlot | null>>;
  showClaudeConfig: boolean; setShowClaudeConfig: React.Dispatch<React.SetStateAction<boolean>>;
  claudeConfigJson: string; setClaudeConfigJson: React.Dispatch<React.SetStateAction<string>>;
  globalClaudeConfig: Record<string, any>; setGlobalClaudeConfig: React.Dispatch<React.SetStateAction<Record<string, any>>>;
  extra: string; setExtra: React.Dispatch<React.SetStateAction<string>>;
  mockConfig: MockConfig; setMockConfig: React.Dispatch<React.SetStateAction<MockConfig>>;
  newApiConfig: NewApiConfig; setNewApiConfig: React.Dispatch<React.SetStateAction<NewApiConfig>>;
  manualBudgets: ManualBudget[]; setManualBudgets: React.Dispatch<React.SetStateAction<ManualBudget[]>>;
  breakerFailureThreshold: string; setBreakerFailureThreshold: React.Dispatch<React.SetStateAction<string>>;
  breakerOpenSecs: string; setBreakerOpenSecs: React.Dispatch<React.SetStateAction<string>>;
  breakerHalfOpenMax: string; setBreakerHalfOpenMax: React.Dispatch<React.SetStateAction<string>>;
  breakerDefaults: SchedulingBreakerSettings | null;
  peakHours: PeakWindow[]; setPeakHours: React.Dispatch<React.SetStateAction<PeakWindow[]>>;
  peakHoursTz: "local" | "utc"; setPeakHoursTz: React.Dispatch<React.SetStateAction<"local" | "utc">>;
  /** disable_during_peak 开关（用户覆盖，存 platform.extra.disable_during_peak；默认 false）。 */
  disableDuringPeak: boolean; setDisableDuringPeak: React.Dispatch<React.SetStateAction<boolean>>;
  autoGroup: boolean; setAutoGroup: React.Dispatch<React.SetStateAction<boolean>>;
  joinGroupIds: number[]; setJoinGroupIds: React.Dispatch<React.SetStateAction<number[]>>;
  levelPriority: number; setLevelPriority: React.Dispatch<React.SetStateAction<number>>;
  expiresAt: number; setExpiresAt: React.Dispatch<React.SetStateAction<number>>;
  expiryEnabled: boolean; setExpiryEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  lockedGroupId: number | null; setLockedGroupId: React.Dispatch<React.SetStateAction<number | null>>;
  isMock: boolean;
  isPassthrough: boolean;
  keyOptional: boolean;
  apiKeyMissing: boolean;
  uniqueGroupInfo: { show: boolean; groupId: number | null; isAuto: boolean };
  resetForm: () => void;
  openCreatePlatform: (presetGroupIds?: number[], lockGid?: number) => void;
  handleEdit: (p: Platform) => Promise<void>;
  handleDuplicate: (p: Platform) => Promise<void>;
  handleProtocolChange: (newProtocol: Protocol, newCodingPlan?: boolean) => void | Promise<void>;
  handleModelChange: (slot: ModelSlot, value: string) => void;
  handleModelSelect: (slot: ModelSlot, value: string) => void;
  handleFetchModels: () => Promise<void>;
  handleFillAll: () => void;
  buildModelsPayload: () => Record<string, string | undefined> | undefined;
  handleSave: () => Promise<void>;
  handleViewLogs: (p: Platform) => void;
  applyPaste: (r: SmartPasteApplyResult) => Promise<void>;
  runBatchCreateFromPaste: (keys: string[], baseName?: string, effectiveEndpoints?: PlatformEndpoint[], effectiveProtocol?: Protocol) => Promise<void>;
}

export function usePlatformForm(listDeps: PlatformFormListDeps): PlatformFormState {
  const {
    t, platforms, setPlatforms, platformsEpochRef, quota,
    groupDetails, setGroupDetails, handleGroupsChanged, groupsReloadRef,
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
  const [saveError, setSaveError] = useState("");
  // GroupsEmbedded 进入全屏视图态（创建/编辑分组）时为 true：隐藏下方分隔线 + 未分组平台列表，避免与全屏视图并列。
  const [groupFullscreen, setGroupFullscreen] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [testingPlatform, setTestingPlatform] = useState<Platform | null>(null);

  const [name, setName] = useState("OpenAI");
  const [protocol, setProtocol] = useState<Protocol>("openai");
  const [codingPlan, setCodingPlan] = useState(false);
  const [apiKey, setApiKey] = useState("");
  // 多 key 预览态：null = 非批量；string[] = 待确认的批量 key 列表。
  // 创建态 + 非 keyOptional + splitApiKeys.length>1 → setBatchPreviewKeys(keys) 触发 MultiKeyPreview。
  const [batchPreviewKeys, setBatchPreviewKeys] = useState<string[] | null>(null);
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
  // peak_hours（用户覆盖，存 platform.extra.peak_hours；时区仅前端态，默认本地）
  const [peakHours, setPeakHours] = useState<PeakWindow[]>([]);
  const [peakHoursTz, setPeakHoursTz] = useState<"local" | "utc">("local");
  // disable_during_peak（用户覆盖，存 platform.extra.disable_during_peak；默认 false）
  const [disableDuringPeak, setDisableDuringPeak] = useState<boolean>(false);
  // 分组归属选项：auto_group（是否建默认分组，默认勾）+ join_group_ids（加入的已有分组）。
  const [autoGroup, setAutoGroup] = useState(true);
  const [joinGroupIds, setJoinGroupIds] = useState<number[]>([]);
  // per-group level_priority 表单态（1~10，默认 5）。仅当平台归属唯一分组时可设。
  const [levelPriority, setLevelPriority] = useState(5);
  // 过期时间（毫秒 unix 时间戳，0 = 永不过期）。路由候选排除的独立维度（不改 status 三态）。
  const [expiresAt, setExpiresAt] = useState(0);
  // 「启用过期」toggle：默认 OFF（隐藏 datetime-local）。仅当用户勾选 toggle 才显示日期选择器；
  // 老平台 expires_at>0 加载时置 ON；粘贴识别填入 expiresAt 但 **保持 toggle OFF**（用户手动启用）。
  const [expiryEnabled, setExpiryEnabled] = useState(false);
  // 锁定分组：从某分组 ➕ 触发创建平台时，预绑该分组且禁止修改归属。
  const [lockedGroupId, setLockedGroupId] = useState<number | null>(null);

  const isMock = protocol === "mock";
  // Claude Code 订阅纯透传：客户端自带订阅 OAuth 认证，aidog 原样转发。
  // 仅需 base_url（host 根），api_key 可空，隐藏 endpoints/models 编辑。
  const isPassthrough = protocol === "claude_code";
  // OpenCode Zen：免费匿名访问（api_key 留空时 proxy 兜底 $opencode），全程不校验 key 存在。
  const keyOptional = protocol === "opencode_zen";
  // 需要 api_key 但未填（keyOptional 平台不要求）—— fetch/列模型按钮共用的禁用判定。
  const apiKeyMissing = !keyOptional && !apiKey;
  // 唯一分组判定：平台最终归属恰好一个分组时，表单提供 level_priority 设置。
  const uniqueGroupInfo = useMemo(() => {
    if (isPassthrough) return { show: false, groupId: null as number | null, isAuto: false };
    if (editing) {
      const autoGd = groupDetails.find(gd => gd.group.auto_from_platform === String(editing.id));
      const total = (autoGd ? 1 : 0) + joinGroupIds.length;
      if (total === 1) return { show: true, groupId: autoGd ? autoGd.group.id : joinGroupIds[0], isAuto: false };
      return { show: false, groupId: null as number | null, isAuto: false };
    }
    if (lockedGroupId != null) return { show: true, groupId: lockedGroupId, isAuto: false };
    const joinCount = joinGroupIds.length;
    if (autoGroup && joinCount === 0) return { show: true, groupId: null as number | null, isAuto: true };
    if (!autoGroup && joinCount === 1) return { show: true, groupId: joinGroupIds[0], isAuto: false };
    return { show: false, groupId: null as number | null, isAuto: false };
  }, [isPassthrough, editing, groupDetails, lockedGroupId, autoGroup, joinGroupIds]);

  const handleProtocolChange = async (newProtocol: Protocol, newCodingPlan?: boolean) => {
    const cp = !!newCodingPlan;
    // Auto-fill name with protocol label if empty or still at a default name
    if (!name.trim() || DEFAULT_NAMES.has(name)) {
      setName(cp ? `${PROTOCOL_LABELS[newProtocol]} Coding Plan` : PROTOCOL_LABELS[newProtocol]);
    }
    // Auto-fill endpoints from defaults（mock 无真实上游，返回空）
    const defaultEps = await getDefaultEndpoints(newProtocol, cp);
    if (defaultEps.length > 0) {
      setEndpoints(defaultEps);
    } else {
      setEndpoints([]);
    }
    // Auto-fill 默认模型预设（与 endpoints 同步随协议切换）。
    // 仅填预设有值的槽位，其余保持空；未覆盖平台返回空对象 = 不改动。
    const defaultModels = await getDefaultModels(newProtocol, cp);
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

  const resetForm = () => {
    setName(""); setProtocol("openai"); setCodingPlan(false); setApiKey("");
    setBatchPreviewKeys(null);
    setModels({ default: "", sonnet: "", opus: "", haiku: "", gpt: "" });
    setAvailableModels([]); setEndpoints([]);
    setEditing(null); setShowForm(false); setFetchError(""); setSaveError("");
    setShowClaudeConfig(false); setClaudeConfigJson("");
    setExtra(""); setMockConfig({ ...DEFAULT_MOCK_CONFIG });
    setNewApiConfig({ ...DEFAULT_NEWAPI_CONFIG });
    setManualBudgets([]);
    setBreakerFailureThreshold(""); setBreakerOpenSecs(""); setBreakerHalfOpenMax("");
    setPeakHours([]); setPeakHoursTz("local"); setDisableDuringPeak(false);
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
    setPeakHours(parsePlatformPeakHours(p.extra ?? ""));
    setDisableDuringPeak(parseDisableDuringPeak(p.extra ?? ""));
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
    setPeakHours(parsePlatformPeakHours(p.extra ?? ""));
    setDisableDuringPeak(parseDisableDuringPeak(p.extra ?? ""));
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
    setFetching(true); setFetchError("");
    let lastError: FetchModelsError | null = null;
    let fetched = false;
    try {
      for (const { proto, url } of tryList) {
        try {
          const modelIds = await platformApi.fetchModels(proto, url, apiKey);
          if (modelIds.length === 0) {
            // 空列表（200 但无 data）不 break，继续试下一 endpoint，但记录为「可能拉不到」
            continue;
          }
          setAvailableModels(modelIds);
          const categorized = autoCategorize(modelIds);
          setModels(categorized);
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

  /** 构建 applyPaste/runBatchCreateFromPaste 的 ctx（每次调用刷新引用最新闭包值）。
   *  ponytail: 字段虽多但全是直传，无逻辑；保持与抽前闭包语义一致。 */
  const buildPasteCtx = (): PlatformPasteCtx => ({
    t,
    name, protocol, endpoints, lockedGroupId, joinGroupIds, autoGroup, expiresAt,
    setName, setProtocol, setApiKey, setCodingPlan, setModels, setAvailableModels,
    setEndpoints, setManualBudgets, setExtra, setMockConfig, setNewApiConfig,
    setBreakerFailureThreshold, setBreakerOpenSecs, setBreakerHalfOpenMax,
    setEditing, setLockedGroupId, setJoinGroupIds,
    setShowClaudeConfig, setClaudeConfigJson, setFetchError, setSaveError,
    setShowPaste, setShowForm, setExpiresAt, setExpiryEnabled,
    setBatchPreviewKeys,
    handleProtocolChange, resetForm,
    platforms, setPlatforms, platformsEpochRef, quota,
    handleGroupsChanged, groupsReloadRef, setToast,
  });

  /** apiKey input onChange 包装：创建态 + 非 keyOptional + 多 key → setBatchPreviewKeys 触发实时预览（D1）。
   *  编辑态 / keyOptional / 单 key → null（走原保存路径）。复用 splitApiKeys 拆分。 */
  const handleApiKeyChange = (v: string) => {
    setApiKey(v);
    if (!editing && !keyOptional) {
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

  /** 确认批量创建（MultiKeyPreview 确认按钮）：复用 runBatchCreateFromPaste → 成功后 resetForm + 关表单。
   *  cancel 由 runBatchCreateFromPaste 内部 toast 反馈；本函数仅负责派发，不重复 toast。 */
  const confirmBatchCreate = async () => {
    if (!batchPreviewKeys || batchPreviewKeys.length === 0) return;
    const keys = batchPreviewKeys;
    setBatchPreviewKeys(null);
    await runBatchCreateFromPaste(keys);
  };

  /** 取消批量预览：清 batchPreviewKeys 并把 apiKey 清回单值首 key（用户可重新输入或继续单 key 保存）。 */
  const cancelBatchPreview = () => {
    setBatchPreviewKeys(null);
    // 留首 key 作单值，方便用户转单平台保存（与原智能粘贴单 key 行为对齐）。
    if (apiKey) {
      const first = splitApiKeys(apiKey)[0];
      if (first) setApiKey(first);
    }
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
    // 多 key 预览态：保存按钮不直接批量创建，引导用户点 MultiKeyPreview 的「确认批量创建」。
    // ponytail: 预览态时禁用保存按钮（PlatformEditForm 渲染判定），此处兜底防回车/快捷键触发。
    if (batchPreviewKeys && batchPreviewKeys.length > 1) {
      setToast({ text: t("platform.batch.previewFirst", "请先确认下方的批量创建预览"), ok: false });
      setTimeout(() => setToast(null), 3000);
      return;
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
      // peak_hours：空数组 → 移除键（无覆盖 → 用 preset 默认）；非空写入。
      extraPayload = serializePlatformPeakHours(extraPayload, peakHours);
      // disable_during_peak：false → 移除键（默认行为）；true → 写入。
      extraPayload = serializeDisableDuringPeak(extraPayload, disableDuringPeak);
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
        quota.scheduleQuotaFor(savedPlatform);
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

  return {
    editing, setEditing, showForm, setShowForm, showPaste, setShowPaste,
    pasteInitialText, setPasteInitialText, shareData, setShareData,
    groupFullscreen, setGroupFullscreen, showKey, setShowKey,
    testingPlatform, setTestingPlatform,
    fetching, setFetching, fetchError, setFetchError, saveError, setSaveError,
    name, setName, protocol, setProtocol, codingPlan, setCodingPlan,
    apiKey, setApiKey,
    batchPreviewKeys, setBatchPreviewKeys,
    handleApiKeyChange, confirmBatchCreate, cancelBatchPreview, previewNames,
    models, setModels, availableModels, setAvailableModels,
    endpoints, setEndpoints, activeDropdown, setActiveDropdown,
    showClaudeConfig, setShowClaudeConfig, claudeConfigJson, setClaudeConfigJson,
    globalClaudeConfig, setGlobalClaudeConfig, extra, setExtra,
    mockConfig, setMockConfig, newApiConfig, setNewApiConfig,
    manualBudgets, setManualBudgets,
    breakerFailureThreshold, setBreakerFailureThreshold,
    breakerOpenSecs, setBreakerOpenSecs,
    breakerHalfOpenMax, setBreakerHalfOpenMax,
    breakerDefaults,
    peakHours, setPeakHours, peakHoursTz, setPeakHoursTz,
    disableDuringPeak, setDisableDuringPeak,
    autoGroup, setAutoGroup, joinGroupIds, setJoinGroupIds,
    levelPriority, setLevelPriority, expiresAt, setExpiresAt, expiryEnabled, setExpiryEnabled,
    lockedGroupId, setLockedGroupId,
    isMock, isPassthrough, keyOptional, apiKeyMissing, uniqueGroupInfo,
    resetForm, openCreatePlatform, handleEdit, handleDuplicate, handleProtocolChange,
    handleModelChange, handleModelSelect, handleFetchModels, handleFillAll, buildModelsPayload,
    handleSave, handleViewLogs, applyPaste, runBatchCreateFromPaste,
  };
}
