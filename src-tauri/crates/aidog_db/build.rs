//! 编译期枚举 `defaults/registry/` 下全部文件，生成 `include_str!` 清单供 `registry.rs` 引用。
//! registry 产物人工维护（新增平台/模型 = 新增一个 JSON 文件），清单必须自动发现，
//! 否则每加一个模型都要手改 Rust 常量。

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// 递归收集 `dir` 下全部 `.json`，并对每个目录/文件登记 rerun-if-changed
/// （目录 mtime 覆盖增删文件，文件 mtime 覆盖内容改动）。
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            println!("cargo:rerun-if-changed={}", path.display());
            out.push(path);
        }
    }
}

fn main() {
    let registry = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../defaults/registry")
        .canonicalize()
        .expect("registry dir");

    let index = registry.join("index.json");
    println!("cargo:rerun-if-changed={}", index.display());
    let mut src = format!("pub static INDEX_JSON: &str = include_str!({index:?});\n");

    let mut platform_files = String::new();
    let mut model_files = String::new();

    let platforms = registry.join("platforms");
    println!("cargo:rerun-if-changed={}", platforms.display());
    let mut codes: Vec<_> = std::fs::read_dir(&platforms)
        .expect("platforms dir")
        .map(|e| e.expect("dir entry").path())
        .collect();
    codes.sort();

    for dir in codes {
        let code = dir.file_name().expect("code").to_string_lossy().into_owned();

        let platform_json = dir.join("platform.json");
        if platform_json.exists() {
            println!("cargo:rerun-if-changed={}", platform_json.display());
            let _ = writeln!(platform_files, "    ({code:?}, include_str!({platform_json:?})),");
        }

        let models_dir = dir.join("models");
        if models_dir.is_dir() {
            let mut files = Vec::new();
            collect(&models_dir, &mut files);
            for f in files {
                // 相对 models_dir 的路径（vendor 前缀模型 id 会落进子目录，如 `mistralai/codestral-2.json`，
                // 光取文件名会跨子目录撞名）。index.json 的 `models` 数组就是这份清单，
                // 远程同步照它逐文件拉，一致性靠 `test_registry.rs` 的漂移断言守住。
                // 分隔符归一成 `/`：Windows 上 `to_string_lossy` 会留 `\`，既对不上
                // index.json 清单（漂移断言炸），远程同步也会拼出 404 的 URL。
                let rel = f
                    .strip_prefix(&models_dir)
                    .expect("model file under models dir")
                    .to_string_lossy()
                    .replace('\\', "/");
                let _ = writeln!(model_files, "    ({code:?}, {rel:?}, include_str!({f:?})),");
            }
        }
    }

    let _ = write!(
        src,
        "pub static PLATFORM_FILES: &[(&str, &str)] = &[\n{platform_files}];\n\
         pub static MODEL_FILES: &[(&str, &str, &str)] = &[\n{model_files}];\n"
    );

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("registry_includes.rs");
    std::fs::write(&out, src).expect("write registry_includes.rs");
}
