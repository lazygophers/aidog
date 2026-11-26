//! 内核自主更新（票 magpie-05）验收测试：全部打本地假下载服务，不访问外部服务。
//!
//! 覆盖：已知/未知大小下载、断线、签名校验失败拒绝安装、替换失败回退、确认后排空重启。
//! 测试密钥是 `cargo-tauri signer generate` 生成的一次性密钥（TEST ONLY，与真实发行密钥无关），
//! 对固定载荷 `aidog-kernel-update-test-payload-v1\n` 以 `--app-version 9.9.9` 预签名。

use super::*;
use aidog_db::test_support::HomeGuard;
use std::sync::Arc;

/// 与真实发行密钥无关的一次性测试公钥（base64，内容 = minisign.pub 两行文本）。
const TEST_PUBKEY_B64: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDQ1QkQ3NDlDMDI1MzE4MkIKUldRckdGTUNuSFM5UlpRcHZJNnM2Q0FKQjVuUTN5VVZtUEpBR1FWZnN0S0lNLzMxNUtndkthS00K";
/// 上面载荷的签名（base64，内容 = `tauri signer sign --app-version 9.9.9` 产出的 .sig）。
const TEST_SIG_B64: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVRckdGTUNuSFM5UlZKR0dCODJraGsrUjNxMHppbHZrREtnVFhDZ2NxaDkrZDNocVNUbzlUTmJhNXZtU1NURnhRdmNRcEh3UjBKSnNVRVdYelQ3NjdDMklYQWxLRUhhUFFrPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk1NjI3MzM4CWZpbGU6cGF5bG9hZC5iaW4JdmVyc2lvbjo5LjkuOQo4UUtLZFpjcTBUeUlxLytHKytEMHlYVE1SOHNMMHlFQ1gxaWJaNzNQc0V1QWNwSm9zNjlFTm1HT1lZYVRiUEs4VzBkbUNtTS9COE9YTy9ydEhlaDJBQT09Cg==";
/// 被签载荷的 base64（解码后 = `aidog-kernel-update-test-payload-v1\n`，36 字节）。
const PAYLOAD_B64: &str = "YWlkb2cta2VybmVsLXVwZGF0ZS10ZXN0LXBheWxvYWQtdjEK";

/// 全局 phase / 环境变量 / 进程 HOME 都是进程级：整个测试模块串行。
// tokio Mutex：异步测试持锁跨 await（HomeGuard / env / 假服务都在锁内），
// 不触发 clippy await_holding_lock（先例 test_registry.rs:1088）。
static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn payload() -> Vec<u8> {
    b64_decode(PAYLOAD_B64)
}

fn b64_decode(s: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).unwrap()
}

#[derive(Clone, Copy, PartialEq)]
enum PkgMode {
    /// 正常响应，带 content-length。
    Full,
    /// chunked，无 content-length（未知大小）。
    NoLength,
    /// content-length 声称 1000 但提前断流（模拟断线）。
    Truncated,
}

struct FakeServer {
    addr: String,
    _handle: tokio::task::JoinHandle<()>,
}

async fn spawn_fake_server(pkg_mode: PkgMode, manifest_version: &str) -> FakeServer {
    spawn_fake_server_with_pkg_override(None, pkg_mode, manifest_version).await
}

