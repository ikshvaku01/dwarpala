use crate::{config::AppConfig, ignore::engine::should_ignore};
use chrono::Local;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn resolve_output_path(root: &Path, config: &AppConfig) -> PathBuf {
    let mut fname = if config.output.filename.is_empty() {
        "compiled_output.txt".to_string()
    } else {
        config.output.filename.clone()
    };
    if config.output.timestamp_suffix {
        let stem = PathBuf::from(&fname).file_stem()
            .unwrap_or_default().to_string_lossy().into_owned();
        fname = format!("{}_{}.txt", stem, Local::now().format("%Y%m%d_%H%M%S"));
    }
    match config.output.path.as_str() {
        "custom" if !config.output.custom_path.is_empty() =>
            PathBuf::from(&config.output.custom_path).join(&fname),
        _ => root.join(&fname),
    }
}

pub fn scan_files(root: &Path, patterns: &[String]) -> (Vec<PathBuf>, Vec<String>) {
    let mut paths: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path())
                .to_string_lossy().replace('\\', "/");
            !should_ignore(&rel, patterns)
        })
        .map(|e| e.path().to_path_buf())
        .collect();
    paths.sort();
    let rel = paths.iter()
        .map(|p| p.strip_prefix(root).unwrap_or(p)
            .to_string_lossy().replace('\\', "/"))
        .collect();
    (paths, rel)
}