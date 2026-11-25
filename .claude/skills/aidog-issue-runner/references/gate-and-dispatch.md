# 假红归因、commit 纪律与派发 prompt 固定要素

## 假红归因（shared-worktree.md「构建目录」节，parallel-implement.md §7/§17/§20，§10 夹具条）

按顺序排查；假红期间你的唯一动作是取证和独占重跑，归因清楚了才动代码：

1. **并发重建**：extern 丢失 / doctest E0463 / 二进制消失这类「不该红的红」→ 前台独占重跑再归因（2026-11-24 起 issue worktree 用各自 target、无全局锁；同一 worktree 内同时只跑一个 cargo 即无并发重建）。
2. **build-script 产物陈旧**：首跑红且无 panic / 报产物缺失 → `touch src-tauri/crates/aidog_core/build.rs` 重跑（`cargo clean -p aidog_db` 后同样触发）。
3. **本机代理 env**：设了 HTTP_PROXY 时 aidog_kernel search / media 等约 5 测红 → 门禁命令前缀 `env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy`。
4. **计时断言偶红**：先单跑该测试一次；绿 = 负载抖动记遗留，仍红才按回归。已知挂钟断言先例：`mock_platform_ttft_and_inter_chunk_split`（预热已内置）。
5. **中途新红**：先 `git log --oneline -5 -- <红测试断言涉及的夹具文件>` 看别的会话是否当天改过夹具；红文件在他人脏文件集里 = 等其收工再复核，不替修、不 stash。
6. **门禁红是范围外 hunk**：先 `git diff` 找请求范围外的改动，回退再重跑；不先改测试断言迁就。

假绿两盲区（采信 agent 门禁结论前）：命令面（缩写命令 / 缓存 / 部分目标——clippy 必须 `--all-targets -D warnings`，cargo test 必须 `--workspace`，`make lint` 整条跑不分项）与 crate 面（子集门禁按数据流向 grep 补全）。

## commit 纪律（shared-worktree.md「硬规则」节 + commit -m 条）

- 禁 `git commit -a` / 无路径 commit；pathspec 精确到文件，禁目录级。整文件提交前确认该文件开工时干净；开工时已脏的文件走 patch 过滤：`git diff -- <文件> > .tmp/<任务>/sel.patch` → 脚本按 hunk 头只留自己的 hunk（patch 以换行结尾）→ `git apply --cached --check` → `git apply --cached` → `git diff --cached` 逐行核对 → **不带 pathspec 的 commit**。
- commit message 带 `$` 用单引号或 `git commit -F <file>`（`$0` 被展开不报错，静默进历史，2026-10-15 实锤）；提交后看输出首行核对 message 原样落盘。
- 开工与提交前各跑 `/usr/bin/git diff --cached --stat`；不明来源的暂存文件不 reset、不带进提交，回报写明清单与「来源不明」。
- `git status` / worktree 操作走 `/usr/bin/git` 直跑（rtk 包装会截断 / 静默失败）；rtk 改写触发的「no verdict」按报错原样重发一次，仍无裁决改用 Read。

## 派发 prompt 固定要素（parallel-implement.md §2/§3/§18/§32，wayfinder-session.md §2）

每份派发 prompt 必含：

1. **规则引用贴原文 + file:line**，不凭记忆概括；prompt 与票面冲突时 agent 按票面执行，安全闸门取更严交集并回报冲突点。
2. **基点自检**：「开工先 `git -C <worktree> rev-parse HEAD`，不等于派单基点 sha 就先 merge --ff-only 再动手，不停下提问」。
3. **路径自检句**：「本 prompt 引用的路径 / 文件名在仓库中不存在时，先自行核实真实路径再动手，不按原文照搬」；清单写死条数 N，条数与磁盘不符以磁盘为准。
4. **状态词三行**（走本地票时）：开工置 `Status: claimed`；结票文末追加 `## Answer` 并置 `Status: resolved`；状态词只有这两个，禁 done / closed。
5. **前台运行固定句**：「依赖性命令（编译 / 门禁 / 取锁）前台运行并设 timeout: 600000，命令与退出码写同一日志文件；harness 自动转后台时回报命令与日志路径、继续做不依赖结果的工作，不 idle 等通知」。固定句防不住自动转后台——「idle → 读 `.tmp/<任务>/` 日志 → 有 EXIT 就 SendMessage 贴结果唤醒」是预期运行形态，含长门禁的派发把一次唤醒计入流程预算。
6. **先红要求**：先只写测试跑出断言红（非编译红、非找不到元素红），回报贴 EXIT 值与失败测试名；做不到时「改坏实现→红→按行号还原」反证，标「非严格先红」。
7. **单层执行**：「你是执行 agent；直接完成任务，不派生子 agent；需要拆分时回报主会话」。
8. **边界兜底句**：「点名单之外发现必要改动时停手回报清单，待裁决；已做的超范围改动精确撤回」。

「新增 X」票派发前 grep 存在性（枚举 / 表 / 常量 / 路由 / UI 生产渲染点——仅测试引用的组件不算挂载面）；已存在改成「补差额」票。
