use super::*;

#[test]
fn codex_write_preserves_existing_values_and_removes_only_unchanged_owned_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "model = \"gpt-5\"\n\n[shell_environment_policy.set]\nUSER_VALUE = \"keep\"\nMANAGED = \"old\"\nUSER_EDITED = \"changed\"\n").unwrap();
    let previous = BTreeMap::from([
        ("MANAGED".to_owned(), "old".to_owned()),
        ("USER_EDITED".to_owned(), "aidog".to_owned()),
    ]);
    let variables = vec![
        Variable {
            key: "MANAGED".into(),
            value: "new".into(),
            claude: false,
            codex: true,
        },
        Variable {
            key: "USER_VALUE".into(),
            value: "overwrite".into(),
            claude: false,
            codex: true,
        },
        Variable {
            key: "ADDED".into(),
            value: "yes\"\nmodel = \"bad".into(),
            claude: false,
            codex: true,
        },
    ];
    let (owned, conflicts) = save_codex(&path, &variables, &previous).unwrap();
    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(conflicts, vec!["USER_VALUE"]);
    assert!(written.contains("USER_VALUE = \"keep\""));
    assert!(written.contains("MANAGED = \"new\""));
    assert!(written.contains("USER_EDITED = \"changed\""));
    assert!(written.contains("ADDED = \"yes\\\"\\nmodel = \\\"bad\""));
    assert_eq!(owned.get("MANAGED").map(String::as_str), Some("new"));
    assert_eq!(
        owned.get("ADDED").map(String::as_str),
        Some("yes\"\nmodel = \"bad")
    );
}

#[test]
fn codex_write_refuses_invalid_toml_and_invalid_variable_names() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "bad = [\n").unwrap();
    assert!(save_codex(&path, &[], &BTreeMap::new()).is_err());
    fs::write(&path, "").unwrap();
    assert!(
        save_codex(
            &path,
            &[Variable {
                key: "BAD-NAME".into(),
                value: "x".into(),
                claude: false,
                codex: true
            }],
            &BTreeMap::new()
        )
        .is_err()
    );
}
