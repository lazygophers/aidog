// ─── 浮窗配置页 (facade) ──────────────────────────────────
// 托盘浮窗展示项的二维布局编辑器（显隐 / 拖拽 / 行列 / 尺寸 / 颜色 / 实时预览）。
//
// 拆分（arch 阶段6 S5）：state/actions 外迁 PopoverConfigTab/usePopoverConfig，
// JSX 区块在 PopoverConfigTab/PopoverLayout，行容器 / 单卡 / 卡片编辑体 / scope 配置
// 各自独立文件，常量与工厂在 PopoverConfigTab/{constants,utils}。
// 外部 import 路径（AppSettings.tsx `from "./PopoverConfigTab"`）零 churn。
import { usePopoverConfig } from "./PopoverConfigTab/usePopoverConfig";
import { PopoverLayout } from "./PopoverConfigTab/PopoverLayout";
import "../styles/popover.css";

export function PopoverConfigTab() {
  const d = usePopoverConfig();
  return <PopoverLayout {...d} />;
}
