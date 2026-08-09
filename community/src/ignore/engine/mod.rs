
use std::{fs, path::Path};

pub fn load_gitignore(root: &Path) -> Vec<String> {
    fs::read_to_string(root.join(".gitignore"))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .map(String::from)
        .collect()
}

pub fn matches_ignore(rel_path: &str, pattern: &str) -> bool {
    let p = pattern.trim().replace('\\', "/");
    if p.is_empty() { return false; }
    let is_dir_only = p.ends_with('/');
    let p = p.trim_end_matches('/');
    let path_parts: Vec<&str> = rel_path.split('/').collect();
    let pat_parts:  Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    if pat_parts.is_empty() { return false; }
    if pat_parts.len() == 1 {
        let comp = pat_parts[0];
        if comp.contains('*') {
            let re_str = format!("^{}$", regex::escape(comp).replace("\\*", ".*"));
            if let Ok(re) = regex::Regex::new(&re_str) {
                return path_parts.iter().enumerate().any(|(i, part)| {
                    re.is_match(part) && !(is_dir_only && i == path_parts.len() - 1)
                });
            }
        } else {
            return path_parts.iter().enumerate().any(|(i, part)| {
                *part == comp && !(is_dir_only && i == path_parts.len() - 1)
            });
        }
    } else if p.contains('*') {
        let re_str = format!("^{}(/|$)", regex::escape(p).replace("\\*", ".*"));
        if let Ok(re) = regex::Regex::new(&re_str) { return re.is_match(rel_path); }
    } else {
        return rel_path == p || rel_path.starts_with(&format!("{p}/"));
    }
    false
}

pub fn should_ignore(rel: &str, patterns: &[String]) -> bool {
    if rel.starts_with(".git/") || rel == ".git" { return true; }
    patterns.iter().any(|p| matches_ignore(rel, p))
}