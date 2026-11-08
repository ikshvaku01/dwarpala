use crate::{
    config::{ensure_base_ignore, AppConfig},
    ignore::engine::should_ignore,
    output::OutputStyle,
};
use anyhow::Result;
use chrono::Local;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// CLI flag > config file > built-in default (`Banner`, via `OutputConfig::default`).
pub fn resolve_output_style(cli: Option<OutputStyle>, config: &AppConfig) -> OutputStyle {
    cli.unwrap_or(config.output.style)
}

/// Output file name without the fixed `.txt` extension. Empty falls back to the default.
pub fn output_stem(filename: &str) -> String {
    let t = filename.trim();
    let stem = match t.len().checked_sub(4).filter(|&i| t.is_char_boundary(i)) {
        Some(i) if t[i..].eq_ignore_ascii_case(".txt") => &t[..i],
        _ => t,
    };
    if stem.is_empty() { "compiled_output".to_string() } else { stem.to_string() }
}

/// The output file is always `.txt`, whatever `config.output.filename` says.
pub fn resolve_output_path(root: &Path, config: &AppConfig) -> PathBuf {
    let stem = output_stem(&config.output.filename);
    let fname = if config.output.timestamp_suffix {
        format!("{stem}_{}.txt", Local::now().format("%Y%m%d_%H%M%S"))
    } else {
        format!("{stem}.txt")
    };
    match config.output.path.as_str() {
        "custom" if !config.output.custom_path.is_empty() =>
            PathBuf::from(&config.output.custom_path).join(&fname),
        _ => root.join(&fname),
    }
}
/// The `baseIgnore` pattern that keeps the compiled output out of its own input:
/// `<name>.txt`, or `<name>_*.txt` when a timestamp is appended.
pub fn output_ignore_pattern(config: &AppConfig) -> String {
    let stem = output_stem(&config.output.filename);
    if config.output.timestamp_suffix { format!("{stem}_*.txt") } else { format!("{stem}.txt") }
}

/// Every `baseIgnore` pattern the current output name can produce: `<name>.txt` and
/// `<name>_*.txt`. A reset uses this to clean up what the old name left behind.
pub fn output_ignore_patterns(config: &AppConfig) -> Vec<String> {
    let stem = output_stem(&config.output.filename);
    vec![format!("{stem}.txt"), format!("{stem}_*.txt")]
}

/// Make sure `baseIgnore` covers the output file, in `config` and in config.json.
/// Returns the pattern and whether it was newly added. Never removes anything.
pub fn ensure_output_ignored(config: &mut AppConfig) -> (String, Result<bool>) {
    let pattern = output_ignore_pattern(config);
    let added = ensure_base_ignore(config, &pattern);
    (pattern, added)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_drops_only_a_trailing_txt() {
        assert_eq!(output_stem("compiled_output.txt"), "compiled_output");
        assert_eq!(output_stem("Report.TXT"), "Report");
        assert_eq!(output_stem("  a.txt "), "a");
        assert_eq!(output_stem("notes"), "notes");
        assert_eq!(output_stem("out.md"), "out.md");
        assert_eq!(output_stem(""), "compiled_output");
        assert_eq!(output_stem(".txt"), "compiled_output");
    }

    #[test]
    fn extension_is_always_txt() {
        let mut c = AppConfig::default();
        c.output.filename = "out.md".into();
        assert_eq!(resolve_output_path(Path::new("/r"), &c), Path::new("/r").join("out.md.txt"));
        c.output.filename = String::new();
        assert_eq!(resolve_output_path(Path::new("/r"), &c), Path::new("/r").join("compiled_output.txt"));
        c.output.filename = "a.txt".into();
        c.output.timestamp_suffix = true;
        let name = resolve_output_path(Path::new("/r"), &c)
            .file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("a_") && name.ends_with(".txt"), "{name}");
    }

    #[test]
    fn output_ignore_pattern_tracks_name_and_timestamp() {
        let mut c = AppConfig::default();
        assert_eq!(output_ignore_pattern(&c), "compiled_output.txt");
        c.output.filename = "mini-compiled.txt".into();
        assert_eq!(output_ignore_pattern(&c), "mini-compiled.txt");
        c.output.timestamp_suffix = true;
        assert_eq!(output_ignore_pattern(&c), "mini-compiled_*.txt");
    }

    #[test]
    fn the_pattern_matches_the_file_that_gets_written() {
        use crate::ignore::engine::matches_ignore;
        let mut c = AppConfig::default();
        for (name, ts) in [("compiled_output", false), ("mini-compiled", true), ("Report.txt", true), ("x", false)] {
            c.output.filename = name.into();
            c.output.timestamp_suffix = ts;
            let pat = output_ignore_pattern(&c);
            let file = resolve_output_path(Path::new("/r"), &c)
                .file_name().unwrap().to_string_lossy().into_owned();
            assert!(matches_ignore(&file, &pat), "{pat} should ignore {file}");
        }
    }

    #[test]
    fn both_pattern_forms_are_listed() {
        let mut c = AppConfig::default();
        c.output.filename = "mini-compiled.txt".into();
        assert_eq!(output_ignore_patterns(&c), ["mini-compiled.txt", "mini-compiled_*.txt"]);
    }
}