//! statusline 脚本生成（原 `src/components/settings/statusline-gen.ts` 下沉 Rust）。
//!
//! 与 hook 脚本同一套机制：`do_sync_group_settings` 每次启动无条件重写
//! `~/.aidog/scripts/aidog-statusline.py` 与 `aidog-subagent-statusline.py`，
//! 并把 `statusLine` / `subagentStatusLine` 原生字段写进每组 settings。
//! 前端只剩只读预览（`preview`），不再生成、不再落盘。

use crate::shared::{
    aidog_scripts_dir, cleanup_legacy_root_script, cleanup_legacy_scripts_dir_file,
};
use aidog_db::Db;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::OnceLock;

/// 渲染引擎正文，逐字嵌进生成的 .py。真值源与 golden 回归用例共用同一个文件。
const ENGINE_PY: &str = include_str!("../../../../scripts/statusline-golden/engine.py");

/// PEP723 inline-metadata 头（仅 stdlib；shebang 是 python3 回退路径）。
const PEP723_HEADER: &str = concat!(
    "#!/usr/bin/env python3\n",
    "# /// script\n",
    "# requires-python = \">=3.8\"\n",
    "# dependencies = []\n",
    "# ///\n",
);

/// 各 segment 类型的默认选项（对齐 `statusline-segments.ts` 的 `SEGMENT_DEFS.defaultOptions`）。
const DEFAULT_OPTIONS: &str = r##"{
  "model": {"format": "short"},
  "context-bar": {"width": 10, "filled": "▓", "empty": "░"},
  "context-pct": {"suffix": "%"},
  "git": {"showRepo": false},
  "cost": {"showDuration": true},
  "rate-limits": {"windows": "both"},
  "separator": {"char": "·"},
  "group-balance": {"prefix": "余额 ", "dynamicColor": false},
  "group-spent": {"prefix": "$"},
  "group-window-cost": {"prefix": "折算$"},
  "group-coding": {"dynamicColor": false},
  "group-cache": {"prefix": "缓存 "},
  "group-tokens": {"prefix": ""},
  "cost-usd": {"prefix": "$"},
  "session-duration": {"format": "human"},
  "api-duration": {"format": "human"},
  "context-tokens": {"abbrev": true, "mode": "split"},
  "context-max": {"abbrev": true},
  "context-cache": {"abbrev": true, "mode": "tokens", "prefix": "缓存 "},
  "rate-limit-5h": {"showReset": false},
  "rate-limit-7d": {"showReset": false},
  "cwd": {"format": "basename"},
  "project-dir": {"format": "basename"},
  "session-id": {"truncate": true},
  "transcript-path": {"format": "basename"},
  "pr-number": {"prefix": "#"},
  "version": {"prefix": "v"},
  "thinking": {"label": "thinking"},
  "token-warn": {"label": "⚠200k"},
  "custom": {"expr": ".model.display_name"}
}"##;

/// 支持按数值自动着色的类型（`autoColor` 仅对这些生效）。
const VALUE_COLORABLE: &[&str] = &[
    "context-pct",
    "context-bar",
    "cost",
    "rate-limits",
    "cost-usd",
    "context-remaining",
    "rate-limit-5h",
    "rate-limit-7d",
    "session-duration",
    "api-duration",
];

/// 需要调 `/api/group-info` 的类型（任一出现即 `NEED_GROUP = True`）。
const GROUP_SEG_TYPES: &[&str] = &[
    "group-balance",
    "group-spent",
    "group-window-cost",
    "group-coding",
    "group-requests",
    "group-cache",
    "group-tokens",
    "group-route",
];

