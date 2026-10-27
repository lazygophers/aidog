#[derive(serde::Serialize)]
pub struct AboutInfo {
    app_version: String,
    tauri_version: String,
    os: String,
    arch: String,
    family: String,
    profile: String,
    /// build.rs 注入的 git 短 commit（无 git 时 "unknown"）
    git_commit: String,
    /// build.rs 注入的构建时间（epoch 秒字符串，前端格式化）
    build_time: String,
}

crate::tauri_command! {
pub fn about_info() -> AboutInfo {
    AboutInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        // 票 08：无界面内核不链 tauri（`desktop` feature 关），此时该字段为 "headless" ——
        // 关于页只在桌面壳展示，Web 形态下显示的是内核实际形态，不编造一个版本号。
        tauri_version: {
            #[cfg(feature = "desktop")]
            {
                tauri::VERSION.to_string()
            }
            #[cfg(not(feature = "desktop"))]
            {
                "headless".to_string()
            }
        },
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: std::env::consts::FAMILY.to_string(),
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
        .to_string(),
        git_commit: env!("AIDOG_GIT_COMMIT").to_string(),
        build_time: env!("AIDOG_BUILD_TIME").to_string(),
    }
}
}

crate::tauri_command! {
/// 关于页展示 SQLite（SQLCipher）加密密钥：先过 macOS 管理员验证（osascript 特权弹窗），
/// 通过才返回 64 位 hex key（sqlcipher 工具直接 `PRAGMA key = "x'<hex>'"` 可开库）。
/// 无测试：osascript 弹系统 GUI 窗，无头环境禁跑（db-pool-routing.md「测试不得弹系统 GUI 窗」）。
pub fn db_master_key_reveal() -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("admin verification unsupported on this platform".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        let ok = std::process::Command::new("osascript")
            .args(["-e", "do shell script \"true\" with administrator privileges"])
            .status()
            .map_err(|e| e.to_string())?
            .success();
        if !ok {
            return Err("admin verification failed or cancelled".to_string());
        }
        let key = aidog_db::crypt::master_key()?;
        Ok(key.iter().map(|b| format!("{b:02x}")).collect())
    }
}
}

#[cfg(test)]
#[path = "test_about.rs"]
mod test_about;
