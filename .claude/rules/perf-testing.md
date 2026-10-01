# 后端压测 / 采样：隔离、测对路径、别被本机环境骗

2026-09-30 perf-backend 轮（`.scratch/perf-backend/`，wayfinder 6 票）沉淀。适用于一切
起 aidog 内核做压测、延迟、CPU / 内存采样的活，含派给后台 agent 的。

## 1. 用户的 AiDog 不能碰

用户 AiDog 常驻 `127.0.0.1:9890`。2026-08 有 perf agent 照旧脚本 `pkill -x aidog`
把它杀掉的事故（旧 `measure.sh`，`d2c3117a9^`）。

- 禁 `pkill` / `killall` / 按名字 kill。只停自己起的、记在 pid 文件里的进程，
  停之前核对命令行，内核再核对环境变量 `HOME`（防 pid 复用误杀）。
- 隔离实例：`HOME=/tmp/aidog-perf-<id>`（数据落 `$HOME/.aidog`，内核写的
  `.claude*` / `.codex` / `.pi` 也跟着进 /tmp），端口避开 9890，脚本见到 9890 直接拒绝。
- 派 agent 时把上面两条原样贴进 prompt——agent 不会自己读这份文件
  （同模式：wayfinder-session.md 第 2 条）。

现成实现：`.scratch/perf-backend/harness/`（`env.sh` 拒 9890、`stop.sh` 三重核对、
`README.md` 一键流程），照用，别重写。

## 2. 必须测真实转发路径

旧压测打的是 mock 分组，在 `gateway/proxy/handler.rs` 里 `Protocol::Mock` →
`handle_mock` 被提前截走，从没经过 `forward_attempt`（协议转换 + 透传 + 写日志）。
数字再漂亮也不是代理开销。

- 上游用本地假上游进程（harness 的 `mock_upstream.mjs`），平台是真协议
  （openai / anthropic）、`base_url` 指向假上游。
- 冒烟时核对假上游计数（`curl 127.0.0.1:<mock>/stats`）和隔离 `log.db` 最新行，
  确认请求真的到了上游。

## 3. 本机代理环境变量会让 curl 绕道

本机 env 有 `HTTP_PROXY=127.0.0.1:7890`（Clash），`NO_PROXY` 用分号分隔，curl 不认，
127.0.0.1 也走代理。首轮 TTFB 全部测成「经 Clash」，作废重跑（`results/baseline.md:14`）。

- 测量脚本开头 `unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy`，
  或 curl 一律带 `--noproxy '*'`。
- 首跑用 `lsof -nP -p <pid> -iTCP` 核实客户端 → 内核 → 假上游都是直连。

## 4. CPU 采样：samply 不可用，用 /usr/bin/sample

`samply` attach 报 `task_for_pid ... code 5`，要先跑 `samply setup` 改系统签名设置，
用户没授权——不要跑，也不要当成「装一下就好」。

- 回退：`/usr/bin/sample <pid> 30 -mayDie -file <out>`，配 harness 的 `sample_analyze.py`
  去掉阻塞等待叶子。
- 要行号：另建带符号副本，`CARGO_PROFILE_RELEASE_STRIP=false
  CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`，`--target-dir` 指到 /tmp 独立目录，
  不污染 `src-tauri/target`。

## 5. 收尾清 /tmp，保留 harness

- 该轮后台 agent 在 /tmp 留下约 10 GB（带符号 target 目录、DB 副本）。收尾
  `du -sh /tmp/aidog-perf-*` 列出来，删自己建的，账本 **遗留** 写删了什么。
- harness 不是一次性草稿。上一套压测脚本随 `d2c3117a9` 的 `.scratch` 清理一起删了，
  这轮只能从 `d2c3117a9^` 翻出来重建。清理 `.scratch/perf-backend/` 之前，
  先把 `harness/` 移到受版本管理的 `scripts/perf/`，再删其余部分。

## 6. 压测二进制血统：detach worktree 构建 + 开测前验符号（2026-10-01）

perf-backend 收尾轮（PR #47）：V6 带符号构建期间共享 checkout 被另一会话切到
master，采到的是 master 二进制——证据为二进制里零 mimalloc 符号（O7 双入口只在
分支上）+ 分支已删函数 `format_pretty_json` 的符号出现在采样结果里，整轮结论
作废重测一轮。

- **被测二进制一律从独立源构建**：`/usr/bin/git worktree add --detach /tmp/<dir>
  <目标 commit>` 后在该目录里 build（manifest 指向 `/tmp/<dir>/src-tauri/
  Cargo.toml`，或 `KERNEL_BIN` 指过去），绝不信共享 checkout 的
  `src-tauri/target`——checkout 的 HEAD 随时被别的会话切走，构建期间或之后
  一切，测到的都是别人的代码。
- **开测前验二进制血统**（两条 `strings`，几秒的事）：
  `strings $KERNEL_BIN | grep -c mimalloc` >0（本次改动引入的符号在）；
  `strings $KERNEL_BIN | grep -c format_pretty_json` =0（本次删掉的符号不在）。
  符号随分支内容定，测前想清楚「这轮引入了什么、删了什么」，各验一条。
- profile 里出现「按理已删的函数」就是血统污染的直接信号，先验二进制再怀疑
  采样工具（ui-parity-audit.md「先核实再下结论」同模式）。
