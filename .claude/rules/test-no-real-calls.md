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
