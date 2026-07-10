//! 系统通知分发服务（N1 — 系统通知模块）。
//!
//! 职责：
//! - 变量替换（{project}/{status}/{time}/{session}/{group}，未知占位保留）。
//! - 按 NotificationSettings.per_type[type].form 选择分发通道（Full/PopupOnly/InboxOnly/SoundOnly）。
//! - TTS 三后端：CrossPlatform（tts crate）/ MacSay（std::process `say`）/ WebSpeech（emit 事件给前端 webview）。
//! - 弹窗（tauri_plugin_notification）+ 收件箱落库（db）+ 未读更新事件 emit。
//!
//! 总开关 OFF 时整体旁路。音量跟随系统（不设置）。
//!
//! 子模块拆分（纯结构搬移，对外路径 `gateway::notification::X` 不变）：
//! - `vars`：占位变量替换（substitute_vars / fill_empty / utf8 推进）。
//! - `render`：通道解析（Channels/channels_for_form）+ 标题正文渲染（render/default_title）+ DispatchResult。
//! - `tts`：弹窗 / TTS 三后端 / 系统提示音（show_popup/speak/play_beep）。
//! - `dispatch`：核心分发入口（event 自含路径 + 类型兼容路径 + 唯一 action key 解析）。

mod dispatch;
mod render;
mod tts;
mod vars;

pub use dispatch::*;
pub use render::*;
pub use tts::*;
// 占位替换 API 对外可达（`gateway::notification::substitute_vars`），当前仅子模块 / 测试内部使用，
// 保留再导出以维持对外路径稳定。
#[allow(unused_imports)]
pub use vars::*;

#[cfg(test)]
mod test_dispatch;
#[cfg(test)]
mod test_render;
#[cfg(test)]
mod test_vars;
