import type { Protocol, PlatformEndpoint, ModelSlot, ClientType } from "../../services/api";
import { getDefaultsJson } from "../../services/api";
import { PROTOCOLS } from "./constants";

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
};

/** defaults.json 运行时缓存：进程内只拉一次 Tauri command，5 函数共享。
 *  bundled/app-data 内容在会话内不变；同步链写入由后端覆盖下次进程启动。
 *  ponytail: 模块加载即发 invoke（Promise），5 函数 await 它 — 单次 RPC 共享，零状态机。 */
/** JSON 内 name/desc 用的 8 locale BCP 47 标识（zh 用 script 子标签 zh-Hans）。 */
export type DefaultsLocale = "en-US" | "zh-Hans" | "ar-SA" | "fr-FR" | "de-DE" | "ru-RU" | "ja-JP" | "es-ES";

type DefaultsDoc = {
  version?: string;
  last_updated?: number;
  protocols: Partial<Record<Protocol, {
    client_type?: ClientType;
    endpoints: { default?: PlatformEndpoint[]; coding_plan?: PlatformEndpoint[] };
    models: { default?: Partial<Record<ModelSlot, string>>; coding_plan?: Partial<Record<ModelSlot, string>> };
    model_list: { default?: string[]; coding_plan?: string[] };
    name?: Partial<Record<DefaultsLocale, string>>;
    desc?: Partial<Record<DefaultsLocale, string>>;
    /** 维护用 metadata：官方文档页 + 定价页 URL（非 UI 展示，仅手动核对更新时一站直达）。 */
    source_urls?: { docs: string; pricing: string };
    /** 官网首页 URL（非文档页；前端平台详情处展示外链）。 */
    homepage?: string;
    /** simpleicons.org slug（Rust logo_sync 拼 https://cdn.simpleicons.org/<slug>）；空串走 favicon/clearbit fallback。 */
    logo_url?: string;
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

/** i18next locale 与 JSON name/desc locale key 已统一为 BCP 47 script 子标签 (zh-Hans)。
 *  locale-rename (07-06) 前 i18next 用 zh-CN 区域子标签，需 LOCALE_TO_DEFAULTS 桥接；
 *  rename 后两端一致，直接用 i18next locale 作 DefaultsLocale 查 name/desc。 */

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

/** 派生协议本地化描述（fallback: locale → en-US → 空串）。 */
export async function getProtocolDesc(protocol: Protocol, locale?: string): Promise<string> {
  const doc = await loadDoc();
  const entry = doc.protocols[protocol];
  const desc = entry?.desc;
  if (!desc) return "";
  const loc = locale ? (locale as DefaultsLocale) : undefined;
  if (loc && desc[loc]) return desc[loc]!;
  return desc["en-US"] ?? "";
}

/** 取 protocol 官网首页 URL（平台详情处展示外链；未配置返空串）。 */
export async function getProtocolHomepage(protocol: Protocol): Promise<string> {
  const doc = await loadDoc();
  return doc.protocols[protocol]?.homepage ?? "";
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

/** 从 getDefaultEndpoints 派生 URL 子串（host + path），注入 PROTOCOLS 供智能识别 base_url 优先匹配。
 *  按 preset.codingPlan 取对应 cp 分支，避免 coding plan 与普通版互相误匹配。
 *  取 host+pathname（非仅 hostname）：同 host 分裂（如 glm open.bigmodel.cn 普通 /api/paas/v4 vs
 *  coding /api/coding/paas/v4）靠 path 子串区分；不同 host（xiaomi_mimo token-plan-cn vs api）靠 host 区分。
 *  matchPlatform 最长串胜出 → 最特异 preset 命中。单一事实源：base_url 改动只动 getDefaultEndpoints。
 *
 *  async 化后改显式初始化：调用方（应用启动期）await 一次，PROTOCOLS[].hosts 写入后稳定。
 *  旧版模块加载即跑 → 现版需调用方确保 host 注入早于第一个 matchPlatform 查询（应用 init 时序）。 */
export async function injectProtocolHosts(): Promise<void> {
  for (const p of PROTOCOLS) {
    const hosts = new Set<string>();
    const eps = await getDefaultEndpoints(p.value, !!p.codingPlan);
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
    if (hosts.size) p.hosts = [...hosts];
  }
}