/// `pkg_override` 覆盖 /pkg 返回的字节（None = 签名载荷本体，用于验签失败用例）。
async fn spawn_fake_server_with_pkg_override(
    pkg_override: Option<Vec<u8>>,
    pkg_mode: PkgMode,
    manifest_version: &str,
) -> FakeServer {
    let pkg = pkg_override.unwrap_or_else(payload);
    let key = platform_key();
    let manifest = serde_json::json!({
        "version": manifest_version,
        "platforms": { key: { "url": "PKG_URL_PLACEHOLDER", "signature": TEST_SIG_B64 } }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let manifest = manifest
        .to_string()
        .replace("PKG_URL_PLACEHOLDER", &format!("http://{addr}/pkg"));
    let app = axum::Router::new()
        .route(
            "/latest.json",
            axum::routing::get(move || {
                let manifest = manifest.clone();
                async move {
                    (
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        manifest,
                    )
                }
            }),
        )
        .route(
            "/pkg",
            axum::routing::get(move || {
                let pkg = pkg.clone();
                async move { pkg_response(pkg_mode, pkg).await }
            }),
        );
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    FakeServer {
        addr,
        _handle: handle,
    }
}

/// 整份清单由调用方给（平台键缺失等形状用例）；/pkg 仍返回签名载荷。
async fn spawn_fake_server_with_manifest(manifest: serde_json::Value) -> FakeServer {
    let pkg = payload();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let app = axum::Router::new()
        .route(
            "/latest.json",
            axum::routing::get(move || {
                let manifest = manifest.to_string();
                async move {
                    (
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        manifest,
                    )
                }
            }),
        )
        .route(
            "/pkg",
            axum::routing::get(move || {
                let pkg = pkg.clone();
                async move { pkg_response(PkgMode::Full, pkg).await }
            }),
        );
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    FakeServer {
        addr,
        _handle: handle,
    }
}

async fn pkg_response(mode: PkgMode, pkg: Vec<u8>) -> axum::response::Response {
    use axum::body::Body;
    use axum::response::IntoResponse;
    match mode {
        PkgMode::Full => (
            [(axum::http::header::CONTENT_LENGTH, pkg.len().to_string())],
            pkg,
        )
            .into_response(),
        PkgMode::NoLength => {
            // 分块 stream：axum chunked 编码不带 content-length。
            let chunks: Vec<Result<_, std::io::Error>> = pkg
                .chunks(10)
                .map(|c| Ok::<_, std::io::Error>(c.to_vec()))
                .collect();
            Body::from_stream(futures::stream::iter(chunks)).into_response()
        }
        PkgMode::Truncated => {
            // chunked 中途断流：先给一块数据再断（hyper 会拒绝谎报的 content-length 头，
            // 不能用它模拟断线；chunked 提前 Err = 连接中断，客户端在 body 中途收到错误）。
            use futures::StreamExt;
            let first = pkg[..10.min(pkg.len())].to_vec();
            let stream = futures::stream::once(async move { Ok::<Vec<u8>, std::io::Error>(first) })
                .chain(futures::stream::once(async {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        "simulated disconnect",
                    ))
                }));
            Body::from_stream(stream).into_response()
        }
    }
}

/// 把清单地址环境变量指到假服务；恢复语义由 TEST_LOCK 串行兜底。
fn set_manifest_env(addr: &str) {
    // edition 2024 起 set_var 是 unsafe：测试经 TEST_LOCK 串行，无并发 env 访问。
    unsafe {
        std::env::set_var(
            "AIDOG_UPDATE_MANIFEST_URL",
            format!("http://{addr}/latest.json"),
        );
    }
}

fn set_exe_env(exe: &std::path::Path) {
    unsafe {
        std::env::set_var("AIDOG_UPD_EXE_FOR_TEST", exe);
    }
}

/// 验签公钥指向测试密钥（生产默认走 tauri.conf.json 内嵌正式公钥，不经此覆盖）。
fn set_test_pubkey_env() {
    unsafe {
        std::env::set_var("AIDOG_UPDATE_PUBKEY_B64", TEST_PUBKEY_B64);
    }
}

fn ready_version() -> String {
    match get_phase() {
        Phase::ReadyToInstall { version, .. } => version,
        other => panic!("expected ready_to_install, got {:?}", other.phase_name()),
    }
}

fn staged_path() -> PathBuf {
    match get_phase() {
        Phase::ReadyToInstall { staged, .. } => PathBuf::from(staged),
        Phase::Failed { error, .. } => panic!("phase failed: {error}"),
        ref other => panic!("unexpected phase {}", other.phase_name()),
    }
}

#[tokio::test]
async fn known_size_download_verifies_and_installs() {
    let _lock = TEST_LOCK.lock().await;
    reset_state_for_test();
    let _home = HomeGuard::new();
    let server = spawn_fake_server(PkgMode::Full, "9.9.9").await;
    set_manifest_env(&server.addr);
    set_test_pubkey_env();

    // 检查：有新版
    let pending = check_update().await.expect("check must succeed");
    let pending = pending.expect("9.9.9 > current, update must be available");
    assert_eq!(pending.version, "9.9.9");

    // 下载：验签通过落暂存
    download_update().await.expect("download must succeed");
    assert_eq!(ready_version(), "9.9.9");
    let staged = staged_path();
    assert_eq!(
        std::fs::read(&staged).unwrap(),
        payload(),
        "staged bytes must equal signed payload"
    );

    // 安装：替换注入的“现役二进制”，旧文件进 .bak
    let bindir = _home.dir.path().join("bin");
    std::fs::create_dir_all(&bindir).unwrap();
    let exe = bindir.join("aidog-kernel");
    std::fs::write(&exe, b"old-binary").unwrap();
    set_exe_env(&exe);

    let version = install_update().expect("install must succeed");
    assert_eq!(version, "9.9.9");
    assert_eq!(
        std::fs::read(&exe).unwrap(),
        payload(),
        "installed binary must be the new package"
    );
    let backup = bindir.join("aidog-kernel.bak");
    assert_eq!(
        std::fs::read(&backup).unwrap(),
        b"old-binary",
        "backup must hold the old binary"
    );
}

