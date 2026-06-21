use super::*;

#[test]
fn read_file_rejects_traversal() {
    let tmp = std::env::temp_dir().join("aidog_skill_read_test");
    let _ = std::fs::create_dir_all(&tmp);
    let _ = std::fs::write(tmp.join("SKILL.md"), "# skill\n");
    let path = tmp.to_string_lossy().to_string();
    // `..` 段 → 拒。
    assert!(read_file(&path, "../etc/passwd").is_err());
    assert!(read_file(&path, "sub/../../etc/passwd").is_err());
    // 绝对路径 → 拒。
    assert!(read_file(&path, "/etc/passwd").is_err());
    // 空 → 拒。
    assert!(read_file(&path, "").is_err());
    // 正常相对文件 → 成功。
    let r = read_file(&path, "SKILL.md").unwrap();
    assert_eq!(r.content.as_deref(), Some("# skill\n"));
    assert!(!r.truncated);
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn detail_lists_files_skill_md_first() {
    let tmp = std::env::temp_dir().join("aidog_skill_detail_test");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(tmp.join("references"));
    std::fs::write(tmp.join("SKILL.md"), "# t\n").unwrap();
    std::fs::write(tmp.join("README.md"), "readme").unwrap();
    std::fs::write(tmp.join("references/x.md"), "x").unwrap();
    let d = detail(&tmp.to_string_lossy()).unwrap();
    assert_eq!(d.files[0].rel_path, "SKILL.md");
    assert!(d.files.iter().any(|f| f.rel_path == "references/x.md"));
    assert!(d.files.iter().any(|f| f.rel_path == "README.md"));
    let _ = std::fs::remove_dir_all(&tmp);
}
