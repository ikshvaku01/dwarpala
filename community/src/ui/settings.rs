//! Settings shown in the menu's Settings tab.
//!
//! To add a setting:
//!   1. add a variant and list it in `ALL` (that is the on-screen order);
//!   2. fill in `group`, `label`, `description`, `kind`, `show`, `path`, `json` and `copy`;
//!   3. handle it in `cycle` (Cycle) or `raw_text` + `store_text` (Text);
//!   4. optionally give it a `preview`.
//! Reset, key handling and drawing are generic over `ALL`; none of them needs to change.

use crate::{
    config::{default_config, release_base_ignore, save_setting, AppConfig},
    ui::app::App,
    utils::{ensure_output_ignored, output_ignore_pattern, output_ignore_patterns, output_stem},
};
use anyhow::{bail, Result};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    OutputStyle,
    DirStructure,
    Filename,
    TimestampSuffix,
    OutputLocation,
    CustomPath,
    GitIgnore,
}

/// How a setting is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// ←/→, Space or Enter flips or cycles the value.
    Cycle,
    /// Enter opens an inline text field.
    Text,
}

impl Setting {
    pub const ALL: [Setting; 7] = [
        Self::OutputStyle,
        Self::DirStructure,
        Self::Filename,
        Self::TimestampSuffix,
        Self::OutputLocation,
        Self::CustomPath,
        Self::GitIgnore,
    ];

