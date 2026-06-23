#![cfg(test)]
use super::*;

#[test]
fn expand_path_tilde_and_plain() {
    let home = dirs::home_dir().unwrap();
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
