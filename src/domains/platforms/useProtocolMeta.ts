import { useEffect, useState } from "react";
import type { ClientType, Protocol } from "../../services/api";
import {
  buildClientTypesFromPresets,
  getDefaultModels,
  getDefaultPeak,
  getProtocolColorMap,
  getProtocolHomepage,
  getProtocolLabel,
  getProtocolLabelMap,
  getProtocolSourceUrls,
  isCodingPlanProtocol,
  type TimeWindow,
} from "./defaults";
import { allModelValues } from "./health";
import { isCurrentlyPeak } from "../../utils/timeWindow";
import { parsePlatformPeak } from "../../services/api";

/** useProtocolMeta — PlatformCard 协议元数据聚合 hook（替代 5 个独立 async effect）。
 *
 *  5 个原本各自 await defaults.json RPC 的 effect（colorMap / isCp / models+peak /
 *  homepage / label+labelMap）合并为单 effect + Promise.all 一次性聚合。docPromise
 *  模块级单例缓存（defaults.ts::loadDoc）→ 100 卡 = 100 次 then，但每卡只一次 setState
 *  批处理，不再 600+ then 链。
 *
 *  ponytail: 纯函数式派生（无状态机），依赖 defaults.ts 单次 RPC 模式不变。
 *
 *  入参（依赖项）：
 *  - protocol: 平台类型（驱动 colorMap / isCp / models / homepage / label 查询）
 *  - extra: platform.extra JSON 串（含用户覆盖 peak；驱动 isPeak 计算）
 *  - lang: i18n 当前 locale（驱动 label / labelMap 本地化）
 *
 *  返回（首次渲染返初始值，异步聚合完成后 setState 触发一次重渲）：
 *  - color: 协议品牌色（fallback var(--accent)）
 *  - isCpProtocol: 协议层 coding plan 套餐标记（preset.is_coding_plan 真值源）
 *  - defaultModels: 高峰判定后的默认模型列表（allModelValues 后）
 *  - homepage: 协议官网 URL（未配置返空串）
 *  - sourceUrls: registry `source_urls` 的文档 / 定价页外链（未配置各返空串）
 *  - protocolLabel: 当前协议本地化名（fallback PROTOCOL_LABELS → key）
 *  - labelMap: 全协议 label 映射（endpoint badge 覆盖所有 ep.protocol）
 *  - peakWindows: 生效的高峰时段（`extra.peak` ?? preset 默认），供徽标判定复用 */
export interface ProtocolMeta {
  color: string;
  isCpProtocol: boolean;
  defaultModels: string[];
  homepage: string;
  sourceUrls: { docs: string; pricing: string };
  protocolLabel: string;
  labelMap: Record<string, string>;
  /** 生效的高峰时段 = 用户 `extra.peak` 优先 → preset 默认（CLAUDE.md 的混合源口径）。
   *  此前 PlatformCard 只读用户侧，导致 glm_coding / deepseek 这类自带预设 peak 的
   *  平台不显高峰徽标。 */
  peakWindows: TimeWindow[];
}

const INITIAL: ProtocolMeta = {
  color: "var(--accent)",
  isCpProtocol: false,
  defaultModels: [],
  homepage: "",
  sourceUrls: { docs: "", pricing: "" },
  protocolLabel: "",
  labelMap: {},
  peakWindows: [],
};