    /// Section heading; consecutive settings with the same group share one heading.
    pub fn group(self) -> &'static str {
        match self {
            Self::OutputStyle | Self::DirStructure | Self::Filename
            | Self::TimestampSuffix | Self::OutputLocation | Self::CustomPath => "Output",
            Self::GitIgnore => "Scanning",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OutputStyle     => "Output style",
            Self::DirStructure    => "Directory structure",
            Self::Filename        => "Output filename",
            Self::TimestampSuffix => "Timestamp suffix",
            Self::OutputLocation  => "Output location",
            Self::CustomPath      => "Custom output folder",
            Self::GitIgnore       => "Use .gitignore",
        }
    }

    /// Keep these short: the menu is 70 columns wide.
    pub fn description(self) -> &'static str {
        match self {
            Self::OutputStyle     => "How each file is introduced in the compiled document",
            Self::DirStructure    => "Put a directory tree at the top of the output",
            Self::Filename        => "Name (.txt is fixed); auto-added to baseIgnore  [Enter] edit",
            Self::TimestampSuffix => "Save as <name>_<date>_<time>.txt; ignored via <name>_*.txt",
            Self::OutputLocation  => "default = project root, custom = the folder below",
            Self::CustomPath      => "Folder used when location is custom  [Enter] to edit",
            Self::GitIgnore       => "Honour .gitignore when scanning (applies next launch)",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Self::Filename | Self::CustomPath => Kind::Text,
            _ => Kind::Cycle,
        }
    }

    /// Fixed text drawn after the editor box (the extension is not editable).
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Filename => ".txt",
            _ => "",
        }
    }

    /// Filename and timestamp decide what the output file is called, so they also decide
    /// which `baseIgnore` pattern keeps it out of the next scan.
    fn affects_output_name(self) -> bool {
        matches!(self, Self::Filename | Self::TimestampSuffix)
    }

    // ── Reading ──────────────────────────────────────────────────────────────

    fn show(self, c: &AppConfig) -> String {
        match self {
            Self::OutputStyle     => c.output.style.label().to_string(),
            Self::DirStructure    => on_off(c.features.dir_structure).into(),
            Self::Filename        => format!("{}.txt", output_stem(&c.output.filename)),
            Self::TimestampSuffix => on_off(c.output.timestamp_suffix).into(),
            Self::OutputLocation  => if c.output.path == "custom" && c.output.custom_path.is_empty() {
                "custom (folder not set)".into()
            } else {
                c.output.path.clone()
            },
            Self::CustomPath      => if c.output.custom_path.is_empty() {
                "(not set)".into()
            } else {
                c.output.custom_path.clone()
            },
            Self::GitIgnore       => on_off(c.features.git_ignore).into(),
        }
    }

    /// Current value, as shown in the list. Style and directory structure show the
    /// effective value, so a CLI override at launch is visible.
    pub fn value(self, app: &App) -> String {
        match self {
            Self::OutputStyle  => app.output_style.label().to_string(),
            Self::DirStructure => on_off(app.dir_structure).to_string(),
            _ => self.show(&app.config),
        }
    }

    /// The value written to a fresh `config.json`, formatted like `value`.
    pub fn default_value(self) -> String {
        self.show(&default_config())
    }

    pub fn is_default(self, app: &App) -> bool {
        let d = default_config();
        match self {
            Self::OutputStyle  => app.output_style == d.output.style,
            Self::DirStructure => app.dir_structure == d.features.dir_structure,
            _ => self.json(&app.config) == self.json(&d),
        }
    }

    /// Optional `(title, lines)` shown under the list while this setting is selected.
    pub fn preview(self, app: &App) -> Option<(&'static str, Vec<String>)> {
        match self {
            Self::OutputStyle => {
                let mut v: Vec<String> = app.output_style
                    .file_header("src/main.rs").lines().map(String::from).collect();
                v.push(String::new());
                v.push("fn main() { … }".into());
                Some(("Preview", v))
            }
            Self::Filename | Self::TimestampSuffix | Self::OutputLocation | Self::CustomPath => {
                Some(("Next compile writes to", vec![app.output_path().display().to_string()]))
            }
            Self::DirStructure | Self::GitIgnore => None,
        }
    }

    // ── config.json plumbing ─────────────────────────────────────────────────

    /// Where this setting lives in `config.json`.
    fn path(self) -> &'static [&'static str] {
        match self {
            Self::OutputStyle     => &["output", "style"],
            Self::DirStructure    => &["features", "dirStructure"],
            Self::Filename        => &["output", "filename"],
            Self::TimestampSuffix => &["output", "timestampSuffix"],
            Self::OutputLocation  => &["output", "path"],
            Self::CustomPath      => &["output", "customPath"],
            Self::GitIgnore       => &["features", "gitIgnore"],
        }
    }

    /// This setting's value inside `c`, exactly as it is written in `config.json`.
    fn json(self, c: &AppConfig) -> Value {
        match self {
            Self::OutputStyle     => serde_json::to_value(c.output.style).unwrap_or(Value::Null),
            Self::DirStructure    => Value::Bool(c.features.dir_structure),
            Self::Filename        => Value::from(c.output.filename.clone()),
            Self::TimestampSuffix => Value::Bool(c.output.timestamp_suffix),
            Self::OutputLocation  => Value::from(c.output.path.clone()),
            Self::CustomPath      => Value::from(c.output.custom_path.clone()),
            Self::GitIgnore       => Value::Bool(c.features.git_ignore),
        }
    }

    /// Copy this one setting from `src` into `dst`.
    fn copy(self, dst: &mut AppConfig, src: &AppConfig) {
        match self {
            Self::OutputStyle     => dst.output.style = src.output.style,
            Self::DirStructure    => dst.features.dir_structure = src.features.dir_structure,
            Self::Filename        => dst.output.filename = src.output.filename.clone(),
            Self::TimestampSuffix => dst.output.timestamp_suffix = src.output.timestamp_suffix,
            Self::OutputLocation  => dst.output.path = src.output.path.clone(),
            Self::CustomPath      => dst.output.custom_path = src.output.custom_path.clone(),
            Self::GitIgnore       => dst.features.git_ignore = src.features.git_ignore,
        }
    }

    /// Mirror the config value into the `App` fields the compile path reads.
    fn sync(self, app: &mut App) {
        match self {
            Self::OutputStyle  => app.output_style  = app.config.output.style,
            Self::DirStructure => app.dir_structure = app.config.features.dir_structure,
            _ => {}
        }
    }

    fn save(self, app: &App) -> Result<()> {
        save_setting(self.path(), self.json(&app.config))
    }

    // ── Editing ──────────────────────────────────────────────────────────────
    /// Flip or cycle a `Kind::Cycle` setting and persist it.
    pub fn change(self, app: &mut App, forward: bool) {
        if self.kind() != Kind::Cycle { return; }
        self.cycle(app, forward);
        self.sync(app);
        let saved = self.save(app);
        let shown = self.value(app);
        let head = match saved {
            Ok(())  => format!("✓ {} → {shown} (saved)", self.label()),
            Err(e)  => format!("✗ {} → {shown} (not saved: {e})", self.label()),
        };
        let extra = if self.affects_output_name() { keep_output_ignored(app) } else { None };
        note(app, head, extra);
    }

    /// Save the text typed into the editor (`Kind::Text` only).
    pub fn commit(self, app: &mut App, text: String) {
        let head = match self.store_text(app, &text) {
            Ok(())  => format!("✓ {} saved", self.label()),
            Err(e)  => format!("✗ {} not saved: {e}", self.label()),
        };
        let extra = if self.affects_output_name() { keep_output_ignored(app) } else { None };
        note(app, head, extra);
    }

    /// Put one setting back to its default and persist it.
    pub fn reset(self, app: &mut App) {
        if self.is_default(app) {
            app.menu_note = format!("{} is already the default ({})", self.label(), self.value(app));
            return;
        }
        // Taken before the copy: these are the patterns the old output name produced.
        let old = if self.affects_output_name() { output_ignore_patterns(&app.config) } else { Vec::new() };
        let d = default_config();
        self.copy(&mut app.config, &d);
        self.sync(app);
        let saved = self.save(app);
        let shown = self.value(app);
        let head = match saved {
            Ok(())  => format!("✓ {} reset to default: {shown}", self.label()),
            Err(e)  => format!("✗ {} reset to {shown}, not saved: {e}", self.label()),
        };
        let extra = if self.affects_output_name() { reset_output_ignored(app, &old) } else { None };
        note(app, head, extra);
    }

    /// Put every setting in this tab back to its default.
    pub fn reset_all(app: &mut App) {
        let old = output_ignore_patterns(&app.config);
        let d = default_config();
        let mut first_err = None;
        for s in Self::ALL {
            s.copy(&mut app.config, &d);
            s.sync(app);
            if let Err(e) = s.save(app) {
                if first_err.is_none() { first_err = Some(e); }
            }
        }
        let head = match first_err {
            None    => "✓ All settings reset to defaults (saved)".to_string(),
            Some(e) => format!("✗ Settings reset, but not saved: {e}"),
        };
        let extra = reset_output_ignored(app, &old);
        note(app, head, extra);
    }

    /// The text to seed the editor with (`Kind::Text` only).
    pub fn raw_text(self, app: &App) -> String {
        match self {
            Self::Filename   => output_stem(&app.config.output.filename),
            Self::CustomPath => app.config.output.custom_path.clone(),
            _ => String::new(),
        }
    }

    fn cycle(self, app: &mut App, forward: bool) {
        match self {
            Self::OutputStyle => {
                app.config.output.style =
                    if forward { app.output_style.next() } else { app.output_style.prev() };
            }
            Self::DirStructure    => app.config.features.dir_structure = !app.dir_structure,
            Self::TimestampSuffix => {
                app.config.output.timestamp_suffix = !app.config.output.timestamp_suffix;
            }
            Self::OutputLocation => {
                let next = if app.config.output.path == "custom" { "default" } else { "custom" };
                app.config.output.path = next.to_string();
            }
            Self::GitIgnore => app.config.features.git_ignore = !app.config.features.git_ignore,
            Self::Filename | Self::CustomPath => {} // edited as text, never cycled
        }
    }

    fn store_text(self, app: &mut App, text: &str) -> Result<()> {
        match self {
            Self::Filename   => app.config.output.filename = filename_from_input(text)?,
            Self::CustomPath => app.config.output.custom_path = text.trim().to_string(),
            _ => return Ok(()),
        }
        self.save(app)
    }
}

