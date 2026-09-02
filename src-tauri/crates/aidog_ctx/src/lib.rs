//! `AppCtx`：命令函数与「外壳」之间的唯一接缝（票 06）。
//!
//! # 为什么存在
//!
//! 迁移前 206 个 command 直接吃 `tauri::AppHandle`（33 处）与 `tauri::State<T>`（165 处），
//! 把 Tauri 的状态表当依赖注入容器用。这让命令体永远只能跑在 Tauri 进程里——无界面内核
//! （票 08）没有 `AppHandle`，也就拿不到 `Db`。
//!
//! 现在命令体一律经 [`ctx()`] / [`db()`] 取依赖，`AppCtx` 由外壳实现：
//! - 桌面壳 `aidog_core::tauri_ctx::TauriCtx`（包着 `AppHandle`）
//! - 无界面内核（票 08）另写一个朴素结构体
//!
//! # 为什么是进程级单例而不是参数
//!
//! `AppCtx` 的实现天然是进程唯一的（一个 `AppHandle` / 一个内核实例），且票 06 的验收要求
//! 「命令函数签名里不再出现 `AppHandle` 与 `tauri::State`」。若改成显式 `ctx: Ctx` 参数，
//! 需要在 `tauri_command!` 宏里对该参数做特判（Tauri 侧要 `CommandArg`、票 08 的 axum 侧
//! 要跳过 JSON 反序列化），孤儿规则还逼着 newtype 定义在带 tauri 的 crate 里。
//! 走 `OnceLock` 单例后签名里只剩纯业务参数，宏与票 08 的 handler 生成都是机械展开。
//!
//! # 禁 tauri 依赖
//!
//! 本 crate 的 `Cargo.toml` **不得**出现 tauri（含可选 feature）。见该文件顶部注释。

use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use aidog_db::Db;
use aidog_middleware::MiddlewareEngine;
use tokio::task::JoinHandle;

/// 代理服务器运行句柄（`Some` = 在跑）。
///
/// 原 `aidog_core::shared::ProxyHandle`，票 06 下沉：它本身与 Tauri 无关
/// （`std::sync::Mutex` + `tokio::task::JoinHandle`），只是过去经 `app.manage` 存在
/// Tauri 状态表里才留在带 tauri 的 crate。
pub struct ProxyHandle(pub StdMutex<Option<JoinHandle<()>>>);

impl ProxyHandle {
    pub fn new() -> Self {
        Self(StdMutex::new(None))
    }

    /// 代理是否在跑。锁中毒（持锁线程 panic）视为「没在跑」，与迁移前
    /// `proxy_status` 的 `map_err(|e| e.to_string())?` 语义等价地不 panic。
    pub fn is_running(&self) -> bool {
        self.0.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

impl Default for ProxyHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// 命令函数可用的全部「外壳能力」。
///
/// 分两类：
/// - **依赖容器**（[`db`](AppCtx::db) / [`middleware`](AppCtx::middleware) /
///   [`proxy_handle`](AppCtx::proxy_handle)）：两种外壳都必须提供，无默认实现。
/// - **界面副作用**（emit / 弹窗 / TTS / 开机自启）：桌面壳有，无界面内核可能没有，
///   故带默认实现（no-op 或 `Err`），内核按需覆盖。默认实现**只允许退化，不允许 panic**。
///
/// 托盘渲染（AppKit `NSStatusItem`）**不在此 trait 内**，整体留在桌面壳侧
/// （`aidog_core::tray_render` 的 `build_tray_menu` / `refresh_tray_menu` 仍吃 `AppHandle`）。
pub trait AppCtx: Send + Sync + 'static {
    /// 主 `Db` 实例（与代理热路径同一个连接，禁另开）。
    fn db(&self) -> &Db;

    /// 中间件规则引擎单例（CRUD reload 与代理消费同源）。
    fn middleware(&self) -> &Arc<MiddlewareEngine>;

    /// 代理运行句柄。
    fn proxy_handle(&self) -> &ProxyHandle;

    /// 向界面广播事件（桌面壳 = Tauri `emit`；无界面内核 = SSE 广播，票 08）。
    ///
    /// 全部为 fire-and-forget：没有界面在听不是错误。
    fn emit(&self, event: &str, payload: serde_json::Value);

    /// 系统通知弹窗。返回是否真的弹了（无桌面会话时 `false`）。
    fn show_popup(&self, _title: &str, _body: &str) -> bool {
        false
    }

    /// 把 TTS 文本交给界面朗读（WebSpeech 后端）。返回是否真的交出去了。
    /// 其余 TTS 后端（`say` / tts crate）是纯进程内副作用，不经本 trait。
    fn speak_via_ui(&self, _text: &str) -> bool {
        false
    }

    /// 读开机自启开关。
    fn autolaunch_enabled(&self) -> Result<bool, String> {
        Err("autolaunch unsupported in this shell".to_string())
    }

    /// 写开机自启开关。
    fn set_autolaunch(&self, _enabled: bool) -> Result<(), String> {
        Err("autolaunch unsupported in this shell".to_string())
    }
}

static CTX: OnceLock<Arc<dyn AppCtx>> = OnceLock::new();

/// 安装进程级 `AppCtx`。外壳启动时调一次。
///
/// 重复调用是启动流程写错了（两个外壳同进程），故 panic 而不是静默忽略——静默会让
/// 后续所有命令读到第一个 ctx 的 `Db`，症状是「改了配置没生效」，比崩溃难查得多。
pub fn install(ctx: Arc<dyn AppCtx>) {
    if CTX.set(ctx).is_err() {
        panic!("aidog_ctx::install called twice");
    }
}

/// 取进程级 `AppCtx`。未安装则返回 `None`（后台任务在 `install` 之前跑到时用）。
pub fn try_ctx() -> Option<&'static dyn AppCtx> {
    CTX.get().map(|c| c.as_ref())
}

/// 取进程级 `AppCtx`。
///
/// # Panics
/// 未 `install` 时 panic。命令只在外壳就绪后才可能被调用，走到这里就是接线错误。
pub fn ctx() -> &'static dyn AppCtx {
    try_ctx().expect("aidog_ctx::install not called before command dispatch")
}

/// [`ctx()`]`.db()` 的简写——命令体里最高频的一句。
pub fn db() -> &'static Db {
    ctx().db()
}

/// 未安装 ctx 时返回 `None` 的 `db()`（启动期后台任务用）。
pub fn try_db() -> Option<&'static Db> {
    try_ctx().map(|c| c.db())
}

/// 广播事件；ctx 未安装时静默丢弃（启动期 / 无头测试）。
pub fn emit(event: &str, payload: serde_json::Value) {
    if let Some(c) = try_ctx() {
        c.emit(event, payload);
    }
}

/// 广播无 payload 的事件（对齐 Tauri `emit(name, ())` 的 `null` 序列化结果）。
pub fn emit_unit(event: &str) {
    emit(event, serde_json::Value::Null);
}
