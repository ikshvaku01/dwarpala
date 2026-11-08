pub mod events;

use crate::{config::AppConfig, edition::Edition, output::OutputStyle, ui::settings::Setting};
use std::{collections::BTreeSet, path::PathBuf, time::{Duration, Instant}};

#[derive(Clone, PartialEq)]
pub enum Status {
    Idle,
    Running,
    Done { path: PathBuf, count: usize },
    Err(String),
}

#[derive(Clone, PartialEq)]
pub enum Panel { Files, Patterns, Log }

#[derive(Clone, PartialEq)]
pub enum MenuTab { Home, License, Settings, Credits }

/// Which layer of the menu owns the arrow keys.
#[derive(Clone, Copy, PartialEq)]
pub enum MenuFocus { Tabs, Content }

impl MenuTab {
    pub fn next(&self) -> Self {
        match self {
            Self::Home => Self::License, Self::License => Self::Settings,
            Self::Settings => Self::Credits, Self::Credits => Self::Home,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            Self::Home => Self::Credits, Self::License => Self::Home,
            Self::Settings => Self::License, Self::Credits => Self::Settings,
        }
    }
}

pub struct App {
    pub root:          PathBuf,
    pub ptype:         String,
    pub edition:       Edition,
    pub patterns:      Vec<String>,
    pub all_files:     BTreeSet<PathBuf>,
    pub files:         Vec<String>,
    pub status:        Status,
    pub progress:      usize,
    pub total:         usize,
    pub cur_file:      String,
    pub log:           Vec<String>,
    pub f_scroll:      usize,
    pub p_scroll:      usize,
    pub l_scroll:      usize,
    pub panel:         Panel,
    pub elapsed:       Duration,
    pub t_start:       Option<Instant>,
    pub dir_structure: bool,
    pub output_style:  OutputStyle,
    /// Effective config (file + CLI overrides). Saving writes single keys, never this whole struct.
    pub config:        AppConfig,
    // ── Menu overlay ──────────────────────────────────
    pub menu_open:     bool,
    pub menu_tab:      MenuTab,
    pub menu_focus:    MenuFocus,
    pub settings_sel:  usize,
    /// `Some(buffer)` while a text setting is being edited.
    pub editing:       Option<String>,
    /// Last save result, shown at the bottom of the Settings tab.
    pub menu_note:     String,
    pub has_license_screen: bool,
    /// True while "Reset all settings?" is waiting for Y.
    pub confirm_reset: bool,
}

impl App {
    pub fn new(
        root: PathBuf, ptype: &str, edition: Edition, patterns: Vec<String>,
        files: Vec<String>, dir_structure: bool,
        output_style: OutputStyle,
        config: AppConfig,
    ) -> Self {
        let all_files = files.iter().map(|r| root.join(r)).collect();
        Self {
            root, ptype: ptype.into(), edition, patterns, all_files, files,
            status: Status::Idle, progress: 0, total: 0, cur_file: String::new(),
            log: vec![format!("Ready [{}]", edition.label())],
            f_scroll: 0, p_scroll: 0, l_scroll: 0,
            panel: Panel::Files, elapsed: Duration::ZERO, t_start: None, dir_structure,
            output_style,
            config,
            menu_open: false,
            menu_tab:  MenuTab::Home,
            menu_focus: MenuFocus::Tabs,
            settings_sel: 0,
            editing: None,
            confirm_reset: false,
            menu_note: String::new(),
            has_license_screen: false,
        }
    }

    pub fn open_menu(&mut self) {
        self.menu_open     = true;
        self.menu_tab      = MenuTab::Home;
        self.menu_focus    = MenuFocus::Tabs;
        self.editing       = None;
        self.confirm_reset = false;
        self.menu_note.clear();
    }
    pub fn close_menu(&mut self) {
        self.menu_open     = false;
        self.editing       = None;
        self.confirm_reset = false;
    }

    /// Switching tabs always returns focus to the tab bar.
    pub fn goto_tab(&mut self, tab: MenuTab) {
        self.menu_tab      = tab;
        self.menu_focus    = MenuFocus::Tabs;
        self.confirm_reset = false;
        self.menu_note.clear();
    }

