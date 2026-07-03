// platformPasteApply — 智能识别结果灌表单 + 批量创建（从 usePlatformForm 抽出控制行数）。
// ponytail: applyPaste + runBatchCreateFromPaste 各 ~80-160 行，独立成文件使 usePlatformForm ≤800。
//   纯函数（非 hook），经 ctx 收 form setters + list 依赖（platforms/quota/handleGroupsChanged 等），
//   保持原闭包语义（ctx 字段引用 usePlatformForm 调用时刻的闭包值）。
import type { TFunction } from "i18next";
import {
  platformApi,
  parseMockConfig, parseNewApiConfig, parsePlatformBreaker,
  type Platform, type Protocol, type PlatformEndpoint,
  type ManualBudget, type MockConfig, type NewApiConfig,
} from "../../services/api";
import { type SmartPasteApplyResult } from "../../components/platforms/SmartPasteModal";
import {
  getDefaultEndpoints, defaultClientForProtocol,
} from "../../domains/platforms";
import { getPrimaryBaseUrl } from "./usePlatformQuota";

/** ctx = applyPaste / runBatchCreateFromPaste 需要的全部 form state + list 依赖。
 *  字段引用 usePlatformForm 调用时刻闭包值（与抽前一致）。 */
export interface PlatformPasteCtx {
  t: TFunction;
  // form state（读 + 写）
  name: string;
  protocol: Protocol;
  endpoints: PlatformEndpoint[];
  lockedGroupId: number | null;
  joinGroupIds: number[];
  autoGroup: boolean;
  expiresAt: number;
  // form setters
  setName: React.Dispatch<React.SetStateAction<string>>;
  setProtocol: React.Dispatch<React.SetStateAction<Protocol>>;
  setApiKey: React.Dispatch<React.SetStateAction<string>>;
  setCodingPlan: React.Dispatch<React.SetStateAction<boolean>>;
  setModels: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  setAvailableModels: React.Dispatch<React.SetStateAction<string[]>>;
  setEndpoints: React.Dispatch<React.SetStateAction<PlatformEndpoint[]>>;
  setManualBudgets: React.Dispatch<React.SetStateAction<ManualBudget[]>>;
  setExtra: React.Dispatch<React.SetStateAction<string>>;
  setMockConfig: React.Dispatch<React.SetStateAction<MockConfig>>;
  setNewApiConfig: React.Dispatch<React.SetStateAction<NewApiConfig>>;
  setBreakerFailureThreshold: React.Dispatch<React.SetStateAction<string>>;
  setBreakerOpenSecs: React.Dispatch<React.SetStateAction<string>>;
  setBreakerHalfOpenMax: React.Dispatch<React.SetStateAction<string>>;
  setEditing: React.Dispatch<React.SetStateAction<Platform | null>>;
  setLockedGroupId: React.Dispatch<React.SetStateAction<number | null>>;
  setJoinGroupIds: React.Dispatch<React.SetStateAction<number[]>>;
  setShowClaudeConfig: React.Dispatch<React.SetStateAction<boolean>>;
  setClaudeConfigJson: React.Dispatch<React.SetStateAction<string>>;
  setFetchError: React.Dispatch<React.SetStateAction<string>>;
  setSaveError: React.Dispatch<React.SetStateAction<string>>;
  setShowPaste: React.Dispatch<React.SetStateAction<boolean>>;
  setShowForm: React.Dispatch<React.SetStateAction<boolean>>;
  setExpiresAt: React.Dispatch<React.SetStateAction<number>>;
  setExpiryEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  // form handler 引用（applyPaste 多 key 分支触发协议切换 + 批量循环 + 末尾 resetForm）
  handleProtocolChange: (newProtocol: Protocol, newCodingPlan?: boolean) => void;
  resetForm: () => void;
  // list 侧依赖（批量创建乐观 append + quota 补查 + 分组刷新 + toast）
  platforms: Platform[];
  setPlatforms: React.Dispatch<React.SetStateAction<Platform[]>>;
  platformsEpochRef: React.MutableRefObject<number>;
  quota: { scheduleQuotaFor: (p: Platform) => void };
  handleGroupsChanged: () => Promise<void>;
  groupsReloadRef: React.MutableRefObject<(() => void) | null>;
  setToast: React.Dispatch<React.SetStateAction<{ text: string; ok: boolean } | null>>;
}

