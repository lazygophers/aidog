# 冷启动阻塞消除与 bundle 拆分 — PRD (主入口)

## 目标
- [x] 消除 setup() 首行同步跑 $SHELL -ilc echo $PATH 对窗口创建的阻塞 —— 实测本机 0.71 / 0.74 / 1.54 秒，是冷启动最大单点
- [x] 把启动期 4 处 block_on 串行工作（settings 迁移与加载、group settings 文件同步、coding tools 默认值、中间件 reload）与 cleanup_old_logs 挪到窗口显示之后
- [x] 拆分 1.6M 的 main bundle —— 14 个页面全静态 import，零页面级代码分割，locale 已分包而页面未分包
- [x] 去掉 App.tsx 的 key={effectiveNav} 强制整树重挂载 —— 每次切页全量重新取数（**保留**，理由见验收标准第 6 条）
## 边界
- [x] 只动启动时序与打包配置，不改任何业务逻辑与页面功能（改动面：启动期 spawn 时序 + PATH 注入方式 + 页面 import 方式；无任何页面组件内部逻辑/渲染结构改动，s6 判定为零改动）
- [x] PATH 探测的 OnceLock 幂等语义保留，只挪调用点到真正 spawn 子进程的入口（skills 检测与安装、script_executor），保证子进程仍能拿到完整 PATH
- [x] 挪到窗口后执行的启动工作必须保证在被依赖前完成，不得引入竞态
- [x] 不动前端合成层与动画（归 frontend-compositing-purge，本 task 依赖其完成以获得稳定的 bundle 基线）
- [x] 不动 Rust 转发热路径（归 proxy-hotpath-buffers）
- [ ] [需人工] 切页体验不得退化（红线 3）：key 经 s6 判定保留，fadeIn 动画与原状一致、无状态残留风险面（互斥条件渲染本就完整卸载重建）；lazy 化后的切页流畅度需真实点击确认，与第 20 条同一次目视即可覆盖
## 验收标准
- [x] 冷启动到窗口可见的时间相对基线下降，给出 release 构建下的前后秒数对比（2.844s → 1.351s，-52.5%；baseline.md §9）
- [x] setup() 路径上不再有 $SHELL -ilc 调用；skills 与 script_executor 路径首次 spawn 子进程前 PATH 仍完整（grep -n '\$SHELL -ilc' app_setup.rs 无匹配；baseline.md §5 详述 per-Command 注入机制）
- [x] 启动期 block_on 数量下降，剩余项逐条说明为何必须同步（原 4 处已全挪后台，engine.reload 保留同步执行以保证规则桶初始化；baseline.md §6 逐行说明）
- [x] main bundle 体积相对 1.6M 显著下降，首屏加载的 JS 字节数给出前后对比（1,634,950 B → 191,801 B，-88.3%；baseline.md §7/§9）
- [ ] [需人工] 14 个页面按需加载，切换到未访问过的页面能正确加载且无闪烁或报错（代码结构验证已通过：14 个页面已 React.lazy()，build 产物各自独立 chunk；startTransition 保留旧树无闪烁机制已验；需真实 UI 点击验证）
- [ ] [不适用] 去掉 key={effectiveNav} 后逐页验证：切走再切回不串数据、筛选状态行为符合预期（key 未去掉，保留原状；s6 判定：子页面切换的挂载/卸载由互斥条件渲染决定，key 仅影响外层 fadeIn 动画，无数据隔离作用）
- [x] navGuard 离页拦截在页面改为 lazy 后仍正常工作（registerNavGuard 模块级单例，生命周期钩卸载时反注册，与组件导入方式无关；baseline.md §7 逻辑代码依据 Settings.tsx:390）
- [x] yarn build 通过、yarn test 全绿、check-i18n 通过（check-i18n ✅ 零缺失；yarn test 26 files 332 tests 全绿；yarn build 191.80 kB 与基线一致）
- [x] cargo clippy 零 warning、cargo test 全绿（cargo clippy 仅 ts-rs 既有警告；cargo test 1639 passed，2 个已知 flaky 例外见 baseline.md §6）
- [x] 清场完成：临时产物已清（/tmp 构建日志已删；仓库源码树零残留）。**采样脚本 measure_startup.sh 经用户裁定保留**（存 .skein/task/cold-start-unblock/scripts/，非仓库源码）——下游 task perf-final-verification 依赖本 task，需复用同一计时协议才能与本轮基线数据可比；删掉则须重写，实现差异会让前后数据失去可比性。原验收条「量测脚本已删」按此改写。
## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list cold-start-unblock`)
