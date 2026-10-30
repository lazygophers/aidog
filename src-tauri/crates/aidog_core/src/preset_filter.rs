//! 预设清单过滤（纯函数）。build.rs 与 lib 测试共用同一份实现，保证「构建期口径 = 被测口径」。

/// 开源默认预设 id：缺它构建失败；`GITHUB_ACTIONS=true` 时清单只含它。
pub const OSS_ID: &str = "oss";

/// `ids` 须含 oss。`only_oss` 为真只留 oss，否则原样全量。缺 oss 返回 Err（构建失败）。
pub fn select_ids(ids: Vec<String>, only_oss: bool) -> Result<Vec<String>, String> {
    if !ids.iter().any(|i| i == OSS_ID) {
        return Err(format!("预设目录缺少必需的 {OSS_ID}.json"));
    }
    Ok(if only_oss { vec![OSS_ID.to_string()] } else { ids })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Vec<String> {
        ["lazygophers", "oss"].map(String::from).to_vec()
    }

    #[test]
    fn github_actions_keeps_only_oss() {
        assert_eq!(select_ids(ids(), true).unwrap(), vec!["oss"]);
    }

    #[test]
    fn local_build_keeps_all() {
        assert_eq!(select_ids(ids(), false).unwrap(), ids());
    }

    #[test]
    fn missing_oss_is_error() {
        assert!(select_ids(vec!["lazygophers".into()], false).is_err());
        assert!(select_ids(vec![], true).is_err());
    }
}