/** 智能识别弹窗确认后，将解析结果填入添加表单。 */
export function applyPaste(r: SmartPasteApplyResult, ctx: PlatformPasteCtx): void {
  const {
    setName, setProtocol, setApiKey, setCodingPlan, setModels, setAvailableModels,
    setEndpoints, setManualBudgets, setExtra, setMockConfig, setNewApiConfig,
    setBreakerFailureThreshold, setBreakerOpenSecs, setBreakerHalfOpenMax,
    setEditing, setLockedGroupId, setJoinGroupIds,
    setShowClaudeConfig, setClaudeConfigJson, setFetchError, setSaveError,
    setShowPaste, setShowForm,
    handleProtocolChange,
    endpoints, protocol,
  } = ctx;
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
    const ex = s.extra ?? "";
    setExtra(ex);
    setMockConfig(parseMockConfig(ex));
    setNewApiConfig(parseNewApiConfig(ex));
    {
      const brk = parsePlatformBreaker(ex);
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
    ctx.setApiKey(keys[0]);
    ctx.setEndpoints(effectiveEndpoints);
    if (r.expiresAt && r.expiresAt > 0) {
      ctx.setExpiresAt(r.expiresAt);
      ctx.setExpiryEnabled(true);
    }
    ctx.setShowPaste(false);
    ctx.setShowForm(true);
    void runBatchCreateFromPaste(keys, ctx, r.platform?.label, effectiveEndpoints, effectiveProtocol);
    return;
  }
  if (r.baseUrls.length > 0) {
    ctx.setEndpoints(computeEndpoints);
  }
  if (keys.length === 1) ctx.setApiKey(keys[0]);
  // 智能粘贴识别到的过期时间（社区分享帖常见「即将过期 06-28 23:59」）。0/未识别 = 不动。
  // 识别到则自动启用 expiry toggle，使过期字段在表单可见（与 coding_plan 自动识别对齐），
  // 否则 toggle 默认 OFF → datetime-local 隐藏 → 用户误判「没识别到过期时间」。
  if (r.expiresAt && r.expiresAt > 0) {
    ctx.setExpiresAt(r.expiresAt);
    ctx.setExpiryEnabled(true);
  }
  ctx.setShowPaste(false);
  // 弹窗可能从主列表「添加平台」直达（表单尚未挂载），apply 后显式拉起表单展示已填字段。
  ctx.setShowForm(true);
}

/**
 * 批量创建 N 平台（智能识别多 key 或手动表单多 key 共用）。
 * 共用当前表单的 protocol / endpoints / 分组（autoGroup / joinGroupIds / lockedGroupId），
 * 每平台挂自己的 api_key，name = `{baseName}-{key 尾4位}`，撞名（同尾4位）自动追号 `-2`。
 * enabled=true、不调 model_test。失败项不中断整批，末尾 toast 汇总「成功 X / 失败 Y + 失败 key」。
 */
export async function runBatchCreateFromPaste(
  keys: string[],
  ctx: PlatformPasteCtx,
  baseName?: string,
  effectiveEndpoints?: PlatformEndpoint[],
  effectiveProtocol?: Protocol,
): Promise<void> {
  const {
    t, name, protocol, endpoints, lockedGroupId, joinGroupIds, autoGroup, expiresAt,
    setPlatforms, platformsEpochRef, quota, handleGroupsChanged, groupsReloadRef,
    resetForm, setToast,
  } = ctx;
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
  const usedNames = new Set(ctx.platforms.map(p => p.name));
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
      quota.scheduleQuotaFor(saved);
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
}
