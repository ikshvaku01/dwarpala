pub mod events;

use crate::edition::Edition;
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
pub enum MenuTab { Home, License, Credits }

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
    // ── Menu overlay ──────────────────────────────────
    pub menu_open: bool,
    pub menu_tab:  MenuTab,
    /// true  → this binary has a license screen reachable via [L]
    /// false → community binary, no license screen
    pub has_license_screen: bool,
}

impl App {
    pub fn new(
        root:          PathBuf,
        ptype:         &str,
        edition:       Edition,
        patterns:      Vec<String>,
        files:         Vec<String>,
        dir_structure: bool,
    ) -> Self {
        let all_files = files.iter().map(|r| root.join(r)).collect();
        Self {
            root, ptype: ptype.into(), edition, patterns, all_files, files,
            status: Status::Idle, progress: 0, total: 0, cur_file: String::new(),
            log: vec![format!(
                "Ready [{}] — [R] compile  [Tab] switch  [↑↓] scroll  [M] menu  [Q] quit",
                edition.label()
            )],
            f_scroll: 0, p_scroll: 0, l_scroll: 0,
            panel: Panel::Files, elapsed: Duration::ZERO, t_start: None, dir_structure,
            menu_open: false,
            menu_tab:  MenuTab::Home,
            has_license_screen: false,
        }
    }

    pub fn open_menu(&mut self)  { self.menu_open = true;  self.menu_tab = MenuTab::Home; }
    pub fn close_menu(&mut self) { self.menu_open = false; }

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
        self.patterns.retain(|p| p != pattern);
        self.refilter();
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