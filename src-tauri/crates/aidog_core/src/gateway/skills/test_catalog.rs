use super::*;

#[test]
fn is_exact_source_matches_owner_repo() {
    assert!(is_exact_source("lazygophers/ccplugin"));
    assert!(is_exact_source("vercel-labs/agent-skills"));
    assert!(is_exact_source("a.b/c_d"));
    // 单 token / 含 @ / 空 / URL / 多斜杠 → 否
    assert!(!is_exact_source("trellis"));
    assert!(!is_exact_source("lazygophers/ccplugin@hooks"));
    assert!(!is_exact_source(""));
    assert!(!is_exact_source("https://github.com/a/b"));
    assert!(!is_exact_source("a/b/c"));
    // 含空格 → 否
    assert!(!is_exact_source("a / b"));
}

#[test]
fn parse_find_output_basic() {
    // 模拟 `npx skills find` 输出（含 ANSI 码 + URL 行配对）。
    let raw = "\x1b[38;5;102mInstall with\x1b[0m npx skills add <owner/repo@skill>\n\n\x1b[38;5;145mxixu-me/skills@github-actions-docs\x1b[0m \x1b[36m217.7K installs\x1b[0m\n\x1b[38;5;102m└ https://skills.sh/xixu-me/skills/github-actions-docs\x1b[0m\n\ngithub/awesome-copilot@git-commit\x1b[0m \x1b[36m35.3K installs\x1b[0m\n└ https://skills.sh/github/awesome-copilot/git-commit\x1b[0m\n";
    let out = parse_find_output(raw);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].id, "xixu-me/skills@github-actions-docs");
    assert_eq!(out[0].name, "github-actions-docs");
    assert_eq!(out[0].description.as_deref(), Some("217.7K installs"));
    assert_eq!(
        out[0].repo_url.as_deref(),
        Some("https://skills.sh/xixu-me/skills/github-actions-docs")
    );
    assert_eq!(out[1].id, "github/awesome-copilot@git-commit");
    assert_eq!(out[1].name, "git-commit");
    assert_eq!(
        out[1].repo_url.as_deref(),
        Some("https://skills.sh/github/awesome-copilot/git-commit")
    );
}

#[test]
fn parse_find_output_empty() {
    assert!(parse_find_output("").is_empty());
    assert!(parse_find_output("Install with npx skills add <owner/repo@skill>\n\n").is_empty());
}

#[test]
fn parse_find_output_no_url_line() {
    // 最后一条缺 URL 行也应提交。
    let raw = "owner/repo@skill-a  10 installs\n└ https://skills.sh/owner/repo/skill-a\nowner/repo@skill-b  5 installs\n";
    let out = parse_find_output(raw);
    assert_eq!(out.len(), 2);
    assert_eq!(out[1].id, "owner/repo@skill-b");
    assert!(out[1].repo_url.is_none());
}

#[test]
fn parse_catalog_wrapped_object() {
    let raw = serde_json::json!({
        "skills": [
            { "id": "vercel-labs/foo", "name": "Foo", "description": "a foo skill" },
            { "slug": "bar/baz", "title": "Baz" }
        ]
    });
    let out = parse_catalog_json(&raw);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].id, "vercel-labs/foo");
    assert_eq!(out[0].name, "Foo");
    assert_eq!(out[1].id, "bar/baz");
    assert_eq!(out[1].name, "Baz");
}

#[test]
fn parse_add_list_output_full_fixture() {
    // 真实 `npx skills add lazygophers/ccplugin -l` 输出片段 (ANSI 已剥, spinner 残留保留)。
    let raw = "\
│
●   claude-code_2-1-177_agent  Agent detected — installing non-interactively
│
◇  Source: https://github.com/lazygophers/ccplugin.git
│
◇  Repository cloned
│
◇  Found 11 skills

│
◇  Available Skills
│
│    architecture-design
│
│      用「正交分解」方法论做架构设计与评审。把系统正交分解。
│
│    perf-optimization
│
│      性能优化的跨栈方法论框架。
│
│    trellis-before-dev
│
│      Discovers and injects project-specific coding guidelines.
│
│
└  Use --skill <name> to install specific skills
";
    let out = parse_add_list_output(raw, "lazygophers/ccplugin");
    assert_eq!(out.len(), 3, "expected 3 skills, got: {:?}", out);
    assert_eq!(out[0].name, "architecture-design");
    assert_eq!(out[0].id, "lazygophers/ccplugin@architecture-design");
    assert_eq!(
        out[0].repo_url.as_deref(),
        Some("https://github.com/lazygophers/ccplugin"),
    );
    assert!(out[0].description.as_deref().unwrap().contains("正交分解"));
    assert_eq!(out[1].name, "perf-optimization");
    assert_eq!(out[2].name, "trellis-before-dev");
}

#[test]
fn parse_add_list_output_empty_and_no_section() {
    // 空输入 → 空
    assert!(parse_add_list_output("", "a/b").is_empty());
    // 无 "Available Skills" header → 空 (in_section 未触)
    assert!(parse_add_list_output("│  some preamble\n│  but no header\n", "a/b").is_empty());
    // 只有 header 无内容 → 空
    let raw = "│\n◇  Available Skills\n│\n└  end\n";
    assert!(parse_add_list_output(raw, "a/b").is_empty());
}

#[test]
fn parse_add_list_output_multiline_description() {
    // description 跨多行 → 应合并 (空格连接)
    let raw = "\
◇  Available Skills
│
│    skill-a
│
│      first line of description
│      second line continues
│
│    skill-b
│
│      single line
│
└  end
";
    let out = parse_add_list_output(raw, "x/y");
    assert_eq!(out.len(), 2);
    let desc_a = out[0].description.as_deref().unwrap();
    assert!(desc_a.contains("first line"));
    assert!(desc_a.contains("second line"), "got: {}", desc_a);
    assert_eq!(out[1].description.as_deref(), Some("single line"));
}

#[test]
fn parse_catalog_bare_array() {
    let raw = serde_json::json!([
        { "repo": "a/b" }
    ]);
    let out = parse_catalog_json(&raw);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, "a/b");
    // name 回退到 id。
    assert_eq!(out[0].name, "a/b");
}
