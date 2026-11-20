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
    <div data-testid="platform-detail" className="mx-auto grid max-w-[1560px] grid-cols-1 items-start gap-0 sm:grid-cols-2 xl:grid-cols-[minmax(280px,340px)_minmax(0,1fr)_minmax(300px,360px)]">
      <div className="col-span-full min-w-0">
        <DetailHeader
          platform={platform}
          membership={membership}
          onBack={onBack}
          service={service}
        />
      </div>
      {/* 配置/快捷操作并入右栏自身堆叠（col3Extra）：grid 共享行高会把短栏下方的板块拉开留白 */}
      <DetailData
        platform={platform}
        col3Extra={
          <>
            <DetailConfig platform={platform} membership={membership} />
            <DetailActions platform={platform} onRefreshQuota={onRefreshQuota} service={service} />
          </>
        }
      />
    </div>
  );
}
