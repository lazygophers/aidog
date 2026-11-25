# 后端压测 / 采样：隔离、测对路径、别被本机环境骗

适用于起 aidog 内核做压测、延迟、CPU / 内存采样的活，含派给后台 agent 的。

## 1. 用户的 AiDog 不能碰

用户 AiDog 常驻 `127.0.0.1:9890`。

- 禁 `pkill` / `killall` / 按名字 kill。只停自己起的、记在 pid 文件里的进程，停之前核对命令行，内核再核对环境变量 `HOME`（防 pid 复用误杀）。
- 隔离实例：`HOME=/tmp/aidog-perf-<id>`（数据落 `$HOME/.aidog`，内核写的 `.claude*` / `.codex` / `.pi` 也跟着进 /tmp），端口避开 9890，脚本见到 9890 直接拒绝。
- 派 agent 时把上面两条原样贴进 prompt——agent 不会自己读这份文件（同模式：wayfinder-session.md §2）。
- 现成实现：`scripts/perf/`（`env.sh` 拒 9890、`stop.sh` 三重核对、`README.md` 一键流程），照用，别重写。

## 2. 必须测真实转发路径

旧压测打 mock 分组，在 `gateway/proxy/handler.rs` 里 `Protocol::Mock` → `handle_mock` 被提前截走，从没经过 `forward_attempt`（协议转换 + 透传 + 写日志），数字再漂亮也不是代理开销。

- 上游用本地假上游进程（harness 的 `mock_upstream.mjs`），平台是真协议（openai / anthropic）、`base_url` 指向假上游。
- 冒烟时核对假上游计数（`curl 127.0.0.1:<mock>/stats`）和隔离 `log.db` 最新行，确认请求真到了上游。

## 3. 本机代理环境变量会让 curl 绕道

本机 env 有 `HTTP_PROXY=127.0.0.1:7890`（Clash），`NO_PROXY` 用分号分隔，curl 不认，127.0.0.1 也走代理——不处理则全部测量实为「经 Clash」。

- 测量脚本开头 `unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy`，或 curl 一律带 `--noproxy '*'`。
- 首跑用 `lsof -nP -p <pid> -iTCP` 核实客户端 → 内核 → 假上游都是直连。

## 4. CPU 采样：samply 不可用，用 /usr/bin/sample

`samply` attach 报 `task_for_pid ... code 5`，要先跑 `samply setup` 改系统签名设置，用户没授权——不要跑，也不要当「装一下就好」。

- 回退：`/usr/bin/sample <pid> 30 -mayDie -file <out>`，配 harness 的 `sample_analyze.py` 去掉阻塞等待叶子。
- 要行号：另建带符号副本，`CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`，`--target-dir` 指 `<项目根>/.worktrees/.target/perf`（worktree 独立 target 子目录，与 release 配置隔离；压测二进制按 §6 在 detach worktree 构建，不污染 `src-tauri/target`）。

## 5. 收尾清 /tmp，保留 harness

- 收尾 `du -sh /tmp/aidog-perf-*` 列出来，删自己建的，账本 **遗留** 写删了什么。
- harness 不是一次性草稿：清理 `.scratch/perf-backend/` 之前，先把 `harness/` 移到受版本管理的 `scripts/perf/`，再删其余。

## 6. 压测二进制血统：detach worktree 构建 + 开测前验符号

共享 checkout 的 HEAD 随时被别的会话切走——在被切走的树上构建，测到的就是别人的代码，整轮结论作废。

- **被测二进制一律从独立源构建**：`/usr/bin/git worktree add --detach /tmp/<dir> <目标 commit>` 后在该目录里 build（manifest 指向 `/tmp/<dir>/src-tauri/Cargo.toml`，或 `KERNEL_BIN` 指过去），绝不信共享 checkout 的 `src-tauri/target`。
- **开测前验血统**（两条 `strings`，几秒的事）：`strings $KERNEL_BIN | grep -c <本轮引入的符号>` >0；`strings $KERNEL_BIN | grep -c <本轮删掉的符号>` =0。符号随分支内容定，测前想清楚「引入了什么、删了什么」各验一条。
- 该 worktree 的 `CARGO_TARGET_DIR` 指 `<项目根>/.worktrees/.target`（带符号构建用子目录 `.worktrees/.target/perf`），禁共享主仓 target（串味与 `cargo clean -p` 误伤见 shared-worktree.md「构建目录」）。
- profile 里出现「按理已删的函数」就是血统污染的直接信号，先验二进制再怀疑采样工具（ui-parity-audit.md「先核实再下结论」同模式）。