#[tokio::test]
async fn unknown_size_chunked_download_succeeds() {
    let _lock = TEST_LOCK.lock().await;
    reset_state_for_test();
    let _home = HomeGuard::new();
    let server = spawn_fake_server(PkgMode::NoLength, "9.9.9").await;
    set_manifest_env(&server.addr);
    set_test_pubkey_env();

    download_update()
        .await
        .expect("chunked download must succeed");
    let staged = staged_path();
    assert_eq!(std::fs::read(&staged).unwrap(), payload());
}

#[tokio::test]
async fn tampered_package_fails_verification_and_refuses_install() {
    let _lock = TEST_LOCK.lock().await;
    reset_state_for_test();
    let _home = HomeGuard::new();
    let server = spawn_fake_server_with_pkg_override(
        Some(b"tampered bytes".to_vec()),
        PkgMode::Full,
        "9.9.9",
    )
    .await;
    set_manifest_env(&server.addr);
    set_test_pubkey_env();

    let err = download_update()
        .await
        .expect_err("bad signature must fail");
    assert!(err.contains("signature verification failed"), "got: {err}");
    match get_phase() {
        Phase::Failed { .. } => {}
        other => panic!("phase must be failed, got {}", other.phase_name()),
    }
    // 拒绝安装：nothing ready
    let err = install_update().expect_err("install must refuse without verified package");
    assert!(err.contains("nothing ready"), "got: {err}");
    let updates_dir = _home.dir.path().join(".aidog/updates");
    assert!(
        !updates_dir.exists() || std::fs::read_dir(&updates_dir).unwrap().count() == 0,
        "no staged file may remain after failed verification"
    );
}

#[tokio::test]
async fn truncated_download_is_reported_as_failure() {
    let _lock = TEST_LOCK.lock().await;
    reset_state_for_test();
    let _home = HomeGuard::new();
    let server = spawn_fake_server(PkgMode::Truncated, "9.9.9").await;
    set_manifest_env(&server.addr);
    set_test_pubkey_env();

    let err = download_update()
        .await
        .expect_err("truncated body must fail");
    // 断线可发生在响应头前（hyper 对 body 错误直接断连 → "fetch package"）或 body 中途
    // （→ "download interrupted"）；断言的核心契约是失败 + Failed 相 + 不落暂存。
    assert!(
        err.contains("download interrupted") || err.contains("fetch package"),
        "got: {err}"
    );
    match get_phase() {
        Phase::Failed { .. } => {}
        other => panic!("phase must be failed, got {}", other.phase_name()),
    }
}

#[tokio::test]
async fn manifest_without_kernel_platform_entry_is_rejected() {
    let _lock = TEST_LOCK.lock().await;
    reset_state_for_test();
    let _home = HomeGuard::new();
    // 平台键存在与否取决于本机 OS/ARCH：构造一个只带别的键的清单 → 必报缺平台。
    let server = spawn_fake_server_with_manifest(serde_json::json!({
        "version": "9.9.9",
        "platforms": { "other-os-other-arch-kernel": { "url": "http://x/pkg", "signature": "" } }
    }))
    .await;
    set_manifest_env(&server.addr);
    set_test_pubkey_env();

    let err = download_update()
        .await
        .expect_err("missing platform entry must fail");
    assert!(err.contains("no entry for platform"), "got: {err}");
}

#[tokio::test]
async fn replace_failure_rolls_back_old_binary() {
    let _lock = TEST_LOCK.lock().await;
    reset_state_for_test();
    let _home = HomeGuard::new();
    let server = spawn_fake_server(PkgMode::Full, "9.9.9").await;
    set_manifest_env(&server.addr);
    set_test_pubkey_env();
    download_update().await.expect("setup download");

    let bindir = _home.dir.path().join("bin");
    std::fs::create_dir_all(&bindir).unwrap();
    let exe = bindir.join("aidog-kernel");
    std::fs::write(&exe, b"old-binary").unwrap();
    // 备份路径被非空目录占住 → rename(exe → backup) 必败 → 现役二进制原样保留。
    let backup = bindir.join("aidog-kernel.bak");
    std::fs::create_dir_all(backup.join("occupied")).unwrap();
    set_exe_env(&exe);

    let err = install_update().expect_err("install must fail when backup target is occupied");
    assert!(err.contains("backup current binary"), "got: {err}");
    assert_eq!(
        std::fs::read(&exe).unwrap(),
        b"old-binary",
        "current binary must be intact"
    );
    // 清掉占位目录再装一次 → 成功（失败尽力恢复路径可继续）。
    std::fs::remove_dir_all(&backup).unwrap();
    install_update().expect("retry after clearing the occupied backup path must succeed");
    assert_eq!(std::fs::read(&exe).unwrap(), payload());
}

