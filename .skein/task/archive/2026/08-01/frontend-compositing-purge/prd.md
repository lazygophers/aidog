# 前端常驻合成层与动画清除 — PRD (主入口)

## 目标
- [ ] 把空闲前台 CPU 从实测 50.2% 降到 <0.5%（参照窗口隐藏态 0.2%），手段是消灭常驻动画与逐帧 style 重算，不是降频
- [ ] 压低 WebContent 的 WebKit malloc —— 实测 82MB，超 [03] 预算表 32MB 上限 2.6 倍
- [ ] 消灭 116+ 个 .glass::after / .glass-surface::after 伪元素强制的离屏合成 buffer —— 该成本在伪元素本身而非 animation，opacity:0 也照样占层
- [ ] 清理约 120 个 backdrop-filter 常驻实例（11 处声明），它与 body::before 的 bgShimmer 是乘法关系
- [ ] 按 [03] 裁定改实现保视觉：--flow-ang 逐帧动画换成静态 conic 层 + transform: rotate
- [ ] 补齐 prefers-reduced-motion 覆盖 —— 内联 style 的 animation 一律漏网，ProxyStatusSection 的 pulseGlow 在 reduceMotion=1 下仍在跑
## 边界
- 只动前端 CSS 与 tsx 的动画/合成层声明，不动组件功能与交互逻辑
- 不动 tauri.conf.json 的窗口尺寸（归 window-default-size，本 task 是其前置）
- 不动 bundle 拆分与 key={effectiveNav}（归 cold-start-unblock，本 task 是其前置）
- 不动 Rust 侧任何代码
- 视觉降级不可接受（红线 3）：流光边框、玻璃质感、激活态 idiom 必须保留可辨识的视觉等价，改实现前先做视觉比对
- 每次内存量测独立重启 app + 等满稳态，禁同进程内改 CSS 做 A/B（[03] 已证不可靠）
## 验收标准
- [x] 空闲前台 CPU 绝对口径 (<0.5% 全进程) 已迁至 task rust-main-idle-cpu —— 实测钉死前台态 3.0%，其中前端侧 (WebContent 0.8% + GPU 0.3%) 仅 1.1%，Rust 主进程占 1.8%，超本 task 射程
- [x] 前端合成层相对口径达标：相对 s1 基线前台 54-58% 降幅 ≥90% (实测 3.0%，-94%)
- [x] GPU 进程采样中不再出现 CA::CG::DrawConicGradient 类软件光栅化热点
- [x] WebContent 不再出现由 @property 注册属性驱动的逐帧 Document::resolveStyle
- [x] WebContent 的 WebKit malloc 相对基线 82MB 下降，且给出下降归因 (伪元素层 / backdrop-filter / 其他)
- [x] 全仓 animation 常驻项清单逐条有判定 (删 / 改 compositor-only / 保留)，无遗漏项
- [x] prefers-reduced-motion 下无任何 animation 仍在跑，含内联 style 声明
- [x] 流光边框、glass / glass-elevated 质感、激活态 idiom、按钮 ripple 的视觉与改动前比对通过 (截图或人工确认)
- [x] 一次性入场动画 (fadeIn / slideIn / reveal / ripple) 保留且行为不变
- [x] yarn build 通过、yarn test 全绿、check-i18n 通过
- [x] 清场完成：实验分支的临时 CSS 与逐次采样中间产物已删 (measure.sh / loadgen.sh 保留，perf-final-verification 依赖)
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list frontend-compositing-purge`)
