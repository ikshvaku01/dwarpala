use super::{App, Panel, Status, MenuTab};
use crate::{edition::Edition, worker::{spawn_worker, Msg}};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyEvent};
use ratatui::prelude::*;
use std::{path::PathBuf, sync::mpsc, time::{Duration, Instant}, thread};

pub const GATED_FEATURES: &[(Edition, &str)] = &[
    (Edition::Pro,        "Clipboard Copy"),
    (Edition::Pro,        "Hateration"),
    (Edition::Pro,        "Ma Sirene"),
    (Edition::Pro,        "Git Version/Diff"),
    (Edition::Team,       "Agent Picker"),
    (Edition::Team,       "Analyzer"),
    (Edition::Enterprise, "Live FS Watcher"),
    (Edition::Enterprise, "Ecoute"),
    (Edition::Enterprise, "Masker"),
    (Edition::Enterprise, "Per-Project Config"),
    (Edition::Enterprise, "Test Generation"),
];

/// Reason the main TUI loop exited.
pub enum AppExit {
    Quit,
    /// User pressed [L]/[Enter] in the License menu tab.
    /// Enterprise main should show the license dialog then relaunch.
    OpenLicenseScreen,
}

pub enum AppEvent {
    Key(KeyEvent),
    FsCreate(PathBuf),
    FsRemove(PathBuf),
    FsModify(PathBuf),
    Worker(Msg),
    /// Fired from the License menu tab — causes run_app to return OpenLicenseScreen.
    RequestLicenseScreen,
    Tick,
}

pub fn run_app<B: Backend>(
    terminal:    &mut Terminal<B>,
    mut app:     App,
    rx:          mpsc::Receiver<AppEvent>,
    tx:          mpsc::Sender<AppEvent>,
    output_path: PathBuf,
) -> Result<AppExit>
where
    B::Error: Send + Sync + 'static,
{
    // Tick thread
    { let tx = tx.clone(); thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(50));
        if tx.send(AppEvent::Tick).is_err() { break; }
    }); }
    // Input thread
    { let tx = tx.clone(); thread::spawn(move || loop {
        if let Ok(Event::Key(k)) = event::read() {
            if tx.send(AppEvent::Key(k)).is_err() { break; }
        }
    }); }

    loop {
        let mut should_draw = false;

        while let Ok(ev) = rx.try_recv() {
            should_draw = true;
            match ev {
                AppEvent::Tick => {
                    if let Some(t) = app.t_start { app.elapsed = t.elapsed(); }
                }
                AppEvent::FsCreate(p)  => app.fs_add(p),
                AppEvent::FsRemove(p)  => app.fs_remove(p),
                AppEvent::FsModify(_)  => {}
                AppEvent::RequestLicenseScreen => {
                    app.close_menu();
                    return Ok(AppExit::OpenLicenseScreen);
                }
                AppEvent::Worker(msg) => handle_worker_msg(&mut app, msg),
                AppEvent::Key(key) => {
                    if key.kind != KeyEventKind::Press { continue; }
                    if app.menu_open {
                        handle_menu_key(&mut app, &tx, key);
                    } else if let Some(exit) = dispatch_main_key(&mut app, &tx, key, &output_path) {
                        return Ok(exit);
                    }
                }
            }
        }

        if should_draw {
            terminal.draw(|f| crate::ui::rendering::draw::draw(f, &app))?;
        }
    }
}

// ── Main TUI key dispatch ────────────────────────────────────────────────────

fn dispatch_main_key(
    app:         &mut App,
    tx:          &mpsc::Sender<AppEvent>,
    key:         KeyEvent,
    output_path: &PathBuf,
) -> Option<AppExit> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return Some(AppExit::Quit),

        KeyCode::Char('m') | KeyCode::Char('M') => app.open_menu(),

        KeyCode::Char('r') | KeyCode::Char('R')
            if !matches!(app.status, Status::Running) =>
        {
            app.status   = Status::Running;
            app.progress = 0;
            app.t_start  = Some(Instant::now());
            app.log("▶ Starting compilation…");
            let tx_w  = tx.clone();
            let paths = app.files.iter().map(|r| app.root.join(r)).collect();
            let pats  = app.patterns.clone();
            let root  = app.root.clone();
            let out   = output_path.clone();
            let ds    = app.dir_structure;
            spawn_worker(root, out, paths, pats, ds, move |msg| {
                let _ = tx_w.send(AppEvent::Worker(msg));
            });
        }

        KeyCode::Char('-')
            if app.panel == Panel::Patterns && !app.patterns.is_empty() =>
        {
            let p = app.patterns[app.p_scroll].clone();
            app.remove_pattern(&p);
        }

        KeyCode::Tab      => app.next_panel(),
        KeyCode::Up       => app.scroll(-1),
        KeyCode::Down     => app.scroll(1),
        KeyCode::PageUp   => app.scroll(-10),
        KeyCode::PageDown => app.scroll(10),
        _ => {}
    }
    None
}

// ── Menu key handling ────────────────────────────────────────────────────────

fn handle_menu_key(app: &mut App, tx: &mpsc::Sender<AppEvent>, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => { app.close_menu(); return; }

        // Number keys → tab selection (no text input in menu so no conflicts)
        KeyCode::Char('1') => { app.menu_tab = MenuTab::Home;    return; }
        KeyCode::Char('2') => { app.menu_tab = MenuTab::License; return; }
        KeyCode::Char('3') => { app.menu_tab = MenuTab::Credits; return; }

        // Arrow keys → tab cycling
        KeyCode::Left => {
            app.menu_tab = match app.menu_tab {
                MenuTab::Home    => MenuTab::Credits,
                MenuTab::License => MenuTab::Home,
                MenuTab::Credits => MenuTab::License,
            };
            return;
        }
        KeyCode::Right => {
            app.menu_tab = match app.menu_tab {
                MenuTab::Home    => MenuTab::License,
                MenuTab::License => MenuTab::Credits,
                MenuTab::Credits => MenuTab::Home,
            };
            return;
        }
        _ => {}
    }

    // Tab-specific actions
    if app.menu_tab == MenuTab::License {
        match key.code {
            KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Enter => {
                if app.has_license_screen {
                    let _ = tx.send(AppEvent::RequestLicenseScreen);
                }
                // Community binary: no-op — the draw layer shows "not available"
            }
            _ => {}
        }
    }
}

// ── Worker ───────────────────────────────────────────────────────────────────

fn handle_worker_msg(app: &mut App, msg: Msg) {
    match msg {
        Msg::Progress { done, total, file } => {
            app.progress = done;
            app.total    = total;
            app.cur_file = file.clone();
            app.log(format!("[{}/{}] {file}", done + 1, total));
        }
        Msg::Log(s) => app.log(s),
        Msg::Done { path, count } => {
            app.elapsed  = app.t_start.map(|t| t.elapsed()).unwrap_or_default();
            app.progress = count;
            app.total    = count;
            app.log(format!("✓ Done — {count} files in {:.1}s", app.elapsed.as_secs_f32()));
            app.status   = Status::Done { path, count };
        }
        Msg::Err(e) => {
            app.log(format!("✗ Error: {e}"));
            app.status = Status::Err(e);
        }
    }
}