#[tokio::test]
async fn confirm_rejects_new_requests_then_restarts_after_drain() {
    let _lock = TEST_LOCK.lock().await;
    reset_state_for_test();
    let _home = HomeGuard::new();
    let server = spawn_fake_server(PkgMode::Full, "9.9.9").await;
    set_manifest_env(&server.addr);
    set_test_pubkey_env();
    download_update().await.expect("setup download");

    let restarted = Arc::new(Mutex::new(false));
    let flag = restarted.clone();
    set_restart_hook(Box::new(move || {
        *flag.lock().unwrap() = true;
        true // 接管，测试内不真退出
    }));

    // 一条在途请求挂住排空：confirm 不能立刻重启，也不能放新请求进来。
    let inflight = aidog_core::gateway::proxy::admit().expect("admit before confirm");
    let confirm = tokio::spawn(confirm_and_restart());

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        !*restarted.lock().unwrap(),
        "must not restart while requests are in flight"
    );
    assert!(
        aidog_core::gateway::proxy::admit().is_none(),
        "new requests must be rejected once confirmed"
    );
    match get_phase() {
        Phase::Restarting => {}
        other => panic!("phase must be restarting, got {}", other.phase_name()),
    }

    drop(inflight);
    let confirm_result = tokio::time::timeout(std::time::Duration::from_secs(2), confirm)
        .await
        .expect("confirm must finish after drain");
    confirm_result
        .expect("confirm task must not panic")
        .expect("confirm must succeed");
    assert!(
        *restarted.lock().unwrap(),
        "restart hook must fire after drain"
    );
    assert_eq!(aidog_core::gateway::proxy::inflight(), 0);
}

#[tokio::test]
async fn confirm_without_ready_package_is_refused() {
    let _lock = TEST_LOCK.lock().await;
    reset_state_for_test();
    let _home = HomeGuard::new();
    let err = confirm_and_restart()
        .await
        .expect_err("confirm must refuse when nothing is staged");
    assert!(err.contains("nothing ready"), "got: {err}");
}

// ─── 纯函数：版本比较与验签 ────────────────────────────────────────────────

#[test]
fn semver_compare_rejects_equal_and_older() {
    let cur = parse_version(env!("CARGO_PKG_VERSION")).unwrap();
    let v999 = parse_version("9.9.9").unwrap();
    let v001 = parse_version("0.0.1").unwrap();
    assert!(v999 > cur);
    assert!(v001 < cur);
    assert_eq!(
        parse_version("v9.9.9").unwrap(),
        v999,
        "leading v must be tolerated"
    );
}

#[test]
fn verify_package_accepts_exact_payload_and_rejects_tampering() {
    // 同步测试：tokio Mutex 用 blocking_lock（当前线程 runtime 外也可用）。
    let _lock = TEST_LOCK.blocking_lock();
    reset_state_for_test();
    set_test_pubkey_env();
    // 正常：真实测试密钥签名 + 版本绑定一致
    verify_package(&payload(), TEST_SIG_B64, "9.9.9").expect("valid signature must pass");
    // 版本绑定不一致
    let err =
        verify_package(&payload(), TEST_SIG_B64, "8.8.8").expect_err("version mismatch must fail");
    assert!(err.contains("does not match announced"), "got: {err}");
    // 篡改包体
    let err =
        verify_package(b"other", TEST_SIG_B64, "9.9.9").expect_err("tampered bytes must fail");
    assert!(err.contains("signature verification failed"), "got: {err}");
    // 篡改签名文本
    let corrupt = TEST_SIG_B64.replace('Q', "A");
    assert!(
        verify_package(&payload(), &corrupt, "9.9.9").is_err(),
        "corrupted signature must fail"
    );
    // 缺 version 绑定的旧式签名：构造失败路径直接验“拒绝”语义由上面 mismatch 覆盖
}