/// 未配置时的默认布局（对齐 `statusline-segments.ts` 的 `DEFAULT_SEGMENTS`）。
const DEFAULT_SEGMENTS: &str = r##"[
  {"id": "d-model", "type": "model", "enabled": true, "newline": false, "color": "#4A9EFF", "options": {"format": "short"}},
  {"id": "d-sep1", "type": "separator", "enabled": true, "newline": false, "options": {"char": " · "}},
  {"id": "d-tokens", "type": "context-tokens", "enabled": true, "newline": false, "color": "#BF5AF2", "options": {"mode": "sum", "abbrev": true}},
  {"id": "d-cost", "type": "cost-usd", "enabled": true, "newline": false, "color": "#8E8E93", "options": {"prefix": "$", "affixPre": "[", "affixSuf": "]·"}},
  {"id": "d-ctx", "type": "context-pct", "enabled": true, "newline": false, "color": "#34C759", "options": {}},
  {"id": "d-cache", "type": "context-cache", "enabled": true, "newline": false, "color": "#34C759", "options": {"mode": "hitrate", "prefix": "缓存 ", "affixPre": "·"}},
  {"id": "d-branch", "type": "git-branch", "enabled": true, "newline": true, "color": "#FFD60A", "options": {}},
  {"id": "d-worktree", "type": "worktree-name", "enabled": true, "newline": false, "options": {"affixPre": "·"}},
  {"id": "d-cwd", "type": "cwd", "enabled": true, "newline": false, "options": {"format": "full", "affixPre": "|"}},
  {"id": "d-coding", "type": "group-coding", "enabled": true, "newline": true, "options": {"dynamicColor": true}},
  {"id": "d-balance", "type": "group-balance", "enabled": true, "newline": false, "options": {"dynamicColor": true, "prefix": "$", "affixPre": "·"}},
  {"id": "d-wcost", "type": "group-window-cost", "enabled": true, "newline": false, "options": {"prefix": "$", "affixPre": "·"}},
  {"id": "d-route", "type": "group-route", "enabled": true, "newline": false, "options": {"affixPre": "·"}},
  {"id": "d-version", "type": "version", "enabled": true, "newline": false, "color": "#8E8E93", "options": {"prefix": "v", "affixPre": " · "}}
]"##;

/// 未配置时的 subagent 默认布局（对齐 `DEFAULT_SUBAGENT_SEGMENTS`）。
const DEFAULT_SUBAGENT_SEGMENTS: &str = r##"[
  {"id": "sa-badge", "type": "agent-badge", "enabled": true, "newline": false, "options": {}},
  {"id": "sa-name", "type": "custom", "enabled": true, "newline": false, "color": "#4A9EFF", "options": {"expr": ".label // .name // .id // \"?\""}},
  {"id": "sa-ctx", "type": "context-pct", "enabled": true, "newline": false, "color": "#34C759", "options": {"suffix": "%", "degradeZero": true, "affixPre": "·"}},
  {"id": "sa-tokens", "type": "context-tokens", "enabled": true, "newline": false, "color": "#BF5AF2", "options": {"mode": "sum", "abbrev": true, "affixPre": "·"}},
  {"id": "sa-dur", "type": "session-duration", "enabled": true, "newline": false, "color": "#8E8E93", "options": {"format": "human", "affixPre": "·"}}
]"##;

/// 单个 segment 的持久化形状（未知字段忽略，缺省即 false / None）。
#[derive(Deserialize)]
struct Seg {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    newline: bool,
    #[serde(default)]
    color: Option<String>,
    #[serde(default, rename = "autoColor")]
    auto_color: bool,
    #[serde(default)]
    align: Option<String>,
    #[serde(default)]
    options: Map<String, Value>,
}

fn default_options(ty: &str) -> Map<String, Value> {
    static TABLE: OnceLock<Map<String, Value>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        serde_json::from_str(DEFAULT_OPTIONS).expect("DEFAULT_OPTIONS must be a JSON object")
    });
    table
        .get(ty)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// `"#RRGGBB"` / `"#RGB"` → `"r;g;b"`（ANSI 参数串），非法返回 None。
fn hex_to_rgb(hex: &str) -> Option<String> {
    let h = hex.trim().trim_start_matches('#');
    let h: String = if h.len() == 3 {
        h.chars().flat_map(|c| [c, c]).collect()
    } else {
        h.to_string()
    };
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let part = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some(format!("{};{};{}", part(0)?, part(2)?, part(4)?))
}

/// 合并默认选项 + 颜色，得到运行期 spec。
fn seg_spec(seg: &Seg) -> Value {
    let mut opts = default_options(&seg.ty);
    for (k, v) in &seg.options {
        opts.insert(k.clone(), v.clone());
    }
    let use_auto = seg.auto_color && VALUE_COLORABLE.contains(&seg.ty.as_str());
    let rgb = if use_auto {
        None
    } else {
        seg.color.as_deref().and_then(hex_to_rgb)
    };
    json!({ "type": seg.ty, "opts": opts, "rgb": rgb, "autoColor": use_auto })
}

