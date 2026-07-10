import type { Protocol, PlatformEndpoint, ModelSlot, ClientType } from "../../services/api";
import { getDefaultsJson, getClientTypesJson } from "../../services/api";
import type { ProtocolOption } from "./constants";

/** 高峰/低峰时段倍率窗口（多窗口数组，UTC+0 基准）。
 *  preset 给 per-protocol 默认；用户覆盖存 platform.extra.peak_hours。
 *  absent / 空数组 = 无调整（multiplier 1.0）。
 *  多窗口 first-match wins（数组顺序，命中第一个即用其 multiplier；都不命中 = 1.0）。
 *  跨天窗口：end_hour < start_hour（半开 [start,end)，22→6 = 22:00-06:00 次日）。 */
export type PeakWindow = {
  /** 0-23 UTC+0，含起始 */
  start_hour: number;
  /** 0-23 UTC+0，不含结束；<start_hour 表跨天 */
  end_hour: number;
  /** >0；>1 加价 / <1 折扣 / =1 无意义（勿存） */
  multiplier: number;
  /** 可选；0=Sunday…6=Saturday；absent = 每天适用 */
  days_of_week?: number[];
  /** 分钟精度起点 (0-59)；缺省 = 0（仅 hour 精度，向后兼容旧数据）。
   *  与 Rust `PeakWindow.start_minute` 对称（serde Option，#[serde(default)]）。 */
  start_minute?: number;
  /** 分钟精度终点 (0-59)；缺省 = 0（仅 hour 精度，向后兼容旧数据）。
   *  与 Rust `PeakWindow.end_minute` 对称。 */
  end_minute?: number;
  /** 月内日过滤 (1-31)；缺省 = 不过滤；与 `days_of_week` 在 UI 层互斥
   *  （hit 层同时 Some 取 AND 兜底，正常不触发）。与 Rust `PeakWindow.days_of_month` 对称。 */
  days_of_month?: number[];
  /** model scope（model 维度过滤，PRD 07-09 D2）；缺省 / undefined = 全平台模型生效（向后兼容）。
   *  元素支持 `"glm-5.2*"` 后缀通配（覆盖 `glm-5.2` / `glm-5.2-turbo`），exact-first。
   *  与 Rust `PeakWindow.models: Option<Vec<String>>` 对称（跨层一致，见 cross-layer-rules.md）。 */
  models?: string[];
  /** 生效期起点（Unix 秒，PRD 07-09 D2 福利期自动切换）；缺省 / undefined = 立即可用。
   *  `epoch_sec < start_at` → 窗口尚未启用，跳过此窗口（first-match 继续后续）。
   *  与 Rust `PeakWindow.start_at: Option<i64>` 对称。 */
  start_at?: number;
  /** 生效期终点（Unix 秒，PRD 07-09 D2）；缺省 / undefined = 永久。
   *  `epoch_sec >= end_at` → 窗口已失效，跳过此窗口。
   *  与 Rust `PeakWindow.end_at: Option<i64>` 对称。 */
  end_at?: number;
};

/** defaults.json 运行时缓存：进程内只拉一次 Tauri command，5 函数共享。
 *  bundled/app-data 内容在会话内不变；同步链写入由后端覆盖下次进程启动。
 *  ponytail: 模块加载即发 invoke（Promise），5 函数 await 它 — 单次 RPC 共享，零状态机。 */
/** JSON 内 name 用的 8 locale BCP 47 标识（zh 用 script 子标签 zh-Hans）。 */
export type DefaultsLocale = "en-US" | "zh-Hans" | "ar-SA" | "fr-FR" | "de-DE" | "ru-RU" | "ja-JP" | "es-ES";