export function useProtocolMeta(
  protocol: Protocol,
  extra: string,
  lang: string,
): ProtocolMeta {
  const [meta, setMeta] = useState<ProtocolMeta>(INITIAL);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      // 单次 Promise.all 聚合 5 派生（docPromise 单例缓存，无 N 次 RPC）
      const [colorMap, isCp, peakWindows, homepage, sourceUrls, protocolLabel, labelMap] = await Promise.all([
        getProtocolColorMap(),
        isCodingPlanProtocol(protocol),
        getDefaultPeak(protocol),
        getProtocolHomepage(protocol),
        getProtocolSourceUrls(protocol),
        getProtocolLabel(protocol, lang),
        getProtocolLabelMap(lang),
      ]);
      if (cancelled) return;
      // 高峰判定：用户 extra.peak 优先 → preset default；命中则取 models.peak 分支
      const userPh = parsePlatformPeak(extra);
      const phWindows = userPh.length > 0 ? userPh : peakWindows;
      const isPeak = isCurrentlyPeak(phWindows, Date.now());
      const modelsBranch = await getDefaultModels(protocol, isPeak);
      if (cancelled) return;
      setMeta({
        color: colorMap[protocol] ?? "var(--accent)",
        isCpProtocol: isCp,
        defaultModels: allModelValues(modelsBranch),
        homepage,
        sourceUrls,
        protocolLabel,
        labelMap,
        peakWindows: phWindows,
      });
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [protocol, extra, lang]);

  return meta;
}

/** useProtocolLabelMap — labelMap-only 维度共享 hook（票 11）。
 *  消费点：GroupCreateModal / GroupEditPanel / ModelInfoTab / usePlatformForm /
 *  Sub2ApiImport / SearchableProtocolSelect / PlatformEditForm / PlatformListView。
 *  统一策略：首渲染 {}（调用方 fallback key / p.label）；cancelled flag 清理；
 *  lang 变化重拉（docPromise 单次 RPC，无新增缓存层）。lang 省略 = 非本地化
 *  （getProtocolLabelMap 内部默认 locale），沿用原 usePlatformForm 的 [] 依赖行为。 */
export function useProtocolLabelMap(lang?: string): Record<string, string> {
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  useEffect(() => {
    let cancelled = false;
    getProtocolLabelMap(lang).then(m => { if (!cancelled) setLabelMap(m); });
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lang]);
  return labelMap;
}

/** useProtocolColorMap — colorMap-only 维度共享 hook（票 11）。
 *  消费点：ProtocolLogo / PlatformListView / PlatformEditForm。
 *  首渲染 {}（调用方 fallback var(--accent)）；无 locale 维度（color 与语言无关）。 */
export function useProtocolColorMap(): Partial<Record<string, string>> {
  const [colorMap, setColorMap] = useState<Partial<Record<string, string>>>({});
  useEffect(() => {
    let cancelled = false;
    getProtocolColorMap().then(m => { if (!cancelled) setColorMap(m); });
    return () => { cancelled = true; };
  }, []);
  return colorMap;
}

/** useProtocolLabel — 单协议 label 维度共享 hook（票 11）。
 *  消费点：SmartPasteModal / ModelTestPanel / Logs/DetailPanel（source+target 各一）。
 *  首渲染 ""；protocol 为空（详情未选 / share 关闭）→ 置 ""。 */
export function useProtocolLabel(protocol: Protocol | undefined, lang: string): string {
  const [label, setLabel] = useState("");
  useEffect(() => {
    let cancelled = false;
    if (!protocol) { setLabel(""); return; }
    getProtocolLabel(protocol, lang).then(l => { if (!cancelled) setLabel(l); });
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [protocol, lang]);
  return label;
}

/** useClientTypeOptions — clientTypeOptions 维度共享 hook（票 11）。
 *  消费点：formSectionsEndpoints。首渲染 []（下拉空不崩）；best-effort catch
 *  保空列表；lang 变化重拉。 */
export function useClientTypeOptions(lang: string): Array<{ value: ClientType; group: string; label: string }> {
  const [options, setOptions] = useState<Array<{ value: ClientType; group: string; label: string }>>([]);
  useEffect(() => {
    let cancelled = false;
    buildClientTypesFromPresets(lang).then(list => { if (!cancelled) setOptions(list); })
      .catch(() => { /* best-effort，空列表保下拉不崩 */ });
    return () => { cancelled = true; };
  }, [lang]);
  return options;
}
