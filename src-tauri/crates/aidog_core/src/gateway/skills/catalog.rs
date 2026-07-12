//! catalog 浏览 / 搜索：`npx skills find` + `npx skills add -l` 解析（含 ANSI 剥离状态机）。

use super::proxy_env::apply_proxy_env;
use super::types::CatalogEntry;
use regex::Regex;
use std::process::Command;
use std::sync::OnceLock;

/// catalog 抓取地址（skills.sh 的 JSON 索引，当前 404 不可用；HTTP 抓取路径已下线，
/// 保留常量 + parse_catalog_json 待端点恢复时复用）。
#[allow(dead_code)]
const CATALOG_URL: &str = "https://skills.sh/api/skills";

/// 浏览 catalog。
///
/// skills.sh `/api/skills` 端点当前 404（HTTP 抓取不可用）；`npx skills find` 无关键词时
/// 只回显帮助文本不返回结果。故 browse 恒返回空 —— 前端「搜索安装」页为搜索驱动，不提供 browse。
pub async fn browse_catalog(_proxy_url: Option<&str>) -> Vec<CatalogEntry> {
    // skills.sh /api/skills 当前 404；npx find 无关键词无结果 → browse 不可用，恒空。
    Vec::new()
}

/// 共享 ANSI 转义序列剥离正则（两解析函数共用）。
static ANSI_RE: OnceLock<Regex> = OnceLock::new();
fn ansi_re() -> &'static Regex {
    ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]").unwrap())
}

/// 搜索 catalog：双模式。
///
/// - **精确 source 形态** (`^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$`)：走 `npx skills add <source> -l -y`
///   解析仓库**真实可装集** (git clone + scan SKILL.md, 11/11 全量)。skills.sh 索引
///   (find 走) 仅含被安装过的 skill (6/11), 新仓库 / 新 skill 缺位 → 这条路解决之。
/// - **其他形态** (普通搜索词 / 含 `@` 后缀 / URL): 走 `npx skills find <kw>` 命中 skills.sh
///   索引, 仍是关键词搜索唯一可用源。
///
/// 两路径统一返回 `Vec<CatalogEntry>`。
pub async fn search(kw: &str, proxy_url: Option<&str>) -> Vec<CatalogEntry> {
    let kw_trim = kw.trim();
    if is_exact_source(kw_trim) {
        npx_list_source(kw_trim, proxy_url)
    } else {
        npx_find(kw_trim, proxy_url)
    }
}

/// 判断关键词是否为精确 `owner/repo` 形态 (无 `@skill` 后缀, 无 URL 前缀)。
/// 精确形态走 `npx skills add -l` 拿仓库全量可装集; 其他走 find。
fn is_exact_source(s: &str) -> bool {
    static SOURCE_RE: OnceLock<Regex> = OnceLock::new();
    let re = SOURCE_RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$").unwrap());
    re.is_match(s)
}

/// `npx skills add <source> -l -y`：列仓库内全部可装 skill (无视 skills.sh 索引)。
///
/// `-l` = list available skills without installing; `-y` = 自动接受 agent / scope prompt
/// (Agent detected 时非交互打印结果)。关闭 stdin 避免挂起。
///
/// 输出含 spinner (◒/◐/◓/◑/●/◇/└) + 框形字符 (│) + ANSI 颜色, 必须先剥离。
/// 每条 skill 占多行 (ANSI 剥离 + 行首 trim 后):
/// ```text
/// │    <skill-name>
/// │
/// │      <description>  ← 可能跨行
/// ```
fn npx_list_source(source: &str, proxy_url: Option<&str>) -> Vec<CatalogEntry> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut cmd = Command::new("npx");
    cmd.args(["--yes", "skills", "add", source, "-l", "-y"]);
    cmd.stdin(std::process::Stdio::null());
    apply_proxy_env(&mut cmd, proxy_url);
    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let raw = String::from_utf8_lossy(&output.stdout);
    parse_add_list_output(&raw, source)
}

/// 解析 `npx skills add <source> -l` 输出为 CatalogEntry。
///
/// 状态机:
/// 1. 剥 ANSI / spinner 残留, 按行扫描;
/// 2. 标记位 `in_skills_section` = 见到 "Available Skills" 后置 true;
/// 3. 在 section 内, 行去前导 `│` + 空白 后:
///    - 空行 → 跳;
///    - `<skill-name>` 形态 (仅 `[A-Za-z0-9._-]+`, 无空格) → 提交上一条 + 开新 entry;
///    - 其他文本 → append 到当前 entry 的 description (空格连接, 防跨行 desc 丢)。
/// 4. 见到 `└` 起首 (footer) → 提交并退出。
///
/// 容错: 空输入 / 无 "Available Skills" / 残缺均不崩, 返回已收集到的。
fn parse_add_list_output(raw: &str, source: &str) -> Vec<CatalogEntry> {
    let clean: String = ansi_re().replace_all(raw, "").to_string();

    static NAME_RE: OnceLock<Regex> = OnceLock::new();
    let name_re = NAME_RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9._-]+$").unwrap());

    let repo_url = format!("https://github.com/{source}");
    let mut out: Vec<CatalogEntry> = Vec::new();
    let mut current: Option<(String, String)> = None; // (name, desc)
    let mut in_section = false;

    let flush =
        |cur: &mut Option<(String, String)>, source: &str, repo_url: &str, out: &mut Vec<CatalogEntry>| {
            if let Some((name, desc)) = cur.take() {
                let desc = desc.trim().to_string();
                out.push(CatalogEntry {
                    id: format!("{source}@{name}"),
                    name,
                    description: if desc.is_empty() { None } else { Some(desc) },
                    repo_url: Some(repo_url.to_string()),
                });
            }
        };

    for line in clean.lines() {
        // 去前导框形字符 / spinner / 状态符号 + 空白。
        // 保留 `└` 不剥 (用作 footer 检测)。
        let stripped = line.trim_start_matches(|c: char| {
            matches!(c, '│' | '◇' | '●' | '◒' | '◐' | '◓' | '◑' | '⊙' | '◌')
                || c.is_whitespace()
        });
        if !in_section {
            if stripped.starts_with("Available Skills") {
                in_section = true;
            }
            continue;
        }
        // section 内: footer 行 (└ 起首) → 收尾退出。
        if line.trim_start().starts_with('└') {
            flush(&mut current, source, &repo_url, &mut out);
            break;
        }
        if stripped.is_empty() {
            continue;
        }
        // 新 skill name 行: 单 token, 仅 [A-Za-z0-9._-], 无空格。
        if name_re.is_match(stripped) {
            flush(&mut current, source, &repo_url, &mut out);
            current = Some((stripped.to_string(), String::new()));
            continue;
        }
        // 其他行 → 追加到 description (空格连接, 防跨行)。
        if let Some((_, desc)) = current.as_mut() {
            if !desc.is_empty() {
                desc.push(' ');
            }
            desc.push_str(stripped);
        }
    }
    // 末尾未见 footer 时收尾。
    flush(&mut current, source, &repo_url, &mut out);
    out
}