/// Turn what the user typed into the stored file name. The extension is always `.txt`.
fn filename_from_input(text: &str) -> Result<String> {
    let t = text.trim();
    if t.is_empty() {
        bail!("name can't be empty (press D to restore the default)");
    }
    if let Some(c) = t.chars().find(|c| c.is_control() || r#"/\:*?"<>|"#.contains(*c)) {
        bail!("a filename can't contain {c:?}");
    }
    Ok(format!("{}.txt", output_stem(t)))
}

/// Log the outcome and show it inside the menu (the log panel is hidden behind it).
/// `extra` becomes a second line in the menu and is appended to the log line.
fn note(app: &mut App, head: String, extra: Option<String>) {
    match extra {
        Some(x) => {
            app.log(format!("{head} · {x}"));
            app.menu_note = format!("{head}\n{x}");
        }
        None => {
            app.log(head.clone());
            app.menu_note = head;
        }
    }
}

/// Keep the compiled output file out of its own input: put its pattern in `baseIgnore`
/// (config.json) and in the live pattern list, which also refilters the file list.
/// Returns a menu line when something was added or failed.
fn keep_output_ignored(app: &mut App) -> Option<String> {
    let (pat, res) = ensure_output_ignored(&mut app.config);
    app.add_pattern(pat.clone()); // live list, even if config.json could not be written
    match res {
        Ok(true)  => Some(format!("+ baseIgnore: {pat}")),
        Ok(false) => None,
        Err(e)    => Some(format!("✗ baseIgnore not saved: {e}")),
    }
}

