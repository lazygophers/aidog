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
  // 多 key 预览态 setter（applyPaste 多 key 分支灌表单后 setBatchPreviewKeys 触发实时预览，
  // 不再立刻批量创建；由 MultiKeyPreview 确认按钮调 confirmBatchCreate → runBatchCreateFromPaste）。
  setBatchPreviewKeys: React.Dispatch<React.SetStateAction<string[] | null>>;
  // form handler 引用（applyPaste 多 key 分支触发协议切换 + 批量循环 + 末尾 resetForm）
  handleProtocolChange: (newProtocol: Protocol, newCodingPlan?: boolean) => void | Promise<void>;
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
export async function applyPaste(r: SmartPasteApplyResult, ctx: PlatformPasteCtx): Promise<void> {
  const {
    setName, setProtocol, setApiKey, setCodingPlan, setModels, setAvailableModels,
    setEndpoints, setManualBudgets, setExtra, setMockConfig, setNewApiConfig,
    setBreakerFailureThreshold, setBreakerOpenSecs, setBreakerHalfOpenMax,
    setEditing, setLockedGroupId, setJoinGroupIds,
    setShowClaudeConfig, setClaudeConfigJson, setFetchError, setSaveError,
    setShowPaste, setShowForm,
    handleProtocolChange,
    endpoints,
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
    await handleProtocolChange(r.platform.value as Protocol, r.platform.codingPlan);
  }
  // 同步计算出本批 pasted 应落入的有效 endpoints（供 setEndpoints + 批量分支共用）。
  // ponytail: 把原 setEndpoints(prev=>...) 回调提取为纯函数 computeEndpoints(prev)，
  // 既写表单态又把同值喂给批量创建，避免批量分支读到 setState 未提交的旧 endpoints。
  const computeEndpoints = async (prev: PlatformEndpoint[]): Promise<PlatformEndpoint[]> => {
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
          else eps.push({ protocol: epProto, base_url: b.url, client_type: await defaultClientForProtocol(epProto) });
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
        eps.push({ protocol: epProto, base_url: b.url, client_type: await defaultClientForProtocol(epProto) });
      }
    }
    return eps;
  };
  // 单 key → 灌表单走旧路径；多 key → 灌表单 + setBatchPreviewKeys 触发实时预览（D3：不再立刻创建）。
  // apiKeys 可能为空（用户只粘贴了 base_url 无 key），保留 setApiKey("") 旧行为。
  const keys = r.apiKeys ?? [];
  if (keys.length > 1) {
    // 多 key 批量：同步计算有效 endpoints（避免读到 setState 未提交的旧表单态）。
    // 协议已由上方 handleProtocolChange(r.platform.value...) 落表单（命中平台时），
    // 未命中平台则沿用当前表单 protocol；此处不再重复设协议。
    const basePrev = r.platform
      ? await getDefaultEndpoints(r.platform.value as Protocol, r.platform.codingPlan)
      : endpoints;
    const effectiveEndpoints = await computeEndpoints(basePrev);
    // 灌表单态（让用户可见将批量化创建的配置），触发预览（与手动表单多 key 同路径，统一预览 UX）。
    // apiKey 灌多 key 拼接文本（用户可见 + 预览组件读 splitApiKeys 重新拆分）。
    ctx.setApiKey(keys.join("\n"));
    ctx.setEndpoints(effectiveEndpoints);
    if (r.expiresAt && r.expiresAt > 0) {
      ctx.setExpiresAt(r.expiresAt);
      ctx.setExpiryEnabled(true);
    }
    ctx.setBatchPreviewKeys(keys);
    ctx.setShowPaste(false);
    ctx.setShowForm(true);
    return;
  }
  // 单 key / 无 key 路径：清预览态（applyPaste 复用同一 ctx，避免上次多 key 预览残留）。
  ctx.setBatchPreviewKeys(null);
  if (r.baseUrls.length > 0) {
    // ponytail: 原 setEndpoints(computeEndpoints) 用 setState callback form 拿 prev，
    // computeEndpoints async 化后无法再走 setState callback（React 不支持 async updater），
    // 改显式取 ctx.endpoints（ctx 引用调用时刻闭包值，与原 prev 同源）。
    ctx.setEndpoints(await computeEndpoints(ctx.endpoints));
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
 * 预览批量创建的 name 列表（只读确认用）。
 * name 规则与 {@link runBatchCreateFromPaste} 完全一致：`{baseName}-{key 尾4位}`，撞名追号 `-2 -3 …`。
 * ponytail: 单源抽 util，保证预览名 = 实际创建名（撞名态在创建过程中可能继续变化，预览为当前快照）。
 *
 * @param keys 已拆分去重的 key 数组
 * @param baseName 平台名前缀（智能识别路径传 preset label；手动表单传当前 name）
 * @param usedNames 当前已存在的平台名集合（撞名判定基准；预览用当前快照，实际创建时动态更新）
 * @returns 与 keys 等长的 name 预览数组
 */
export function previewBatchNames(
  keys: string[],
  baseName: string,
  usedNames: Set<string>,
): string[] {
  const prefix = (baseName || "Platform").trim();
  // ponytail: 复制一份避免污染调用方传入的 Set（预览不写回 usedNames，实际创建才写回）。
  const used = new Set(usedNames);
  const out: string[] = [];
  for (const k of keys) {
    const tail = k.length >= 4 ? k.slice(-4) : k;
    let pname = `${prefix}-${tail}`;
    if (used.has(pname)) {
      let seq = 2;
      while (used.has(`${pname}-${seq}`)) seq++;
      pname = `${pname}-${seq}`;
    }
    used.add(pname); // 预览内同尾4位连发也追号（与实际创建语义一致）
    out.push(pname);
  }
  return out;
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
  // ponytail: 复用 previewBatchNames 单源逻辑 → 实际创建循环里逐个动态追号（创建中 usedNames 增长）。
  const joinIds = lockedGroupId != null ? [lockedGroupId] : joinGroupIds;
  const auto = lockedGroupId != null ? false : autoGroup;
  let okCount = 0;
  const failures: { key: string; err: string }[] = [];
  // 批量进行中即时反馈（避免 N 次串行 invoke 时用户以为卡死）。
  setToast({ text: t("platform.batch.progress", "批量创建中… {{done}}/{{total}}", { done: 0, total: keys.length }), ok: true });
  for (let i = 0; i < keys.length; i++) {
    const k = keys[i];
    // name 计算复用 previewBatchNames 的单元素版本（保持预览 = 实际创建名一致）。
    const pname = previewBatchNames([k], prefix, usedNames)[0];
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
      failures.push({ key: k.slice(-4), err: e?.toString() || "Unknown error" });
      console.error("batch create failed", { keyTail: k.slice(-4) }, e);
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

