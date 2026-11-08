use clap::ValueEnum;
use serde::{Deserialize, Serialize};

const RULE_WIDTH: usize = 50;

/// Horizontal rule shared by the banner style and the directory-structure block.
pub fn rule() -> String {
    "-".repeat(RULE_WIDTH)
}

/// How each file is introduced in the compiled document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OutputStyle {
    /// Ruled banner with `FILE: <path>` (original behaviour).
    #[default]
    Banner,
    /// One-line `// <path>` comment.
    Comment,
}

impl OutputStyle {
    /// Every preset, in menu order. Add new variants here.
    pub const ALL: [OutputStyle; 2] = [Self::Banner, Self::Comment];

    pub fn label(self) -> &'static str {
        match self {
            Self::Banner  => "banner",
            Self::Comment => "comment",
        }
    }

    /// Header for the directory-structure block, without the trailing blank
    /// line (the worker adds that).
    pub fn dir_header(self) -> String {
        match self {
            Self::Banner => {
                let sep = rule();
                format!("{sep}\nDirectory Structure\n{sep}")
            }
            Self::Comment => "// Directory Structure".to_string(),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Banner  => "Ruled banner per file (default)",
            Self::Comment => "One-line // path header, saves tokens",
        }
    }

    /// Header text for one file, without the trailing blank line
    /// (the worker adds that).
    pub fn file_header(self, rel: &str) -> String {
        match self {
            Self::Banner => {
                let sep = rule();
                format!("{sep}\nFILE: {rel}\n{sep}")
            }
            Self::Comment => format!("// {rel}"),
        }
    }

    pub fn next(self) -> Self { self.step(1) }
    pub fn prev(self) -> Self { self.step(-1) }

    fn step(self, d: isize) -> Self {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap_or(0) as isize;
        Self::ALL[(i + d).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_matches_legacy_layout() {
        // Old code: writeln!(w, "{sep}\nFILE: {rel}\n{sep}\n")
        let legacy = format!("{sep}\nFILE: Cargo.toml\n{sep}\n", sep = "-".repeat(50));
        // New code: writeln!(w, "{}\n", header) == header + "\n\n"
        assert_eq!(format!("{}\n", OutputStyle::Banner.file_header("Cargo.toml")), legacy);
    }

    #[test]
    fn comment_header_is_single_line() {
        assert_eq!(OutputStyle::Comment.file_header("src/main.rs"), "// src/main.rs");
    }

    #[test]
    fn cycling_wraps_both_ways() {
        assert_eq!(OutputStyle::Banner.next(),  OutputStyle::Comment);
        assert_eq!(OutputStyle::Comment.next(), OutputStyle::Banner);
        assert_eq!(OutputStyle::Banner.prev(),  OutputStyle::Comment);
    }

    #[test]
    fn serde_uses_lowercase_names() {
        assert_eq!(serde_json::to_string(&OutputStyle::Comment).unwrap(), "\"comment\"");
        assert_eq!(serde_json::from_str::<OutputStyle>("\"banner\"").unwrap(), OutputStyle::Banner);
    }

    #[test]
    fn banner_dir_header_matches_legacy_layout() {
        // Old code: writeln!(w, "{sep}\nDirectory Structure\n{sep}\n\n{tree}\n")
        let sep  = "-".repeat(50);
        let tree = "TREE";
        let legacy = format!("{sep}\nDirectory Structure\n{sep}\n\n{tree}\n");
        assert_eq!(format!("{}\n\n{tree}\n", OutputStyle::Banner.dir_header()), legacy);
    }

    #[test]
    fn comment_dir_header_is_single_line() {
        assert_eq!(OutputStyle::Comment.dir_header(), "// Directory Structure");
    }
}