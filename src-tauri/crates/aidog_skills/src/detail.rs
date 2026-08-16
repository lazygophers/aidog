//! skill 详情只读浏览：文件树列举 + 单文件读取（含路径遍历防护）。

use super::types::{SkillDetail, SkillFile, SkillFileContent, BINARY_SNIFF_BYTES, MAX_READ_BYTES};
use std::fs;
use std::path::PathBuf;

/// 列 skill 目录文件树（递归，相对路径），供详情视图浏览。
///
/// 安全: `installed_path` canonicalize 后须存在且为目录。文件遍历仅限该目录子树。
/// 跳过 `.git/` 目录（避免列版本控制元数据）；其他 dotfile（`.env.example`）保留。
pub fn detail(installed_path: &str) -> Result<SkillDetail, String> {
    let root = PathBuf::from(installed_path.trim());
    let canon = root
        .canonicalize()
        .map_err(|e| format!("skill path not found: {e}"))?;
    if !canon.is_dir() {
        return Err(format!("skill path is not a directory: {}", canon.display()));
    }
    let skill_name = canon
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut files: Vec<SkillFile> = Vec::new();
    collect_files(&canon, &canon, &mut files)?;

    // SKILL.md 置首，其余按 rel_path 字母序。
    files.sort_by(|a, b| {
        let a_skill = a.rel_path == "SKILL.md";
        let b_skill = b.rel_path == "SKILL.md";
        b_skill
            .cmp(&a_skill)
            .then_with(|| a.rel_path.cmp(&b.rel_path))
    });

    Ok(SkillDetail {
        skill_name,
        root: canon.to_string_lossy().to_string(),
        files,
    })
}

/// 递归收集文件（相对 base 的路径）。跳过 `.git/` 子目录。
fn collect_files(base: &PathBuf, dir: &PathBuf, out: &mut Vec<SkillFile>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("read_dir failed: {e}"))?;
    for ent in entries.flatten() {
        let name = ent.file_name();
        let name_str = name.to_string_lossy();
        let path = ent.path();
        if path.is_dir() {
            // 跳过 .git 版本控制目录。
            if name_str == ".git" {
                continue;
            }
            collect_files(base, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .map_err(|e| format!("strip_prefix failed: {e}"))?
                .to_string_lossy()
                .replace('\\', "/");
            let meta = ent.metadata().map_err(|e| format!("metadata failed: {e}"))?;
            let size = meta.len();
            let is_text = sniff_text(&path).unwrap_or(false);
            out.push(SkillFile {
                rel_path: rel,
                size,
                is_text,
            });
        }
    }
    Ok(())
}

/// 启发式: 读首 BINARY_SNIFF_BYTES 字节，无 NUL → text。
fn sniff_text(path: &PathBuf) -> Result<bool, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("open failed: {e}"))?;
    use std::io::Read;
    let mut buf = vec![0u8; BINARY_SNIFF_BYTES];
    let n = f.read(&mut buf).map_err(|e| format!("read failed: {e}"))?;
    Ok(!buf[..n].contains(&0u8))
}

/// 读 skill 内单文件（只读浏览）。
///
/// 安全（路径遍历防护，见 [[pathbuf-starts-with-traversal]]）:
/// 1. `installed_path` canonicalize 得 skill 根
/// 2. `rel` 标准化校验: 拒含 `..` 段 / 以 `/` 或 Windows 盘符开头
/// 3. 拼接后 canonicalize，断言 `starts_with(skill_root)`
/// 4. 必须是文件（非目录/符号链接逃逸）
pub fn read_file(installed_path: &str, rel: &str) -> Result<SkillFileContent, String> {
    let rel = rel.trim().replace('\\', "/");
    if rel.is_empty() {
        return Err("empty file path".to_string());
    }
    // 拒绝对路径与 `..` 遍历。
    if rel.starts_with('/') || rel.len() >= 2 && rel.as_bytes()[1] == b':' {
        return Err("absolute path not allowed".to_string());
    }
    for seg in rel.split('/') {
        if seg == ".." {
            return Err("path traversal not allowed".to_string());
        }
    }

    let root = PathBuf::from(installed_path.trim())
        .canonicalize()
        .map_err(|e| format!("skill path not found: {e}"))?;
    if !root.is_dir() {
        return Err("skill path is not a directory".to_string());
    }

    let target = root.join(&rel);
    let canon = target
        .canonicalize()
        .map_err(|e| format!("file not found: {e}"))?;
    if !canon.starts_with(&root) {
        return Err("path escapes skill directory".to_string());
    }
    if !canon.is_file() {
        return Err("not a file".to_string());
    }

    let meta = fs::metadata(&canon).map_err(|e| format!("metadata failed: {e}"))?;
    let size = meta.len();

    // 二进制检测：非文本 → 不返回内容。
    if !sniff_text(&canon).unwrap_or(false) {
        return Ok(SkillFileContent {
            content: None,
            truncated: false,
            size,
        });
    }

    let bytes = fs::read(&canon).map_err(|e| format!("read failed: {e}"))?;
    let (content, truncated) = if bytes.len() > MAX_READ_BYTES {
        (
            String::from_utf8_lossy(&bytes[..MAX_READ_BYTES]).to_string(),
            true,
        )
    } else {
        (String::from_utf8_lossy(&bytes).to_string(), false)
    };
    Ok(SkillFileContent {
        content: Some(content),
        truncated,
        size,
    })
}

#[cfg(test)]
#[path = "test_detail.rs"]
mod test_detail;
