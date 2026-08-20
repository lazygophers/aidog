# 冷启动阻塞消除与 bundle 拆分 — PRD (主入口)

## 目标
- [ ] 消除 setup() 首行同步跑 $SHELL -ilc echo $PATH 对窗口创建的阻塞 —— 实测本机 0.71 / 0.74 / 1.54 秒，是冷启动最大单点
- [ ] 把启动期 4 处 block_on 串行工作（settings 迁移与加载、group settings 文件同步、coding tools 默认值、中间件 reload）与 cleanup_old_logs 挪到窗口显示之后
- [ ] 拆分 1.6M 的 main bundle —— 14 个页面全静态 import，零页面级代码分割，locale 已分包而页面未分包
- [ ] 去掉 App.tsx 的 key={effectiveNav} 强制整树重挂载 —— 每次切页全量重新取数
## 边界
- 只动启动时序与打包配置，不改任何业务逻辑与页面功能
- PATH 探测的 OnceLock 幂等语义保留，只挪调用点到真正 spawn 子进程的入口（skills 检测与安装、script_executor），保证子进程仍能拿到完整 PATH
- 挪到窗口后执行的启动工作必须保证在被依赖前完成，不得引入竞态
- 不动前端合成层与动画（归 frontend-compositing-purge，本 task 依赖其完成以获得稳定的 bundle 基线）
- 不动 Rust 转发热路径（归 proxy-hotpath-buffers）
- 切页体验不得退化（红线 3）：去掉 key 后页面状态残留必须处理正确，不得出现串数据
## 验收标准
- [ ] 冷启动到窗口可见的时间相对基线下降，给出 release 构建下的前后秒数对比
- [ ] setup() 路径上不再有 $SHELL -ilc 调用；skills 与 script_executor 路径首次 spawn 子进程前 PATH 仍完整（有用例证明）
- [ ] 启动期 block_on 数量下降，剩余项逐条说明为何必须同步
- [ ] main bundle 体积相对 1.6M 显著下降，首屏加载的 JS 字节数给出前后对比
- [ ] 14 个页面按需加载，切换到未访问过的页面能正确加载且无闪烁或报错
- [ ] 去掉 key={effectiveNav} 后逐页验证：切走再切回不串数据、筛选状态行为符合预期
- [ ] navGuard 离页拦截在页面改为 lazy 后仍正常工作
- [ ] yarn build 通过、yarn test 全绿、check-i18n 通过
- [ ] cargo clippy 零 warning、cargo test 全绿
- [ ] 清场完成：量测脚本与中间产物已删
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list cold-start-unblock`)
