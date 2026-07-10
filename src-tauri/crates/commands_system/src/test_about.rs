#![cfg(test)]
use super::*;

#[test]
fn about_info_populated() {
    let info = about_info();
    let json = serde_json::to_value(&info).unwrap();
    assert!(!json["app_version"].as_str().unwrap().is_empty());
    assert!(!json["tauri_version"].as_str().unwrap().is_empty());
    assert!(!json["os"].as_str().unwrap().is_empty());
    assert!(!json["arch"].as_str().unwrap().is_empty());
}
