//! 内核自主更新（票 magpie-05）。
//!
//! 链路：清单 → 下载（字节进度）→ minisign 签名校验 → 原子替换（失败回退旧版）
//! → 管理接口确认 → 排空在途 → 退出交宿主拉起新版。
//!
//! 发行契约（主对话拍板，2026-11-26，见 `.scratch/memory/2026-11-26.md`）：与桌面共用
//! Tauri updater 的 Ed25519 密钥与 `latest.json` 清单（`tauri.conf.json` 的 endpoint 与
//! pubkey 经 `include_str!` 单点编译期内嵌，禁第二份拷贝）。内核平台键 = `OS-ARCH-kernel`
//! （桌面键 `OS-ARCH` 不冲突，desktop updater 忽略多出的键）。
//!
//! 校验语义（镜像 `tauri-plugin-updater` 2.14.1 `updater.rs::verify_signature` + 版本绑定）：
//! latest.json 本身不签名（上游 config.rs:117-124），签名只覆盖包体；因此除验签外还要求
//! 签名 trusted comment 的 `version:` == 清单 version，否则拒绝安装（防「旧包配新版本号」
//! 降级攻击）。清单缺该字段的旧签名一律拒绝。
//!
//! 部署契约（同日拍板）：内核装在服务用户可写目录（systemd 样例为 `/var/lib/aidog/bin`），
//! 服务进程自替换无需提权 helper。替换 = 备份旧文件 → rename 新文件，任一步失败即回退。
//!
//! 重启方式：确认后排空完成即 `exit(75)`（EX_TEMPFAIL），systemd `Restart=on-failure`
//! 拉起新版（样例单元 RestartSec=5）。测试经 [`set_restart_hook`] 注入，不真退出。
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::Serialize;

/// 排空等待上限：超过即带日志退出（长活连接不得无限期挂住重启）。
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// systemd `Restart=on-failure` 触发用的退出码（EX_TEMPFAIL）。
const RESTART_EXIT_CODE: i32 = 75;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Checking,
    Downloading { downloaded: u64, total: Option<u64> },
    ReadyToInstall { version: String, staged: String },
    Installing,
    Failed { error: String },
    Restarting,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LatestManifest {
    version: String,
    #[serde(default)]
    platforms: HashMap<String, PlatformEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PlatformEntry {
    url: String,
    signature: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingUpdate {
    version: String,
    url: String,
    signature: String,
}

fn state() -> &'static Mutex<Phase> {
    static STATE: OnceLock<Mutex<Phase>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(Phase::Idle))
}

fn set_phase(p: Phase) {
    *state().lock().expect("update state poisoned") = p;
}

fn get_phase() -> Phase {
    state().lock().expect("update state poisoned").clone()
}

/// 测试复位：confirm 测试会把全局 Phase 留在 Restarting，后续测试各自回 Idle。
#[cfg(test)]
pub fn reset_state_for_test() {
    *state().lock().expect("update state poisoned") = Phase::Idle;
}

impl Phase {
    fn phase_name(&self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Checking => "checking",
            Phase::Downloading { .. } => "downloading",
            Phase::ReadyToInstall { .. } => "ready_to_install",
            Phase::Installing => "installing",
            Phase::Failed { .. } => "failed",
            Phase::Restarting => "restarting",
        }
    }
}

/// 只允许从 Idle / Failed 进入活动态：并发更新互斥的唯一判定点。
fn try_begin(next: Phase) -> Result<(), String> {
    let mut s = state().lock().expect("update state poisoned");
    match *s {
        Phase::Idle | Phase::Failed { .. } => {
            *s = next;
            Ok(())
        }
        ref other => Err(format!(
            "update already in progress (phase: {})",
            other.phase_name()
        )),
    }
}

/// 桌面 updater 同源的 endpoint 与公钥，编译期内嵌自 `tauri.conf.json`（单一真值源）。
fn upstream() -> &'static (String, String) {
    static CFG: OnceLock<(String, String)> = OnceLock::new();
    CFG.get_or_init(|| {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../../tauri.conf.json"))
                .expect("tauri.conf.json is valid JSON");
        let u = &conf["plugins"]["updater"];
        (
            u["endpoints"][0]
                .as_str()
                .expect("updater endpoint present")
                .to_string(),
            u["pubkey"]
                .as_str()
                .expect("updater pubkey present")
                .to_string(),
        )
    })
}

/// 清单地址：`AIDOG_UPDATE_MANIFEST_URL` 覆盖（自建源 / 测试），缺省 = 桌面同源 endpoint。
pub fn manifest_url() -> String {
    let over = std::env::var("AIDOG_UPDATE_MANIFEST_URL").unwrap_or_default();
    if over.is_empty() {
        upstream().0.clone()
    } else {
        over
    }
}