type DefaultsDoc = {
  version?: string;
  last_updated?: number;
  protocols: Partial<Record<Protocol, {
    /** 协议层 coding plan 套餐标记（真值源）：true = 整套协议走 coding 套餐（独立子域 / 配额计费）。
     *  与 endpoint 级 `coding_plan` flag 语义不同（端点路由级）；两者并存。
     *  absent = false（向后兼容）。与 Rust `gateway::coding_plan::default_is_coding_plan` 对称。 */
    is_coding_plan?: boolean;
    client_type?: ClientType;
    endpoints: { default?: PlatformEndpoint[]; coding_plan?: PlatformEndpoint[] };
    models: { default?: Partial<Record<ModelSlot, string>>; coding_plan?: Partial<Record<ModelSlot, string>> };
    model_list: { default?: string[]; coding_plan?: string[] };
    name?: Partial<Record<DefaultsLocale, string>>;
    /** 维护用 metadata：官方文档页 + 定价页 URL（非 UI 展示，仅手动核对更新时一站直达）。 */
    source_urls?: { docs: string; pricing: string };
    /** 官网首页 URL（非文档页；前端平台详情处展示外链）。 */
    homepage?: string;
    /** simpleicons.org slug（Rust logo_sync 拼 https://cdn.simpleicons.org/<slug>）；空串走 favicon/clearbit fallback。 */
    logo_url?: string;
    /** 品牌色（hex，用于 PlatformCard / ProtocolLogo 等圆圈 fallback 背景）。
     *  派生层 getProtocolColorMap 从此字段派生；absent → 调用方回退 var(--accent)。 */
    color?: string;
    /** 协议搜索关键词（拼音/子串匹配用）；派生层 buildProtocolsFromPresets 透传到 ProtocolOption.keywords。 */
    keywords?: string[];
    /** coding plan 专属 token 前缀（如小米 token-plan 的 "tp-"）；机制 B 升级用。 */
    codingKeyPrefixes?: string[];
    /** 高峰/低峰时段倍率（多窗口，UTC+0 基准）。
     *  preset 给 per-protocol 默认；用户覆盖存 platform.extra.peak_hours。
     *  absent / 空数组 = 无调整（multiplier 1.0）。
     *  多窗口 first-match wins。跨天: end_hour < start_hour（半开 [start,end)）。 */
    peak_hours?: PeakWindow[];
  }>>;
};

let docPromise: Promise<DefaultsDoc> | null = null;

async function loadDoc(): Promise<DefaultsDoc> {
  if (!docPromise) {
    docPromise = getDefaultsJson()
      .then((raw) => {
        try {
          const parsed = JSON.parse(raw) as DefaultsDoc;
          if (parsed && parsed.protocols) return parsed;
        } catch (e) {
          // fall through to empty
          console.warn("[defaults] parse defaults.json failed:", e);
        }
        return { protocols: {} } as DefaultsDoc;
      })
      .catch((e) => {
        console.warn("[defaults] get_defaults_json failed:", e);
        return { protocols: {} } as DefaultsDoc;
      });
  }
  return docPromise;
}

/** 测试专用：清缓存让下一轮 loadDoc 重新走 mockIPC（生产代码禁调）。 */
export function __resetDefaultsCacheForTests(): void {
  docPromise = null;
}


/** 短路空响应：JSON 缺 protocol 时返默认值（保 4 函数向后兼容）。 */
function pickBranch<T>(section: { default?: T; coding_plan?: T } | undefined, codingPlan: boolean | undefined, fallback: T): T {
  if (!section) return fallback;
  const cp = !!codingPlan;
  const branch = cp ? section.coding_plan : section.default;
  // coding_plan 分支缺失时回落 default（保旧行为：cp 但无独立配置 = 与 default 同）
  return (branch ?? section.default ?? fallback);
}

/** 根据端点协议返回推荐的默认客户端类型（async：从 defaults.json 读 client_type 字段）。 */
export async function defaultClientForProtocol(protocol: Protocol): Promise<ClientType> {
  const doc = await loadDoc();
  return doc.protocols[protocol]?.client_type ?? "default";
}

/** 根据 ProtocolOption 生成默认端点（含 coding_plan 标记）
 *  数据来源：platform-presets.json（运行时可被 ~/.aidog/platform-presets.json 覆盖） */
export async function getDefaultEndpoints(protocol: Protocol, codingPlan?: boolean): Promise<PlatformEndpoint[]> {
  const doc = await loadDoc();
  const entry = doc.protocols[protocol];
  if (!entry) return [];
  const list = pickBranch<PlatformEndpoint[]>(entry.endpoints, codingPlan, []);
  // 浅拷贝（保旧行为：调用方 mutate 不污染源）
  return list.map((ep) => ({ ...ep }));
}

