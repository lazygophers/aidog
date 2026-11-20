# 单元测试禁真实外呼：TTS、HTTP 一律 mock 或本地假上游

单元测试过程中不得真实调用 TTS 或任何外部服务（用户 2026-10-28 原话拍板：
「所有的单元测试在测试过程中，不要真实再用 TTS。」）。

**为什么**：真实外呼花钱（计费 API）、慢（网络往返进测试耗时）、不确定（外网
行为变了测试就 flaky）。TTS 的 MacSay 后端会真出声（`aidog_notification/src/tts.rs:38`
`speak()` 调系统 `say` 命令），CrossPlatform 后端调 `tts` crate 真发声——测试碰
这两个路径就是对着用户喇叭播报。

**规则**：

1. 测 TTS 只断言数据与分发（现例：`test_dispatch.rs:59` `assert!(r.tts)` 字段
   断言；`test_kernel.rs:514` show_popup 走事件广播不真弹窗），禁真调
   `speak()` / `say` / `tts` crate / 前端 SpeechSynthesis。
2. 测 HTTP 逻辑一律打本地假上游，禁直连外部域名。现成手法：
   - Rust 配额脚本：`aidog_adapter/src/quota/test_balance.rs:22` `retarget()`
     把脚本内真域名替换成本地 stub 地址，`spawn_capture` 起本地假上游；
   - 通知 webhook：`test_dispatch.rs:207` `http://127.0.0.1:{port}/hook`；
   - 内核压测假上游：`scripts/perf/mock_upstream.mjs`（真协议、`base_url`
     指假上游，见 perf-testing.md §2）；
   - DB：`aidog_db::test_support::three_file_db()` 临时文件库，不碰真数据。
3. base_url 用 `https://x.com` 这类占位符时，必须确认代码在发请求**之前**
   就短路返回（如脚本内空 key 检查，`test_special_scripts.rs:181`），否则换成
   本地 stub。

## 测试 HOME 隔离检测脚本（2026-11-20）

背景：全部已知写入者用 `aidog_db/src/test_support.rs` 的 `HomeGuard` 隔离，但无全局拦截，
未来新测试漏用会静默写真实 HOME（真实用户的 `~/.aidog`）。

- **何时跑**：改动 `test_support.rs` / `HomeGuard`、新增触碰 FS 的测试、或怀疑有测试写真实
  HOME 时手动跑；不挂 `make lint` 常规链（全量一轮太重），可选定期执行。
- **怎么跑**：
  ```sh
  node scripts/check-test-home-isolation.mjs                              # 全量
  node scripts/check-test-home-isolation.mjs -- -p aidog_db test_rw_pool  # 子集
  ```
  脚本把 HOME / CODEX_HOME / CLAUDE_CONFIG_DIR 指向 tempdir（与 HomeGuard 同源三变量）、
  去掉六个代理变量后跑 cargo test；cargo 步骤取 `/tmp/aidog-cargo.lock`（node mkdir 轮询）。
- **报警怎么看**（双侧检测，退出码 1 = 泄漏，报两条口径）：
  1. **fakeHome 侧（主信号）**：HOME 指向 tempdir 后，走 env HOME 的漏隔离测试写的是
     fakeHome 根本碰不到真实 HOME——cargo 跑完后 fakeHome 的 `.aidog`/`.codex`/`.claude`/`.pi`
     点目录下出现文件 = 有测试写 HOME 未用 `HomeGuard`，实锤，按清单 grep 写入路径补守卫
     （只查点目录：cargo/mise toolchain 会往 fake HOME 写 `Library/Caches` 类缓存，全目录
     对比全是工具噪声，2026-11-20 实测 mise 误报）；
  2. **真实 `~/.aidog` 新增文件侧（兜底）**：只抓硬编码绝对路径的泄漏；排除常驻侧新建噪声
     （`logs/aidog.*.log` 整点轮转、`subagent-cap/<会话id>` 会话注册、`settings.<group>.json`
     用户新建分组时落盘——2026-11-20 三次实测误报，模式与依据均在脚本 `NOISE` 数组注释内），
     报出即人工判断。
  退出码 2 = 无泄漏但 cargo test 红（测试本身的问题，分开归因）；0 = 通过。
- **为什么只比新增文件不比 mtime**：常驻 AiDog 每秒都在改已有文件（db-wal/shm、日志追加），
  mtime 对比全是噪声。