/// 验签公钥：`AIDOG_UPDATE_PUBKEY_B64` 覆盖（自建源换自己的密钥 / 测试注入），缺省 = 桌面
/// 同源 pubkey。覆盖与清单 URL 覆盖同一信任层级：能改进程 env 的本就是部署者本人。
pub fn pubkey_b64() -> String {
    let over = std::env::var("AIDOG_UPDATE_PUBKEY_B64").unwrap_or_default();
    if over.is_empty() {
        upstream().1.clone()
    } else {
        over
    }
}

/// 本机在 latest.json 里的平台键：`OS-ARCH-kernel`（桌面键不带 `-kernel` 后缀）。
pub fn platform_key() -> String {
    format!("{}-{}-kernel", std::env::consts::OS, std::env::consts::ARCH)
}

/// 测试注入点：替换「当前二进制路径」。生产 = `current_exe()`。
fn exe_path() -> PathBuf {
    #[cfg(test)]
    if let Ok(p) = std::env::var("AIDOG_UPD_EXE_FOR_TEST") {
        return PathBuf::from(p);
    }
    std::env::current_exe().expect("current_exe unavailable")
}

/// 测试注入点：排空后调用；返回 true = 已接管（confirm 正常返回，不退出），false = 继续 exit(75)。
#[cfg(test)]
static RESTART_HOOK: Mutex<Option<Box<dyn Fn() -> bool + Send + Sync>>> = Mutex::new(None);

#[cfg(test)]
pub fn set_restart_hook(f: Box<dyn Fn() -> bool + Send + Sync>) {
    *RESTART_HOOK.lock().expect("hook poisoned") = Some(f);
}

fn parse_manifest(text: &str) -> Result<LatestManifest, String> {
    serde_json::from_str(text).map_err(|e| format!("manifest is not valid latest.json: {e}"))
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("build http client: {e}"))
}

async fn fetch_manifest() -> Result<LatestManifest, String> {
    let text = http_client()?
        .get(manifest_url())
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("fetch manifest: {e}"))?
        .text()
        .await
        .map_err(|e| format!("read manifest: {e}"))?;
    parse_manifest(&text)
}

/// 本机平台条目；缺失 = 该平台暂无内核发行资产。
/// 资产 URL scheme 必须与清单源同级或更严（清单走 TLS 时资产禁明文 http），防清单校验
/// 被降级到明文下载。自建源 / 测试（AIDOG_UPDATE_MANIFEST_URL 覆盖为 http）不受限。
fn entry_for_platform(m: &LatestManifest) -> Result<PendingUpdate, String> {
    let key = platform_key();
    let e = m.platforms.get(&key).ok_or_else(|| {
        format!("manifest has no entry for platform key \"{key}\" (asset not published?)")
    })?;
    let manifest_https = manifest_url().starts_with("https://");
    if manifest_https && !e.url.starts_with("https://") {
        return Err(format!(
            "package url must be https when the manifest is served over TLS, got: {}",
            e.url
        ));
    }
    Ok(PendingUpdate {
        version: m.version.clone(),
        url: e.url.clone(),
        signature: e.signature.clone(),
    })
}

fn parse_version(s: &str) -> Result<Version, String> {
    Version::parse(s.trim_start_matches('v')).map_err(|e| format!("invalid semver \"{s}\": {e}"))
}

/// 检查更新：拉清单、与当前版本比较。有新版返回 Some。
pub async fn check_update() -> Result<Option<PendingUpdate>, String> {
    try_begin(Phase::Checking)?;
    let result = fetch_manifest().await.and_then(|m| {
        if parse_version(&m.version)? > parse_version(env!("CARGO_PKG_VERSION"))? {
            Ok(Some(entry_for_platform(&m)?))
        } else {
            Ok(None)
        }
    });
    set_phase(Phase::Idle);
    result
}

/// 下载 + 落暂存文件（字节进度入状态）。校验不过不落盘并转 Failed。
pub async fn download_update() -> Result<PendingUpdate, String> {
    try_begin(Phase::Downloading {
        downloaded: 0,
        total: None,
    })?;
    let result = download_update_inner().await;
    if let Err(e) = &result {
        set_phase(Phase::Failed { error: e.clone() });
    }
    result
}