/** 主流平台预设默认模型（按 PlatformModels 槽位语义归类）。
 *  与 getDefaultEndpoints 同址同模式：从 defaults.json 读，落 CreatePlatform.models。 */
export async function getDefaultModels(protocol: Protocol, codingPlan?: boolean): Promise<Partial<Record<ModelSlot, string>>> {
  const doc = await loadDoc();
  const entry = doc.protocols[protocol];
  if (!entry) return {};
  return { ...pickBranch<Partial<Record<ModelSlot, string>>>(entry.models, codingPlan, {}) };
}

/** 平台内置候选模型列表（供模型槽位下拉冷启动兜底）。 */
export async function getDefaultModelList(protocol: Protocol, codingPlan?: boolean): Promise<string[]> {
  const doc = await loadDoc();
  const entry = doc.protocols[protocol];
  if (!entry) return [];
  const list = pickBranch<string[]>(entry.model_list, codingPlan, []);
  return [...list];
}

/** preset 该协议的 peak_hours 默认（用户覆盖存 platform.extra.peak_hours；absent/空 = 1.0 无调整）。
 *  deep copy 防 mutate 污染 docPromise 缓存。 */
export async function getDefaultPeakHours(protocol: Protocol): Promise<PeakWindow[]> {
  const doc = await loadDoc();
  const list = doc.protocols[protocol]?.peak_hours ?? [];
  return list.map(w => ({
    ...w,
    days_of_week: w.days_of_week ? [...w.days_of_week] : undefined,
    days_of_month: w.days_of_month ? [...w.days_of_month] : undefined,
    models: w.models ? [...w.models] : undefined,
  }));
}

/** i18next locale 与 JSON name locale key 已统一为 BCP 47 script 子标签 (zh-Hans)。
 *  locale-rename (07-06) 前 i18next 用 zh-CN 区域子标签，需 LOCALE_TO_DEFAULTS 桥接；
 *  rename 后两端一致，直接用 i18next locale 作 DefaultsLocale 查 name。 */

/** 派生协议本地化显示名（fallback: locale → en-US → protocol key）。
 *  调用方: SearchableProtocolSelect 渲染 + 拼音搜索 + Sub2ApiImport option。
 *  ponytail: 复用 docPromise 单次 RPC，纯函数式派生，零状态机。 */
export async function getProtocolLabel(protocol: Protocol, locale?: string): Promise<string> {
  const doc = await loadDoc();
  const entry = doc.protocols[protocol];
  const name = entry?.name;
  if (!name) return protocol;
  const loc = locale ? (locale as DefaultsLocale) : undefined;
  if (loc && name[loc]) return name[loc]!;
  return name["en-US"] ?? protocol;
}

/** 取 protocol 官网首页 URL（平台详情处展示外链；未配置返空串）。 */
export async function getProtocolHomepage(protocol: Protocol): Promise<string> {
  const doc = await loadDoc();
  return doc.protocols[protocol]?.homepage ?? "";
}

/** 协议是否标记为 coding plan 套餐（数据驱动真值源，非硬编码协议键名）。
 *  PlatformCard「Coding Plan」徽标据此判断；3 协议 glm_coding / bailian_coding /
 *  compshare_coding 的 JSON 条目标 `is_coding_plan: true`，其他 absent = false（向后兼容）。
 *  与 Rust `gateway::coding_plan::default_is_coding_plan` 对称（跨层一致）。 */
export async function isCodingPlanProtocol(protocol: Protocol): Promise<boolean> {
  const doc = await loadDoc();
  return doc.protocols[protocol]?.is_coding_plan ?? false;
}

/** 批量取协议 label（一次 RPC 拉全表后内存过滤，避免 N 次 await）。
 *  codingPlan 变体共用同 value 的 name，调用方自行追加 "Coding Plan" 后缀。 */
