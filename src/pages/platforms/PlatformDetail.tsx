// PlatformDetail — 平台详情独立全页（方向 5 密度终端：1px 网格密排、数字等宽右对齐）。
// T2 骨架：板块 A 页头 + H 配置（只读）+ I 快捷操作。
// T3 数据板块（B/B' 曲线、C 统计、D 健康明细、E 失败分析、F 水位、G 模型分布，
// 见 .scratch/平台-详情/实现说明.md）在 <div data-testid="detail-t3-mount"> 处挂载。
// 导航：无 react-router，由 Platforms.tsx 本地 state 切换；返回回平台列表，列表状态不丢。
import type { Platform } from "../../services/api";
import type { PlatformCardService } from "../../components/platforms/PlatformCard";
import { DetailHeader } from "../../components/platforms/detail/DetailHeader";
import { DetailConfig } from "../../components/platforms/detail/DetailConfig";
import { DetailActions } from "../../components/platforms/detail/DetailActions";
import { DetailData } from "../../components/platforms/detail/DetailData";

export interface PlatformDetailProps {
  platform: Platform;
  membership?: string[];
  onBack: () => void;
  onRefreshQuota?: (p: Platform) => void;
  service: PlatformCardService;
}

export function PlatformDetail({ platform, membership, onBack, onRefreshQuota, service }: PlatformDetailProps) {
  return (
    <div data-testid="platform-detail" style={{ display: "flex", flexDirection: "column" }}>
      <DetailHeader
        platform={platform}
        membership={membership}
        onBack={onBack}
        service={service}
      />
      {/* T3 数据板块挂载点：曲线 / 统计 / 健康明细 / 失败分析 / 模型分布（F 水位待读取命令） */}
      <div data-testid="detail-t3-mount">
        <DetailData platform={platform} />
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(340px, 1fr))", alignItems: "start" }}>
        <DetailConfig platform={platform} membership={membership} />
        <DetailActions platform={platform} onRefreshQuota={onRefreshQuota} service={service} />
      </div>
    </div>
  );
}
