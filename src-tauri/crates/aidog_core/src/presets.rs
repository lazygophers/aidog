//! 推荐配置预设：编译期清单（build.rs 按 `GITHUB_ACTIONS` 过滤）+ 列表 command。
//! 兜底（`sync_settings::bundled_oss_preset`）与前端列表共用同一份清单。

use serde::Serialize;

include!(concat!(env!("OUT_DIR"), "/claude_presets.rs"));

/// 预设显示名字段：仅供选择 UI，写出前必须剥离（Claude Code 不认识）。
pub const PRESET_NAME_KEY: &str = "_aidog_preset_name";

#[derive(Debug, Serialize, PartialEq)]
pub struct PresetItem {
    pub id: String,
    pub name: String,
    /// 已剥离显示名。
    pub config: serde_json::Value,
}

/// 清单 → 列表项；显示名缺省回落 id，config 一律剥离显示名。
pub fn build_items(manifest: &[(&str, &str)]) -> Result<Vec<PresetItem>, String> {
    manifest
        .iter()
        .map(|(id, raw)| {
            let mut config: serde_json::Value =
                serde_json::from_str(raw).map_err(|e| format!("preset {id}: {e}"))?;
            let name = config
                .as_object_mut()
                .and_then(|o| o.remove(PRESET_NAME_KEY))
                .and_then(|n| n.as_str().map(str::to_owned))
                .unwrap_or_else(|| (*id).to_string());
            Ok(PresetItem { id: (*id).to_string(), name, config })
        })
        .collect()
}

/// 内置清单里的 oss 预设（已剥离显示名）。
pub(crate) fn bundled_oss_config() -> serde_json::Value {
    build_items(CLAUDE_PRESETS)
        .ok()
        .and_then(|v| v.into_iter().find(|p| p.id == "oss"))
        .map(|p| p.config)
        .unwrap_or_else(|| serde_json::Value::Object(Default::default()))
}

/// 按 kind 取预设列表。当前仅 `claude`（codex/pi 由后续票扩白名单）。
pub fn list_presets(kind: &str) -> Result<Vec<PresetItem>, String> {
    match kind {
        "claude" => build_items(CLAUDE_PRESETS),
        other => Err(format!("unsupported preset kind: {other}")),
    }
}

crate::tauri_command! {
    /// 只读：返回内置推荐配置预设列表（id / 显示名 / 已剥离显示名的 config）。
    pub async fn settings_preset_list(kind: String) -> Result<Vec<PresetItem>, String> {
        list_presets(&kind)
    }
}

#[cfg(test)]
#[path = "preset_filter.rs"]
mod preset_filter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_strip_display_name_from_config() {
        let m = [
            ("a", r#"{"_aidog_preset_name":"A 名","env":{"X":"1"}}"#),
            ("b", r#"{"env":{}}"#),
        ];
        let items = build_items(&m).unwrap();
        assert_eq!(items[0].name, "A 名");
        assert!(items[0].config.get(PRESET_NAME_KEY).is_none());
        assert_eq!(items[0].config["env"]["X"], "1");
        assert_eq!(items[1].name, "b", "缺显示名回落 id");
        assert!(build_items(&[("bad", "{")]).is_err());
    }

    #[test]
    fn unknown_kind_is_error() {
        assert!(list_presets("codex").unwrap_err().contains("unsupported"));
    }

    /// 发版自检用：`GITHUB_ACTIONS=true` 长度 1 且 id=oss；不设则等于磁盘份数（>=2）。
    #[test]
    fn bundled_claude_preset_manifest_len() {
        let ids: Vec<&str> = CLAUDE_PRESETS.iter().map(|(i, _)| *i).collect();
        println!("CLAUDE_PRESETS len={} ids={ids:?}", ids.len());
        let list = list_presets("claude").unwrap();
        assert_eq!(list.len(), ids.len());
        assert!(list.iter().all(|p| p.config.get(PRESET_NAME_KEY).is_none()));
        let on_disk = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../defaults/settings"))
            .unwrap()
            .filter(|e| e.as_ref().unwrap().path().extension().is_some_and(|x| x == "json"))
            .count();
        if option_env!("GITHUB_ACTIONS") == Some("true") {
            assert_eq!(ids, ["oss"]);
        } else {
            assert_eq!(ids.len(), on_disk);
        }
    }
}