export async function getProtocolLabelMap(locale?: string): Promise<Record<Protocol, string>> {
  const doc = await loadDoc();
  const loc = locale ? (locale as DefaultsLocale) : undefined;
  const out = {} as Record<Protocol, string>;
  for (const proto of Object.keys(doc.protocols) as Protocol[]) {
    const name = doc.protocols[proto]?.name;
    if (!name) { out[proto] = proto; continue; }
    out[proto] = (loc && name[loc]) || name["en-US"] || proto;
  }
  return out;
}

/** 从 getDefaultEndpoints 派生 URL 子串（host + path），供智能识别 base_url 优先匹配。
 *  按 preset.codingPlan 取对应 cp 分支，避免 coding plan 与普通版互相误匹配。
 *  取 host+pathname（非仅 hostname）：同 host 分裂（如 glm open.bigmodel.cn 普通 /api/paas/v4 vs
 *  coding /api/coding/paas/v4）靠 path 子串区分；不同 host（xiaomi_mimo token-plan-cn vs api）靠 host 区分。
 *  matchPlatform 最长串胜出 → 最特异 preset 命中。单一事实源：base_url 改动只动 getDefaultEndpoints。
 *
 *  注：buildProtocolsFromPresets 内联本逻辑（hosts 直接写入 ProtocolOption.hosts），
 *  无独立 inject 步骤（旧 PROTOCOLS 模块级常量已删除）。 */
function deriveProtocolHosts(eps: PlatformEndpoint[]): string[] {
  const hosts = new Set<string>();
  for (const ep of eps) {
    try {
      const u = new URL(ep.base_url);
      const host = u.host.replace(/^www\./, "").toLowerCase();
      // host + path（去尾斜杠），path 为空则仅 host。含 path 让同 host 分裂可区分。
      const path = u.pathname.replace(/\/+$/, "").toLowerCase();
      const sub = path && path !== "/" ? host + path : host;
      if (host) hosts.add(sub);
    } catch { /* 非法 URL 跳过 */ }
  }
  return [...hosts];
}

/** 派生 ProtocolOption 列表（替代旧模块级 PROTOCOLS 硬编码常量）。
 *  loadDoc → 每 key 派生 {value, label, codingPlan, keywords, hosts, codingKeyPrefixes}。
 *  - label: name[locale] || name["en-US"] || key（三级回退链）；
 *  - codingPlan: is_coding_plan || false；
 *  - keywords: keywords || []；
 *  - hosts: 派生自 endpoints（host+path 子串，含 cp 分支），并入旧 injectProtocolHosts 逻辑；
 *  - codingKeyPrefixes: codingKeyPrefixes || []。
 *  调用方：SearchableProtocolSelect / Sub2ApiImport / PlatformEditForm / ccswitchMatch 等。
 *  ponytail: 复用 docPromise 单次 RPC，纯函数式派生，零状态机。 */
export async function buildProtocolsFromPresets(locale?: string): Promise<ProtocolOption[]> {
  const doc = await loadDoc();
  const loc = locale ? (locale as DefaultsLocale) : undefined;
  const out: ProtocolOption[] = [];
  for (const proto of Object.keys(doc.protocols) as Protocol[]) {
    const entry = doc.protocols[proto];
    if (!entry) continue;
    const name = entry.name;
    const label = (name && ((loc && name[loc]) || name["en-US"])) || proto;
    const codingPlan = !!entry.is_coding_plan;
    // hosts: 合并 default + coding_plan 分支端点（cp 协议自身也带 endpoints）
    const epsAll = [
      ...(entry.endpoints?.default ?? []),
      ...(entry.endpoints?.coding_plan ?? []),
    ];
    const hosts = deriveProtocolHosts(epsAll);
    out.push({
      value: proto,
      label,
      codingPlan,
      keywords: entry.keywords ?? [],
      ...(hosts.length ? { hosts } : {}),
      ...(entry.codingKeyPrefixes?.length ? { codingKeyPrefixes: entry.codingKeyPrefixes } : {}),
    });
  }
  return out;
}

/** 派生协议品牌色映射（替代旧模块级 PROTOCOL_COLORS 硬编码常量）。
 *  loadDoc → 每 key color 字段；absent 不写入（调用方回退 var(--accent)）。
 *  调用方：ProtocolLogo / PlatformCard / PlatformListView / PlatformEditForm。 */
