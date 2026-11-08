//! 隐私脱敏设置命令（票 04）：读写 scope=`privacy` key=`scrub` + 「试一试」预览。
//! 预览走与生产同一条 `scrub_json_with_model` 路径（规则 + 本地模型合并）；文本包一层
//! `{"text": ...}` 只借 JSON 叶子遍历，不做「厂商实际收到什么」的实时抓取对照（越界）。

use crate::gateway::privacy::{self, PrivacyScrubSettings};
use crate::gateway::models::SetSettingInput;
use aidog_db::Db;
use serde::Serialize;

/// 「试一试」预览结果：原文 / 脱敏后 / 命中条目数。
#[derive(Debug, Clone, Serialize)]
pub struct PrivacyScrubPreview {
    pub original: String,
    pub scrubbed: String,
    pub hits: usize,
}

/// 读取脱敏设置（缺省 / 解析失败 → 默认全关）。
pub async fn privacy_scrub_settings(db: &Db) -> PrivacyScrubSettings {
    privacy::get_privacy_scrub_settings(db).await
}

/// 保存脱敏设置并刷新代理设置缓存（请求路径读 settings_cache 快照，不刷则要重启代理才生效）。
pub async fn save_privacy_scrub_settings(
    db: &Db,
    settings: &PrivacyScrubSettings,
) -> Result<(), String> {
    aidog_db::set_setting(db, SetSettingInput {
        scope: "privacy".to_string(),
        key: "scrub".to_string(),
        value: serde_json::to_value(settings).map_err(|e| format!("serialize: {e}"))?,
    })
    .await
    .map_err(|e| {
        tracing::error!(command = "privacy_scrub_set", error = %e, "persist privacy scrub settings failed");
        e
    })?;
    crate::gateway::proxy::refresh_proxy_settings_cache(db).await;
    Ok(())
}

/// 「试一试」预览：总开关关 → 原样返回（hits=0）；开 → 生产 scrub 路径（含模型合并）。
/// 模型检测失败 → Err（fail-closed，与生产 503 同语义，预览不展示半脱敏结果）。
pub async fn scrub_preview_text(db: &Db, text: &str) -> Result<PrivacyScrubPreview, String> {
    let settings = privacy::get_privacy_scrub_settings(db).await;
    if !settings.enabled {
        return Ok(PrivacyScrubPreview {
            original: text.to_string(),
            scrubbed: text.to_string(),
            hits: 0,
        });
    }
    let mut body = serde_json::json!({ "text": text });
    let map = privacy::scrub_json_with_model(&mut body, &privacy::scrub_options(&settings), &settings)
        .await
        .map_err(|_| "检测服务不可用（fail-closed），无法预览".to_string())?;
    Ok(PrivacyScrubPreview {
        original: text.to_string(),
        scrubbed: body["text"].as_str().unwrap_or(text).to_string(),
        hits: map.len(),
    })
}

crate::tauri_command! {
/// 当前脱敏设置。
pub async fn privacy_scrub_get() -> PrivacyScrubSettings {
    privacy_scrub_settings(aidog_ctx::db()).await
}
}

crate::tauri_command! {
/// 保存脱敏设置（整体替换）。
pub async fn privacy_scrub_set(settings: PrivacyScrubSettings) -> Result<(), String> {
    save_privacy_scrub_settings(aidog_ctx::db(), &settings).await
}
}

crate::tauri_command! {
/// 「试一试」预览。
pub async fn privacy_scrub_preview(text: String) -> Result<PrivacyScrubPreview, String> {
    scrub_preview_text(aidog_ctx::db(), &text).await
}
}

#[cfg(test)]
#[path = "test_privacy.rs"]
mod test_privacy;