async fn download_update_inner() -> Result<PendingUpdate, String> {
    let pending = entry_for_platform(&fetch_manifest().await?)?;

    let mut resp = http_client()?
        .get(&pending.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("fetch package: {e}"))?;

    let total = resp.content_length();
    let mut data: Vec<u8> = Vec::new();
    loop {
        // reqwest 0.13：chunk() = Result<Option<Bytes>>（0.12 及以前才是 Option<Result>）。
        let chunk = match resp.chunk().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(e) => {
                return Err(format!(
                    "download interrupted after {} bytes: {e}",
                    data.len()
                ));
            }
        };
        data.extend_from_slice(&chunk);
        set_phase(Phase::Downloading {
            downloaded: data.len() as u64,
            total,
        });
    }

    // 验签通过才落盘：签名无效 / 版本不绑定 → 拒绝安装（ Failed 相）。
    verify_package(&data, &pending.signature, &pending.version)?;

    let data_dir = aidog_core::shared::aidog_data_dir()?;
    let dir = data_dir.join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let staged = dir.join(format!("kernel-{}.pkg", pending.version));
    let mut file =
        std::fs::File::create(&staged).map_err(|e| format!("create {}: {e}", staged.display()))?;
    file.write_all(&data)
        .map_err(|e| format!("write staged: {e}"))?;
    drop(file);

    set_phase(Phase::ReadyToInstall {
        version: pending.version.clone(),
        staged: staged.display().to_string(),
    });
    Ok(pending)
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("invalid base64: {e}"))
}

/// 镜像 `tauri-plugin-updater` 的验签 + 版本绑定：包体必须带有效 minisign 签名，
/// 且签名 trusted comment 的 `version:` == 清单 version（缺该字段即拒绝）。
fn verify_package(data: &[u8], signature_b64: &str, announced_version: &str) -> Result<(), String> {
    let public_key = PublicKey::decode(&String::from_utf8_lossy(&b64_decode(&pubkey_b64())?))
        .map_err(|e| format!("decode updater pubkey: {e}"))?;
    let signature = Signature::decode(&String::from_utf8_lossy(&b64_decode(signature_b64)?))
        .map_err(|e| format!("decode package signature: {e}"))?;
    public_key
        .verify(data, &signature, true)
        .map_err(|e| format!("signature verification failed: {e:?}"))?;
    // verify() 校验了 global signature 之后 trusted comment 才可信（上游 updater.rs 注释同口径）。
    let signed = signature
        .trusted_comment()
        .split('\t')
        .find_map(|f| f.strip_prefix("version:"))
        .ok_or("signature carries no signed version; refusing (downgrade protection)")?;
    let matches = match (parse_version(signed), parse_version(announced_version)) {
        (Ok(a), Ok(b)) => a == b,
        _ => signed == announced_version,
    };
    if matches {
        Ok(())
    } else {
        Err(format!(
            "signed version {signed} does not match announced {announced_version}; refusing"
        ))
    }
}

/// 验签通过（ReadyToInstall）才允许安装：原子替换现役二进制，失败回退旧版。
/// 返回 Err 时现役二进制保证未被破坏（先备份后替换，任一步失败即还原）。
pub fn install_update() -> Result<String, String> {
    let (version, staged) = match get_phase() {
        Phase::ReadyToInstall { version, staged } => (version, PathBuf::from(staged)),
        ref other => {
            return Err(format!(
                "nothing ready to install (phase: {})",
                other.phase_name()
            ));
        }
    };
    set_phase(Phase::Installing);
    let result = install_inner(&staged);
    match &result {
        Ok(()) => set_phase(Phase::ReadyToInstall {
            version: version.clone(),
            staged: staged.display().to_string(),
        }),
        Err(e) if e.contains("rollback: FAILED") => {
            // 回退也失败 = 现役二进制状态不明，只有这种情形才转 Failed；
            // 其余失败已回退成功，暂存包仍在，保持可重试（Failed 会挡住 install 重试）。
            set_phase(Phase::Failed { error: e.clone() })
        }
        Err(_) => set_phase(Phase::ReadyToInstall {
            version: version.clone(),
            staged: staged.display().to_string(),
        }),
    }
    result?;
    Ok(version)
}

fn install_inner(staged: &std::path::Path) -> Result<(), String> {
    if !staged.is_file() {
        return Err(format!("staged file missing: {}", staged.display()));
    }
    let exe = exe_path();
    let dir = exe.parent().ok_or("exe has no parent dir")?;
    // 写权限预检：部署目录不可写（如 /usr/local/bin + 非特权服务账户）先给明确报错。
    let probe = dir.join(format!(".aidog-upd-probe-{}", std::process::id()));
    if let Err(e) = std::fs::File::create(&probe) {
        return Err(format!(
            "no write permission in {} (deploy the kernel to a service-user-writable dir): {e}",
            dir.display()
        ));
    }
    let _ = std::fs::remove_file(&probe);

    let backup = dir.join("aidog-kernel.bak");
    // staging 在数据目录，可能与二进制目录跨文件系统：先复制到目标目录旁的临时名再 rename。
    let local_tmp = dir.join(format!(".aidog-upd-new-{}", std::process::id()));
    std::fs::copy(staged, &local_tmp).map_err(|e| format!("stage copy: {e}"))?;

    std::fs::rename(&exe, &backup).map_err(|e| {
        let _ = std::fs::remove_file(&local_tmp);
        format!("backup current binary: {e}")
    })?;
    match std::fs::rename(&local_tmp, &exe) {
        Ok(()) => Ok(()),
        Err(e) => {
            // 替换失败：回退旧版，不留半死状态。
            let rb = std::fs::rename(&backup, &exe);
            let _ = std::fs::remove_file(&local_tmp);
            Err(format!(
                "replace binary: {e}; rollback: {}",
                rb.map_or_else(|re| format!("FAILED: {re}"), |_| "ok".to_string())
            ))
        }
    }
}

