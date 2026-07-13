

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

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub fn about_info() -> AboutInfo {
    tracing::debug!(command = "about_info", "command invoked");
    AboutInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: std::env::consts::FAMILY.to_string(),
        profile: if cfg!(debug_assertions) { "debug" } else { "release" }.to_string(),
        git_commit: env!("AIDOG_GIT_COMMIT").to_string(),
        build_time: env!("AIDOG_BUILD_TIME").to_string(),
    }
}

#[cfg(test)]
#[path = "test_about.rs"]
mod test_about;
