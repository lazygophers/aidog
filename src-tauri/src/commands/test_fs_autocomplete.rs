#![cfg(test)]
use super::*;
use aidog_core::gateway::db::test_support::HomeGuard;

#[test]
fn expand_path_tilde_and_plain() {
    // 包 HomeGuard：expand_path("~") 读 HOME，须指向 tempdir 而非真实 home。
    let g = HomeGuard::new();
    let home = g.home();
    assert_eq!(expand_path("~"), home);
    assert_eq!(expand_path("~/sub"), home.join("sub"));
    assert_eq!(expand_path("/abs/path"), std::path::PathBuf::from("/abs/path"));
}

#[test]
fn autocomplete_lists_dir_entries() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("alpha")).unwrap();
    std::fs::write(dir.path().join("beta.txt"), "x").unwrap();
    std::fs::write(dir.path().join("gamma.txt"), "x").unwrap();

    // list all (trailing slash)
    let all = fs_autocomplete(format!("{}/", dir.path().display())).unwrap();
    assert!(all.len() >= 3);
    // dirs come first
    assert!(all[0].is_dir);

    // prefix filter
    let filtered = fs_autocomplete(format!("{}/be", dir.path().display())).unwrap();
    assert_eq!(filtered.len(), 1);

    // nonexistent → empty
    let none = fs_autocomplete("/nonexistent-aidog-xyz/".into()).unwrap();
    assert!(none.is_empty());
}