/// 确认安装 → 拒新请求 + 排空在途 → 退出交宿主拉起新版。必须已 ReadyToInstall（防误触）。
pub async fn confirm_and_restart() -> Result<(), String> {
    match get_phase() {
        Phase::ReadyToInstall { .. } => {}
        ref other => {
            return Err(format!(
                "nothing ready to install (phase: {})",
                other.phase_name()
            ));
        }
    }
    if !aidog_core::gateway::proxy::begin_drain() {
        return Err("restart already in progress".into());
    }
    set_phase(Phase::Restarting);
    // 确认后不再接新请求（drain 已置位），等在途清零；超时兜底退出，不无限挂住。
    let _ = tokio::time::timeout(DRAIN_TIMEOUT, aidog_core::gateway::proxy::wait_empty()).await;
    tracing::info!("update confirmed: drained (or timeout), exiting for restart");
    // 测试接管：hook 返回 true 时 confirm 正常返回，不真退出（进程退出会杀掉测试）。
    #[cfg(test)]
    {
        if let Some(f) = RESTART_HOOK.lock().expect("hook poisoned").take()
            && f()
        {
            return Ok(());
        }
    }
    std::process::exit(RESTART_EXIT_CODE);
}

/// 供管理接口读取的更新状态。
pub fn status() -> serde_json::Value {
    let phase = get_phase();
    serde_json::json!({
        "current_version": env!("CARGO_PKG_VERSION"),
        "platform_key": platform_key(),
        "updates_enabled": true,
        "inflight": aidog_core::gateway::proxy::inflight(),
        "phase": phase,
    })
}

fn err_response(e: String) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    let status = if e.starts_with("fetch") || e.starts_with("download interrupted") {
        axum::http::StatusCode::BAD_GATEWAY
    } else {
        axum::http::StatusCode::BAD_REQUEST
    };
    (status, axum::Json(serde_json::json!({ "error": e })))
}

/// 内核更新命令挂到代理端口的管理路由上（ADR 0010 闸门照常生效：本机 / 开关 / 凭据）。
/// 只在内核装；桌面壳不经这组路由（桌面有自己的 updater 流程）。
pub fn install_routes(router: axum::Router) -> axum::Router {
    use axum::routing::post;
    router
        .route("/rpc/kernel_update_status", post(rpc_status))
        .route("/rpc/kernel_update_check", post(rpc_check))
        .route("/rpc/kernel_update_download", post(rpc_download))
        .route("/rpc/kernel_update_install", post(rpc_install))
        .route("/rpc/kernel_update_confirm", post(rpc_confirm))
}

async fn rpc_status() -> axum::Json<serde_json::Value> {
    axum::Json(status())
}

async fn rpc_check()
-> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
    match check_update().await {
        Ok(Some(p)) => Ok(axum::Json(serde_json::json!({
            "available": true, "version": p.version,
        }))),
        Ok(None) => Ok(axum::Json(serde_json::json!({ "available": false }))),
        Err(e) => Err(err_response(e)),
    }
}

async fn rpc_download()
-> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
    // 长任务后台跑；进度经 kernel_update_status 轮询。
    tokio::spawn(async move {
        if let Err(e) = download_update().await {
            tracing::warn!(error = %e, "kernel update download failed");
        }
    });
    Ok(axum::Json(serde_json::json!({ "started": true })))
}

async fn rpc_install()
-> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
    match install_update() {
        Ok(version) => Ok(axum::Json(
            serde_json::json!({ "installed": true, "version": version }),
        )),
        Err(e) => Err(err_response(e)),
    }
}

async fn rpc_confirm()
-> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
    // 排空 + 退出是长过程：先应答再后台执行（退出会掐断本连接，不能等它）。
    if !matches!(get_phase(), Phase::ReadyToInstall { .. }) {
        return Err(err_response(
            "nothing ready to install (download and install first)".into(),
        ));
    }
    tokio::spawn(async move {
        if let Err(e) = confirm_and_restart().await {
            tracing::warn!(error = %e, "kernel update confirm failed");
        }
    });
    Ok(axum::Json(serde_json::json!({ "restarting": true })))
}

#[cfg(test)]
#[path = "update_test.rs"]
mod update_test;