## 7. 性能异常归因：双版本同环境对照，未复现关闭带复活条件

§6 血统流程是前提，本条管归因和关闭。

- **归因要双版本对照**：异常是旧版本测出的，复现实验必须新旧两个二进制在**同一隔离环境、同一负载**下并排跑（各自 detach worktree 构建、各自隔离实例、同一假上游）。只测新版本复现不出，分不清「已修好」和「环境性当初就不复现」。
- **关闭票必带复活条件**（对齐 pr-merge-discipline.md 三要素）：写明「什么信号出现时重开」（例：真实负载 p99 周期性 >1s 且 conv <0.6s，重开时核对同机并发）。「未复现」不是「不存在」，没写复活条件的关闭下一轮会被当新票再开。
- 方向不一致（基线比新版还慢）本身是信息：异常源不在本次改动面，归因往环境（同机并发 / 系统调度）收，不在改动 diff 里翻。

## 8. 性能优化轮：静态审计只管定位，收益验收要实测；「短路/加缓存」查全部调用点

- **静态审计的收益是机制推断，不是验收证据**：改完热路径 / 缓存，至少对审计清单排名最高的改动跑前后对照（scripts/perf/ 隔离实例 + §7 双版本同负载，改动前 commit 与改动后各一 binary），数字进回报与账本。没有数字的收益一律按「做了没验证（机制推断）」档交付；实测不可行（需用户环境 / 长负载）时列人工验收，不静默记成「已优化」。
- **「按开关短路 / 加缓存」类改动的调用点要 grep 全部 caller**，implementer 自查与 reviewer 检查面同一条：同 shape 兄弟调用点不改，语义不对称——一半请求短路、一半照旧序列化（先例：响应头序列化 gate 只落一处，send_upstream / relay_passthrough 两个兄弟站点漏改）。

## 9. 测空闲 CPU / 内存：必须并列记录窗口内请求数

aidog 日常从不空闲（Claude Code 持续经代理收发），不记请求数的「空闲 CPU」测到的是负载。实测（102 个 30 秒窗口，macOS 8 核）：零请求窗口五进程 CPU 合计中位 0.03%（n=39），有请求窗口中位 2.40%（n=63）；同机同样静置曾测出 6.07% / 3.13% / 2.11% / 0.53%，差 11 倍，会被误读成「空闲有开销」。

- 采样脚本并列一列「窗口内新增 `proxy_log` 行数」，分析只取该列为 0 的窗口；真空闲 CPU 基线 0.03%。
- 在途判据不用整表 `status_code=0`（历史孤儿行永久堆积），限近 5 分钟内创建。
- 内存用 `/usr/bin/footprint -p <PID>` 的 `phys_footprint`，不用 RSS。
- 隔离与环境约束照 §1、§3（不碰用户 9890 实例、curl 直连）。

## 10. 去重 / 缓存类优化：先用真实数据验命中率，A/B 负载对齐用户配置

去重先例：据「request_body 与 upstream_request_body 几乎相同」实现逐字节去重，隔离 A/B 后发现模型映射改写 `model` 字段、用户真实流量 0/37 行逐字节相同，收益 0，撤回（0a140e44a）。对照：VACUUM 按比例触发、内存峰值等改动走「只读实测 → 静态审计 → 逐项改 → 双版本同负载复测」，结论可信（VACUUM 2 次 7.3s → 0、footprint 峰值 623MB → 160MB、wal 562MB → 4.7MB）。

- **「几乎相同」≠「逐字节相同」**：平均长度不同本身就是反证。去重、缓存、短路类优化的命中前提，开工前用只读 RPC（ui-color-bug-verify.md「显示类 bug」节手法）对用户实例抽样比对，算出命中率写进派发 prompt；命中率接近 0 不开工。
- **A/B 负载配置对齐用户实例的关键特征**（模型映射、日志开关、协议转换）：负载没配到优化触发条件，路径根本不触发，数字作废。派 A/B agent 时 prompt 列出「本优化的触发条件」，agent 冒烟先确认新路径被走到（日志 / 计数），再出数字。
- **开工先 `df -h .`**：性能轮要编 release / 带符号二进制，余量判据照 shared-worktree.md「构建目录」节（<10G 不开跑）。
