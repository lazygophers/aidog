//! CPA 配置导入的压缩包解压支持。
//!
//! 与 `parser.rs` 的 YAML/JSON 解析正交：仅负责把 zip/tar/tar.gz/tgz
//! 解到临时目录供后续扫描。rar/7z 仅识别拒绝不解压。

use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// 解压 ZIP 文件，返回临时目录路径。
pub(super) fn unzip_archive(zip_path: &Path) -> Result<TempDir, String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("打开 ZIP 失败: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("ZIP 解析失败: {e}"))?;

    let temp_dir = TempDir::new()
        .map_err(|e| format!("创建临时目录失败: {e}"))?;

    archive.extract(temp_dir.path())
        .map_err(|e| format!("ZIP 解压失败: {e}"))?;

    Ok(temp_dir)
}

/// 解压 TAR/TAR.GZ/TGZ 文件，返回临时目录路径。
pub(super) fn untar_archive(tar_path: &Path) -> Result<TempDir, String> {
    let file = fs::File::open(tar_path)
        .map_err(|e| format!("打开 TAR 失败: {e}"))?;

    let temp_dir = TempDir::new()
        .map_err(|e| format!("创建临时目录失败: {e}"))?;

    let is_gz = tar_path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s == "gz" || tar_path.file_stem()
            .and_then(|st| st.to_str())
            .map(|st| st.ends_with(".tar"))
            .unwrap_or(false))
        .unwrap_or(false);

    if is_gz {
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(temp_dir.path())
            .map_err(|e| format!("TAR.GZ 解压失败: {e}"))?;
    } else {
        let mut archive = tar::Archive::new(file);
        archive.unpack(temp_dir.path())
            .map_err(|e| format!("TAR 解压失败: {e}"))?;
    }

    Ok(temp_dir)
}

/// 判断路径是否为支持的压缩文件。
pub(super) fn is_supported_archive(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        match ext {
            "zip" => return true,
            "gz" => return true,
            "tgz" => return true,
            "tar" => {
                // .tar 无扩展名，检查文件名
                return path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|stem| !stem.ends_with(".tar"))
                    .unwrap_or(true);
            }
            _ => return false,
        }
    }
    // 检查 .tar 结尾（无扩展名）
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.ends_with(".tar"))
        .unwrap_or(false)
}

/// 判断路径是否为不支持的压缩文件（rar/7z）。
pub(super) fn is_unsupported_archive(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        matches!(ext, "rar" | "7z" | "xz" | "bz" | "bz2")
    } else {
        false
    }
}