/// 按 `newline` 切行，行的 align 取该行首个 segment 的 align。
fn group_rows(segs: &[&Seg]) -> Vec<Value> {
    let mut rows: Vec<(String, Vec<Value>)> = Vec::new();
    for seg in segs {
        let need_new = match rows.last() {
            None => true,
            Some((_, cur)) => seg.newline && !cur.is_empty(),
        };
        if need_new {
            rows.push((
                seg.align.clone().unwrap_or_else(|| "left".into()),
                Vec::new(),
            ));
        }
        rows.last_mut()
            .expect("row pushed above")
            .1
            .push(seg_spec(seg));
    }
    rows.into_iter()
        .map(|(align, segs)| json!({ "align": align, "segs": segs }))
        .collect()
}

fn generate_main(segments: &[Seg]) -> String {
    let active: Vec<&Seg> = segments.iter().filter(|s| s.enabled).collect();
    if active.is_empty() {
        return format!("{PEP723_HEADER}print('')\n");
    }
    let rows = Value::Array(group_rows(&active));
    let need_group = active
        .iter()
        .any(|s| GROUP_SEG_TYPES.contains(&s.ty.as_str()));
    let config = B64.encode(rows.to_string());
    [
        PEP723_HEADER,
        "# Generated by aidog — do not edit manually",
        ENGINE_PY,
        "",
        "import base64",
        &format!(r#"ROWS = json.loads(base64.b64decode("{config}").decode("utf-8"))"#),
        &format!("NEED_GROUP = {}", if need_group { "True" } else { "False" }),
        "",
        "def main():",
        "    payload = json.loads(sys.stdin.read() or '{}')",
        "    gi = fetch_group_info() if NEED_GROUP else None",
        "    for line in render(payload, ROWS, gi):",
        r"        sys.stdout.write(line + '\n')",
        "",
        "main()",
        "",
    ]
    .join("\n")
}

fn generate_subagent(segments: &[Seg]) -> String {
    let active: Vec<&Seg> = segments.iter().filter(|s| s.enabled).collect();
    if active.is_empty() {
        return format!("{PEP723_HEADER}pass\n");
    }
    let specs = Value::Array(active.iter().map(|s| seg_spec(s)).collect());
    let config = B64.encode(specs.to_string());
    [
        PEP723_HEADER,
        "# Generated by aidog — do not edit manually (SubagentStatusLine)",
        ENGINE_PY,
        "",
        "import base64",
        &format!(r#"SEGS = json.loads(base64.b64decode("{config}").decode("utf-8"))"#),
        "",
        "def main():",
        "    payload = json.loads(sys.stdin.read() or '{}')",
        "    now = _now_epoch()",
        "    for line in render_subagent(payload, SEGS, now):",
        r"        sys.stdout.write(line + '\n')",
        "",
        "main()",
        "",
    ]
    .join("\n")
}

fn parse_segments(stored: Option<&Map<String, Value>>, is_main: bool) -> Vec<Seg> {
    let from_store = stored
        .and_then(|o| o.get("segments"))
        .and_then(|v| serde_json::from_value::<Vec<Seg>>(v.clone()).ok());
    from_store.unwrap_or_else(|| {
        let raw = if is_main {
            DEFAULT_SEGMENTS
        } else {
            DEFAULT_SUBAGENT_SEGMENTS
        };
        serde_json::from_str(raw).expect("default segments must parse")
    })
}

/// `_aidog_statusline` / `_aidog_subagent_statusline` 三态。
enum Materialized {
    Disabled,
    /// 用户自定义命令（已 trim；空串按禁用处理）。
    Custom(String),
    /// 内置模式生成的脚本正文。
    Builtin(String),
}

fn materialize(stored: Option<&Value>, is_main: bool) -> Materialized {
    let obj = stored.and_then(Value::as_object);
    let enabled = obj
        .and_then(|o| o.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !enabled {
        return Materialized::Disabled;
    }
    if obj.and_then(|o| o.get("mode")).and_then(Value::as_str) == Some("custom") {
        let cmd = obj
            .and_then(|o| o.get("customCommand"))
            .and_then(Value::as_str)
            .unwrap_or("");
        return Materialized::Custom(cmd.trim().to_string());
    }
    let segments = parse_segments(obj, is_main);
    Materialized::Builtin(if is_main {
        generate_main(&segments)
    } else {
        generate_subagent(&segments)
    })
}

/// 只读预览：返回内置模式下的脚本正文，非内置/未启用返回空串。不落盘。
pub fn preview(stored: &Value, is_main: bool) -> String {
    match materialize(Some(stored), is_main) {
        Materialized::Builtin(s) => s,
        _ => String::new(),
    }
}

fn write_script(filename: &str, legacy_sh: &str, content: &str) -> Result<String, String> {
    let scripts_dir = aidog_scripts_dir()?;
    // 迁移清理：删除旧版 bash 脚本（~/.aidog/ 根 + scripts/ 下）。
    cleanup_legacy_root_script(filename);
    cleanup_legacy_root_script(legacy_sh);
    cleanup_legacy_scripts_dir_file(&scripts_dir, legacy_sh);
    let path = scripts_dir.join(filename);
    std::fs::write(&path, content).map_err(|e| format!("write script: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)
            .map_err(|e| format!("stat script: {e}"))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).map_err(|e| format!("chmod script: {e}"))?;
    }
    Ok(path.to_string_lossy().into_owned())
}

/// 落盘两个 statusline 脚本，返回要写进每组 settings 的原生字段值
/// （`None` = 删该字段）。每次启动无条件重写，与 hook 脚本同机制。
pub async fn prepare_statusline_fields(
    db: &Db,
    base_config: &Value,
) -> Result<[(&'static str, Option<Value>); 2], String> {
    let invoker = crate::shared::resolve_script_invoker(db).await;
    let targets = [
        (
            "statusLine",
            "_aidog_statusline",
            true,
            "aidog-statusline.py",
            "aidog-statusline.sh",
        ),
        (
            "subagentStatusLine",
            "_aidog_subagent_statusline",
            false,
            "aidog-subagent-statusline.py",
            "aidog-subagent-statusline.sh",
        ),
    ];
    let mut out = [("statusLine", None), ("subagentStatusLine", None)];
    for (i, (field, aidog_key, is_main, filename, legacy_sh)) in targets.into_iter().enumerate() {
        let value = match materialize(base_config.get(aidog_key), is_main) {
            Materialized::Disabled => None,
            Materialized::Custom(cmd) if cmd.is_empty() => None,
            Materialized::Custom(cmd) => Some(json!({ "type": "command", "command": cmd })),
            Materialized::Builtin(script) => {
                let path = write_script(filename, legacy_sh, &script)?;
                Some(json!({ "type": "command", "command": invoker.command_for(&path) }))
            }
        };
        out[i] = (field, value);
    }
    Ok(out)
}

/// 把 `prepare_statusline_fields` 的结果写进一组 settings。
pub fn inject_statusline(config: &mut Value, fields: &[(&'static str, Option<Value>); 2]) {
    let Some(obj) = config.as_object_mut() else {
        return;
    };
    for (field, value) in fields {
        match value {
            Some(v) => {
                obj.insert((*field).to_string(), v.clone());
            }
            None => {
                obj.remove(*field);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segs(raw: &str) -> Vec<Seg> {
        serde_json::from_str(raw).unwrap()
    }

    #[test]
    fn hex_to_rgb_parses_both_forms() {
        assert_eq!(hex_to_rgb("#4A9EFF").unwrap(), "74;158;255");
        assert_eq!(hex_to_rgb("#f00").unwrap(), "255;0;0");
        assert_eq!(hex_to_rgb(" 34C759 ").unwrap(), "52;199;89");
        assert!(hex_to_rgb("#12345").is_none());
        assert!(hex_to_rgb("#GGGGGG").is_none());
        assert!(hex_to_rgb("").is_none());
    }

    #[test]
    fn default_options_merge_and_override() {
        let s = &segs(r##"[{"type":"model","enabled":true,"options":{"format":"long"}}]"##)[0];
        let spec = seg_spec(s);
        assert_eq!(spec["opts"]["format"], "long");

        let s = &segs(r##"[{"type":"context-pct","enabled":true}]"##)[0];
        assert_eq!(seg_spec(s)["opts"]["suffix"], "%");
    }

    #[test]
    fn auto_color_only_for_value_colorable() {
        let s = &segs(
            r##"[{"type":"context-pct","enabled":true,"autoColor":true,"color":"#ffffff"}]"##,
        )[0];
        let spec = seg_spec(s);
        assert_eq!(spec["autoColor"], true);
        assert!(spec["rgb"].is_null());

        // model 不在 VALUE_COLORABLE：autoColor 失效，颜色照常生效。
        let s =
            &segs(r##"[{"type":"model","enabled":true,"autoColor":true,"color":"#ffffff"}]"##)[0];
        let spec = seg_spec(s);
        assert_eq!(spec["autoColor"], false);
        assert_eq!(spec["rgb"], "255;255;255");
    }

    #[test]
    fn group_rows_splits_on_newline_and_keeps_align() {
        let list = segs(
            r#"[{"type":"model","enabled":true,"newline":true},
                {"type":"cwd","enabled":true},
                {"type":"version","enabled":true,"newline":true,"align":"right"}]"#,
        );
        let refs: Vec<&Seg> = list.iter().collect();
        let rows = group_rows(&refs);
        assert_eq!(rows.len(), 2);
        // 首个 segment 的 newline 不产生空行。
        assert_eq!(rows[0]["align"], "left");
        assert_eq!(rows[0]["segs"].as_array().unwrap().len(), 2);
        assert_eq!(rows[1]["align"], "right");
        assert_eq!(rows[1]["segs"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn empty_active_segments_produce_stub_scripts() {
        let list = segs(r##"[{"type":"model","enabled":false}]"##);
        assert_eq!(generate_main(&list), format!("{PEP723_HEADER}print('')\n"));
        assert_eq!(generate_subagent(&list), format!("{PEP723_HEADER}pass\n"));
    }

    #[test]
    fn generated_script_embeds_engine_and_decodable_config() {
        let list: Vec<Seg> = serde_json::from_str(DEFAULT_SEGMENTS).unwrap();
        let script = generate_main(&list);
        assert!(script.starts_with(PEP723_HEADER));
        assert!(script.contains(ENGINE_PY));
        // 默认布局含 group-* → 需要拉 group-info。
        assert!(script.contains("NEED_GROUP = True"));

        let b64 = script
            .split("base64.b64decode(\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let rows: Value = serde_json::from_slice(&B64.decode(b64).unwrap()).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 3);
        assert_eq!(rows[0]["segs"][0]["type"], "model");
        assert_eq!(rows[0]["segs"][0]["rgb"], "74;158;255");
    }

    #[test]
    fn subagent_script_has_no_group_fetch() {
        let list: Vec<Seg> = serde_json::from_str(DEFAULT_SUBAGENT_SEGMENTS).unwrap();
        let script = generate_subagent(&list);
        assert!(script.contains("render_subagent"));
        assert!(!script.contains("NEED_GROUP"));
    }

    #[test]
    fn materialize_covers_three_states() {
        assert!(matches!(materialize(None, true), Materialized::Disabled));
        assert!(matches!(
            materialize(Some(&json!({"enabled": false})), true),
            Materialized::Disabled
        ));
        let m = materialize(
            Some(&json!({"enabled": true, "mode": "custom", "customCommand": "  echo hi  "})),
            true,
        );
        match m {
            Materialized::Custom(c) => assert_eq!(c, "echo hi"),
            _ => panic!("expected custom"),
        }
        // 启用但无 segments → 回落默认布局。
        let m = materialize(Some(&json!({"enabled": true})), false);
        match m {
            Materialized::Builtin(s) => assert!(s.contains("render_subagent")),
            _ => panic!("expected builtin"),
        }
    }

    #[test]
    fn preview_empty_unless_builtin() {
        assert_eq!(preview(&json!({"enabled": false}), true), "");
        assert_eq!(
            preview(
                &json!({"enabled": true, "mode": "custom", "customCommand": "x"}),
                true
            ),
            ""
        );
        assert!(preview(&json!({"enabled": true}), true).starts_with(PEP723_HEADER));
    }

    #[test]
    fn inject_sets_and_removes_fields() {
        let mut config = json!({"statusLine": {"type": "command", "command": "old"}, "keep": 1});
        let fields = [
            (
                "statusLine",
                Some(json!({"type": "command", "command": "new"})),
            ),
            ("subagentStatusLine", None),
        ];
        inject_statusline(&mut config, &fields);
        assert_eq!(config["statusLine"]["command"], "new");
        assert_eq!(config["keep"], 1);

        let mut config = json!({"statusLine": {"x": 1}, "subagentStatusLine": {"x": 1}});
        let fields = [("statusLine", None), ("subagentStatusLine", None)];
        inject_statusline(&mut config, &fields);
        assert!(config.get("statusLine").is_none());
        assert!(config.get("subagentStatusLine").is_none());
    }
}
