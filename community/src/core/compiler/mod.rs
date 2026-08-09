use crate::ignore::engine::should_ignore;
use std::{fs, path::Path};

/// Public alias used by worker/mod.rs
pub fn build_dir_tree_string(root: &Path, patterns: &[String]) -> String {
    let name = root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let mut lines = vec![format!("{name}/")];
    recurse(root, root, "", patterns, &mut lines);
    lines.join("\n")
}

fn recurse(root: &Path, dir: &Path, prefix: &str, patterns: &[String], out: &mut Vec<String>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();
    entries.sort_by(|a, b| {
        let ad = a.path().is_dir();
        let bd = b.path().is_dir();
        bd.cmp(&ad).then_with(|| a.file_name().cmp(&b.file_name()))
    });
    let entries: Vec<_> = entries.into_iter().filter(|e| {
        let rel = e.path().strip_prefix(root).unwrap_or(&e.path())
            .to_string_lossy().replace('\\', "/");
        !should_ignore(&rel, patterns)
    }).collect();
    for (i, entry) in entries.iter().enumerate() {
        let last         = i == entries.len() - 1;
        let child_prefix = format!("{prefix}{}", if last { "    " } else { "|   " });
        let is_dir       = entry.path().is_dir();
        out.push(format!("{prefix}+-- {}{}", entry.file_name().to_string_lossy(),
                         if is_dir { "/" } else { "" }));
        if is_dir { recurse(root, &entry.path(), &child_prefix, patterns, out); }
    }
}