# PRD: 修复 TTS 后端初始化失败

## 现象
日志反复：`WARN notify: tts backend init failed error=Operation failed`
触发：notification_test / notification_test_tts / 任何 tts 通道开启的通知（task_complete/waiting_input）

## 根因
- 默认 `TtsBackend::CrossPlatform` (`models.rs:1757/1909`)
- `speak_cross_platform` (`notification.rs:289`) 在 `std::thread::spawn` 独立线程内 `tts::Tts::default()`
- tts crate macOS 后端 = AVFoundation，`AVSpeechSynthesizer` 在后台线程构造返回 `Err("Operation failed")`（tts crate 已知 macOS 后台线程限制）
- init 失败仅 warn，无兜底 → TTS 静默失效

## 修复方案
cross_platform backend init/speak 失败时 fallback 到 `say` 命令（macOS only）。保留 cross_platform 跨平台语义，失败兜底保证 macOS 至少有声。

## 产出
### `src-tauri/src/gateway/notification.rs`
1. 新增 `fallback_say(text)`：
   - macOS: `std::process::Command::new("say").arg(text).status()`，失败 warn
   - 非 macOS: no-op（无 say，保留 tts crate 唯一选项）
2. `speak_cross_platform`:
   - `Tts::default()` Err → warn + `fallback_say`
   - `t.speak()` Err → warn + `fallback_say`
   - 成功：保留 sleep 200ms

## 验证
- cargo build 通过
- cargo clippy 无 warning
- cargo test（notification 相关）通过
- macOS 运行：notification_test_tts 不再 "init failed" warn，say 兜底发声

## 范围（不做）
- 不改默认 backend（保留 cross_platform 跨平台语义）
- 不改 TtsBackend enum
- 不改前端

## 依赖
无前端改动