/// 解析 skills.sh 返回的 JSON 到 CatalogEntry 列表（端点恢复时复用；当前仅测试调用）。
///
/// 容错：接受 `{ "skills": [...] }` 或裸数组；每项尽量从常见字段名提取。
#[allow(dead_code)]
fn parse_catalog_json(raw: &serde_json::Value) -> Vec<CatalogEntry> {
    let arr = raw
        .get("skills")
        .and_then(|v| v.as_array())
        .or_else(|| raw.as_array());
    let Some(items) = arr else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item
                .get("id")
                .or_else(|| item.get("slug"))
                .or_else(|| item.get("repo"))
                .or_else(|| item.get("fullName"))
                .and_then(|v| v.as_str())?
                .to_string();
            if id.is_empty() {
                return None;
            }
            let name = item
                .get("name")
                .or_else(|| item.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or(&id)
                .to_string();
            let description = item
                .get("description")
                .or_else(|| item.get("summary"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let repo_url = item
                .get("repoUrl")
                .or_else(|| item.get("url"))
                .or_else(|| item.get("html_url"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(CatalogEntry {
                id,
                name,
                description,
                repo_url,
            })
        })
        .collect()
}

/// `npx skills find <kw>`：解析输出为 CatalogEntry 列表。
///
/// find 为交互式命令，关闭 stdin + 带关键词时非交互打印结果。输出含 ANSI 颜色码与 spinner
/// 残留，须先剥离。每条结果两行：`owner/repo@skill  <N> installs` + `└ https://skills.sh/...`。
/// 空关键词时 find 只回显帮助（无结果）→ 返回空。
fn npx_find(kw: &str, proxy_url: Option<&str>) -> Vec<CatalogEntry> {
    let kw = kw.trim();
    if kw.is_empty() {
        return Vec::new();
    }
    let mut cmd = Command::new("npx");
    cmd.args(["--yes", "skills", "find", kw]);
    cmd.stdin(std::process::Stdio::null());
    apply_proxy_env(&mut cmd, proxy_url);
    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let raw = String::from_utf8_lossy(&output.stdout);
    parse_find_output(&raw)
}

/// 解析 `npx skills find` 的 stdout（已含 ANSI / spinner 残留）为 CatalogEntry。
fn parse_find_output(raw: &str) -> Vec<CatalogEntry> {
    // 剥离 ANSI 转义序列（颜色 / 光标控制）。
    let clean: String = ansi_re().replace_all(raw, "").to_string();

    // id 行：`owner/repo@skill   <count> installs`（owner/repo 与 @skill 间无空格）。
    static ID_RE: OnceLock<Regex> = OnceLock::new();
    let id_re = ID_RE.get_or_init(|| {
        Regex::new(r"([A-Za-z0-9._-]+/[A-Za-z0-9._-]+@[A-Za-z0-9._-]+)\s+(.+)").unwrap()
    });
    // URL 行：`└ https://skills.sh/owner/repo/skill`（前缀可能是 └ / └─ / 空格）。
    static URL_RE: OnceLock<Regex> = OnceLock::new();
    let url_re = URL_RE.get_or_init(|| Regex::new(r"https://skills\.sh/\S+").unwrap());

    let mut out = Vec::new();
    let mut pending: Option<(String, String)> = None; // (id, installs)
    let flush = |pending: &mut Option<(String, String)>, url: Option<&str>, out: &mut Vec<CatalogEntry>| {
        if let Some((id, installs)) = pending.take() {
            let name = id.split('@').next_back().unwrap_or(&id).to_string();
            out.push(CatalogEntry {
                id,
                name,
                description: Some(installs),
                repo_url: url.map(|s| s.to_string()),
            });
        }
    };
    for line in clean.lines() {
        let line = line.trim();
        if let Some(caps) = id_re.captures(line) {
            // 新 id 行：先提交上一条（无 URL）。
            flush(&mut pending, None, &mut out);
            let id = caps[1].to_string();
            let installs = caps[2].trim().to_string();
            pending = Some((id, installs));
        } else if pending.is_some() {
            if let Some(m) = url_re.find(line) {
                flush(&mut pending, Some(m.as_str()), &mut out);
            }
        }
    }
    // 收尾：最后一条若无 URL 行也提交。
    flush(&mut pending, None, &mut out);
    out
}

#[cfg(test)]
#[path = "test_catalog.rs"]
mod test_catalog;
