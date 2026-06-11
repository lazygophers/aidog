# PRD: 可配置 tray 面板

## 需求
系统设置页配置系统托盘展示：多平台同显 + 拖拽排序 + 每项样式(颜色/字号)/开关 + 今日消耗，删平台卡片 tray 开关。

## 决策（已确认）
| 项 | 结论 |
| --- | --- |
| 多平台 | 同时展示多个，拖拽排序（横排，按 order） |
| 位置 | 收敛为**排序**（macOS 绝对定位系统管控做不到） |
| 颜色 | **三态**：跟随系统(labelColor) / 预设语义色(systemRed/Green/Orange 自适应暗亮) / 自定义 hex(带可读性警告) |
| 今日消耗 | MVP **tokens**（proxy_log SUM，created_at≥今日本地0点）；cost 进阶另做 |
| 多平台宽度 | **不限**（用户自负责，菜单栏可能挤/截断） |
| 配置位置 | **AppSettings.tsx 新 tray tab**（Settings.tsx 是 Claude Code 配置器，非 app 设置）；删平台卡片 tray 开关 |
| 存储 | settings KV（scope=tray, key=config JSON），无需建表 |
| 迁移 | 升级若无 tray config，从旧 show_in_tray=1 平台生成默认 config（避免空白） |

## 技术边界（research 核实 objc2 源码）
可配：多平台横排✓/每段颜色✓(需 NSColor feature)/字号✓/排序✓/分隔符✓/≤2行✓；不可：绝对像素位置✗

## 涉及面（research 6 文档）
- 后端: settings tray config 模型 + read/write + command + 迁移(lib.rs)；tray 渲染重构(lib.rs tray_quota_text→多 item NSMutableAttributedString 颜色/字号/排序/layout)；今日 tokens 查询(db.rs)；Cargo.toml objc2-app-kit +NSColor feature
- 前端: AppSettings.tsx 新 tray tab(TrayConfigTab: 多选平台+拖拽排序+display+颜色三态+字号+开关+今日消耗项+layout/separator)；删 Platforms.tsx tray 开关；api.ts trayConfigApi

## 验收
- AppSettings tray tab 配置多平台+排序+颜色/字号+今日 tokens 开关
- 托盘按 config 渲染多 item（颜色/字号/排序/单两行）
- 删平台卡片开关 + 旧配置迁移默认
- cargo build+test+tsc

## 注意
- 基于 a391ad4d 已落地的 lib.rs tray（单平台→多 item 重构）
- 多窗口并行 db.rs/lib.rs → 主工作区，commit 仅本 task
