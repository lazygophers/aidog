import { useEffect, useState } from "react";
import type { Protocol } from "../../services/api";
import {
  getDefaultModels,
  getDefaultPeak,
  getProtocolColorMap,
  getProtocolHomepage,
  getProtocolLabel,
  getProtocolLabelMap,
  getProtocolSourceUrls,
  isCodingPlanProtocol,
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
 *  - labelMap: 全协议 label 映射（endpoint badge 覆盖所有 ep.protocol） */
export interface ProtocolMeta {
  color: string;
  isCpProtocol: boolean;
  defaultModels: string[];
  homepage: string;
  sourceUrls: { docs: string; pricing: string };
  protocolLabel: string;
  labelMap: Record<string, string>;
}

const INITIAL: ProtocolMeta = {
  color: "var(--accent)",
  isCpProtocol: false,
  defaultModels: [],
  homepage: "",
  sourceUrls: { docs: "", pricing: "" },
  protocolLabel: "",
  labelMap: {},
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
      });
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [protocol, extra, lang]);

  return meta;
}