    /// Where the next compile writes. Resolved on demand so settings take effect immediately.
    pub fn output_path(&self) -> PathBuf {
        crate::utils::resolve_output_path(&self.root, &self.config)
    }

    /// The setting under the cursor, or `None` when the cursor is on the "Reset all" button row.
    pub fn selected_setting(&self) -> Option<Setting> {
        Setting::ALL.get(self.settings_sel).copied()
    }

    pub fn settings_move(&mut self, d: isize) {
        let n = Setting::ALL.len() as isize + 1; // +1 for the "Reset all" button row
        self.settings_sel = (self.settings_sel as isize + d).rem_euclid(n) as usize;
        self.menu_note.clear();
    }

    pub fn fs_add(&mut self, path: PathBuf) {
        self.all_files.insert(path.clone());
        let rel = self.rel(&path);
        if !self.is_ignored(&rel) && !self.files.contains(&rel) {
            let pos = self.files.partition_point(|f| f.as_str() < rel.as_str());
            self.files.insert(pos, rel.clone());
            self.log(format!("+ {rel}"));
        }
    }

    pub fn fs_remove(&mut self, path: PathBuf) {
        self.all_files.remove(&path);
        let rel = self.rel(&path);
        if let Some(i) = self.files.iter().position(|f| *f == rel) {
            self.files.remove(i);
            self.log(format!("- {rel}"));
            self.f_scroll = self.f_scroll.min(self.files.len().saturating_sub(1));
        }
    }

    pub fn add_pattern(&mut self, pattern: String) {
        if self.patterns.contains(&pattern) { return; }
        self.patterns.push(pattern);
        self.refilter();
    }

    pub fn remove_pattern(&mut self, pattern: &str) {
        self.remove_patterns(&[pattern.to_string()]);
    }

    /// Drop several patterns and refilter once.
    pub fn remove_patterns(&mut self, pats: &[String]) {
        let before = self.patterns.len();
        self.patterns.retain(|p| !pats.contains(p));
        // Without this clamp, pressing `-` on the last pattern and then `-` again could index past the end.
        self.p_scroll = self.p_scroll.min(self.patterns.len().saturating_sub(1));
        if self.patterns.len() != before {
            self.refilter();
        }
    }

    pub fn refilter(&mut self) {
        self.files = self.all_files.iter()
            .map(|p| self.rel(p))
            .filter(|r| !self.is_ignored(r))
            .collect();
        self.f_scroll = self.f_scroll.min(self.files.len().saturating_sub(1));
        self.log(format!("↺ {} files (patterns: {})", self.files.len(), self.patterns.len()));
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        self.l_scroll = self.log.len().saturating_sub(1);
    }

    pub fn scroll(&mut self, d: i32) {
        let (cur, max) = match self.panel {
            Panel::Files    => (&mut self.f_scroll, self.files.len()),
            Panel::Patterns => (&mut self.p_scroll, self.patterns.len()),
            Panel::Log      => (&mut self.l_scroll, self.log.len()),
        };
        if max == 0 { return; }
        *cur = ((*cur as i64) + d as i64).clamp(0, (max - 1) as i64) as usize;
    }

    pub fn next_panel(&mut self) {
        self.panel = match self.panel {
            Panel::Files    => Panel::Patterns,
            Panel::Patterns => Panel::Log,
            Panel::Log      => Panel::Files,
        };
    }

    fn rel(&self, path: &PathBuf) -> String {
        path.strip_prefix(&self.root).unwrap_or(path)
            .to_string_lossy().replace('\\', "/")
    }

    fn is_ignored(&self, rel: &str) -> bool {
        crate::ignore::engine::should_ignore(rel, &self.patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(PathBuf::from("."), "rust", Edition::Community, vec![], vec![], true,
                 OutputStyle::Banner, AppConfig::default())
    }

    #[test]
    fn selection_wraps_through_the_reset_button_row() {
        let mut a = app();
        a.settings_move(-1);                      // up from the first row lands on the button
        assert_eq!(a.selected_setting(), None);
        a.settings_move(1);                       // down from the button wraps to the first row
        assert_eq!(a.selected_setting(), Some(Setting::ALL[0]));
        for _ in 0..Setting::ALL.len() { a.settings_move(1); }
        assert_eq!(a.selected_setting(), None);
    }
}