export async function getProtocolColorMap(): Promise<Partial<Record<Protocol, string>>> {
  const doc = await loadDoc();
  const out: Partial<Record<Protocol, string>> = {};
  for (const proto of Object.keys(doc.protocols) as Protocol[]) {
    const color = doc.protocols[proto]?.color;
    if (color) out[proto] = color;
  }
  return out;
}

// ─── Client Types 派生层（替代旧模块级 CLIENT_TYPES 硬编码常量） ─────────────

/** client-types.json 单 entry（12 条：1 默认 + 5 Claude Code + 4 Codex + 2 IDE；value/group/name{8 locale}/desc{8 locale}）。
 *  真值源 `src-tauri/defaults/client-types.json`，运行时可被 `~/.aidog/client-types.json` 覆盖。
 *  前端禁直读 github / 文件系统，一律 invoke `get_client_types_json`。 */
export type ClientTypeEntry = {
  value: ClientType;
  /** 分组名（"Claude Code" / "Codex" / "IDE" / "" 默认）；UI optgroup 用 */
  group: string;
  name: Partial<Record<DefaultsLocale, string>>;
  desc?: Partial<Record<DefaultsLocale, string>>;
};

type ClientTypesDoc = {
  version?: string;
  last_updated?: number;
  client_types: ClientTypeEntry[];
};

/** client-types.json 运行时缓存：进程内只拉一次 Tauri command。
 *  同 docPromise 模式：模块加载即发 invoke（Promise），多函数 await 它 — 单次 RPC 共享。 */
let clientTypesDocPromise: Promise<ClientTypesDoc> | null = null;

async function loadClientTypesDoc(): Promise<ClientTypesDoc> {
  if (!clientTypesDocPromise) {
    clientTypesDocPromise = getClientTypesJson()
      .then((raw) => {
        try {
          const parsed = JSON.parse(raw) as ClientTypesDoc;
          if (parsed && Array.isArray(parsed.client_types)) return parsed;
        } catch (e) {
          console.warn("[client-types] parse client-types.json failed:", e);
        }
        return { client_types: [] } as ClientTypesDoc;
      })
      .catch((e) => {
        console.warn("[client-types] get_client_types_json failed:", e);
        return { client_types: [] } as ClientTypesDoc;
      });
  }
  return clientTypesDocPromise;
}

/** 测试专用：清缓存让下一轮 loadClientTypesDoc 重新走 mockIPC（生产代码禁调）。 */
export function __resetClientTypesCacheForTests(): void {
  clientTypesDocPromise = null;
}

/** 派生客户端类型列表（替代旧模块级 CLIENT_TYPES 硬编码常量）。
 *  loadClientTypesDoc → 每 entry 派生 {value, group, label}。
 *  - label: name[locale] || name["en-US"] || value（三级回退链）。
 *  调用方：formSectionsEndpoints 下拉 + 其他 CLIENT_TYPES 引用点。
 *  ponytail: 复用 clientTypesDocPromise 单次 RPC，纯函数式派生，零状态机。 */
export async function buildClientTypesFromPresets(locale?: string): Promise<Array<{ value: ClientType; group: string; label: string }>> {
  const doc = await loadClientTypesDoc();
  const loc = locale ? (locale as DefaultsLocale) : undefined;
  return doc.client_types.map((entry) => {
    const name = entry.name;
    const label = (name && ((loc && name[loc]) || name["en-US"])) || entry.value;
    return { value: entry.value, group: entry.group, label };
  });
}

/** 批量取 client_type value → label 映射（一次 RPC 拉全表后内存过滤，避免 N 次 await）。
 *  调用方需 label 时优先用本函数；未知 value 不写入（调用方回退原 value 展示）。 */
export async function getClientTypeLabelMap(locale?: string): Promise<Record<string, string>> {
  const doc = await loadClientTypesDoc();
  const loc = locale ? (locale as DefaultsLocale) : undefined;
  const out: Record<string, string> = {};
  for (const entry of doc.client_types) {
    const name = entry.name;
    out[entry.value] = (name && ((loc && name[loc]) || name["en-US"])) || entry.value;
  }
  return out;
}
