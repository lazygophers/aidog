use super::*;

#[test]
fn toml_kv_parser() {
    assert_eq!(parse_toml_kv("model = \"gpt-5\""), Some(("model".into(), "gpt-5".into())));
    assert_eq!(parse_toml_kv("wire_api = 'responses'"), Some(("wire_api".into(), "responses".into())));
    assert_eq!(parse_toml_kv("requires_openai_auth = true"), Some(("requires_openai_auth".into(), "true".into())));
    // inline comment。
    assert_eq!(
        parse_toml_kv("base_url = \"https://x.com\" # primary"),
        Some(("base_url".into(), "https://x.com".into()))
    );
}
