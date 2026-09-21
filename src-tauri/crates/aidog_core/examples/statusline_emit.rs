//! Golden-harness emitter: render a statusline script to stdout.
//!
//! Usage: `statusline_emit <main|subagent>` with the segment array (or `null`
//! for the built-in default layout) on stdin. Used by
//! `scripts/statusline-golden/build.mjs` — the only consumer.

use std::io::Read;

fn main() {
    let kind = std::env::args()
        .nth(1)
        .expect("usage: statusline_emit <main|subagent>");
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .expect("read stdin");
    let segments: serde_json::Value = serde_json::from_str(&raw).expect("parse segments json");

    let mut stored = serde_json::json!({ "enabled": true, "mode": "builtin" });
    if !segments.is_null() {
        stored["segments"] = segments;
    }
    print!(
        "{}",
        aidog_core::statusline::preview(&stored, kind != "subagent")
    );
}