/// A reset puts the output name back, so take the patterns that were added for the old name
/// out of `baseIgnore` again (config.json and the live list), then make sure the pattern the
/// output file needs now is there. `old` = what the output name could produce before the reset.
/// Returns a menu line when something was removed, added or failed.
fn reset_output_ignored(app: &mut App, old: &[String]) -> Option<String> {
    let keep = output_ignore_pattern(&app.config);
    let (removed, written) = release_base_ignore(&mut app.config, old, &keep);
    app.remove_patterns(&removed); // live list, even if config.json could not be written
    let added = keep_output_ignored(app);

    let mut parts: Vec<String> = Vec::new();
    match written {
        Err(e) => parts.push(format!("✗ baseIgnore not saved: {e}")),
        Ok(()) if !removed.is_empty() => {
            parts.push(format!("- baseIgnore: {}", removed.join(", ")));
        }
        Ok(()) => {}
    }
    if let Some(x) = added { parts.push(x); }
    if parts.is_empty() { None } else { Some(parts.join(" · ")) }
}

fn on_off(b: bool) -> &'static str { if b { "on" } else { "off" } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::OutputStyle;

    #[test]
    fn labels_are_unique_and_descriptions_fit() {
        for (i, a) in Setting::ALL.iter().enumerate() {
            assert!(a.description().chars().count() <= 64, "{a:?}: description too long for the menu");
            for b in &Setting::ALL[i + 1..] {
                assert_ne!(a.label(), b.label());
                assert_ne!(a.path(), b.path());
            }
        }
    }

    #[test]
    fn every_setting_path_exists_in_the_default_file() {
        let d = default_config();
        let root = serde_json::to_value(&d).unwrap();
        for s in Setting::ALL {
            let mut node = &root;
            for k in s.path() {
                node = node.get(*k).unwrap_or_else(|| panic!("{s:?}: no key `{k}`"));
            }
            assert_eq!(*node, s.json(&d), "{s:?}");
        }
    }

    #[test]
    fn copying_from_default_restores_every_setting() {
        let d = default_config();
        let mut c = AppConfig::default();
        c.output.style = OutputStyle::Comment;
        c.output.filename = "x.txt".into();
        c.output.timestamp_suffix = true;
        c.output.path = "custom".into();
        c.output.custom_path = "D:/out".into();
        c.features.dir_structure = false;
        c.features.git_ignore = false;
        for s in Setting::ALL {
            assert_ne!(s.json(&c), s.json(&d), "{s:?} should differ before the reset");
            s.copy(&mut c, &d);
            assert_eq!(s.json(&c), s.json(&d), "{s:?}");
        }
    }

    #[test]
    fn typed_filenames_always_end_in_txt() {
        assert_eq!(filename_from_input("notes").unwrap(), "notes.txt");
        assert_eq!(filename_from_input(" notes.txt ").unwrap(), "notes.txt");
        assert_eq!(filename_from_input("out.md").unwrap(), "out.md.txt");
        assert!(filename_from_input("  ").is_err());
        assert!(filename_from_input("a/b").is_err());
        assert!(filename_from_input("a:b").is_err());
    }
}