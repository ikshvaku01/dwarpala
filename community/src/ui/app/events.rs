use super::{App, Panel, Status, MenuTab, MenuFocus};
use crate::{edition::Edition, ui::settings::{Kind, Setting}, worker::{spawn_worker, Msg}};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyEvent, KeyModifiers};
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
    terminal: &mut Terminal<B>,
    mut app:  App,
    rx:       mpsc::Receiver<AppEvent>,
    tx:       mpsc::Sender<AppEvent>,
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
                    } else if let Some(exit) = dispatch_main_key(&mut app, &tx, key) {
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
    app: &mut App,
    tx:  &mpsc::Sender<AppEvent>,
    key: KeyEvent,
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
            let out   = app.output_path();
            let ds    = app.dir_structure;
            let style = app.output_style;
            spawn_worker(root, out, paths, pats, ds, style, move |msg| {
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
//
// Three layers, as in most terminal UIs:
//
//   Tabs     ←/→ switch tab   ↓/Enter go into the tab   Esc close menu
//   Content  ↑/↓ pick a row   ←/→/Enter change the row  Esc back to Tabs
//   Editing  typing edits a text field                  Enter save · Esc cancel
//
// Tab / Shift+Tab, [ ] and 1-4 switch tabs from Tabs and Content, so one key always
// works wherever focus is. Esc peels back one layer.

fn handle_menu_key(app: &mut App, tx: &mpsc::Sender<AppEvent>, key: KeyEvent) {
    // A text field being edited owns the keyboard.
    if app.editing.is_some() {
        handle_edit_key(app, key);
        return;
    }

    if matches!(key.code, KeyCode::Char('m' | 'M')) {
        app.close_menu();
        return;
    }

    // Tab switching that works from every level.
    let target = match key.code {
        KeyCode::Char('1') => Some(MenuTab::Home),
        KeyCode::Char('2') => Some(MenuTab::License),
        KeyCode::Char('3') => Some(MenuTab::Settings),
        KeyCode::Char('4') => Some(MenuTab::Credits),
        KeyCode::Tab     | KeyCode::Char(']') => Some(app.menu_tab.next()),
        KeyCode::BackTab | KeyCode::Char('[') => Some(app.menu_tab.prev()),
        _ => None,
    };
    if let Some(tab) = target {
        app.goto_tab(tab);
        return;
    }

    match app.menu_focus {
        MenuFocus::Tabs    => handle_tabs_key(app, tx, key),
        MenuFocus::Content => handle_content_key(app, key),
    }
}

fn handle_tabs_key(app: &mut App, tx: &mpsc::Sender<AppEvent>, key: KeyEvent) {
    match (app.menu_tab.clone(), key.code) {
        (_, KeyCode::Esc)   => app.close_menu(),
        (_, KeyCode::Left)  => app.goto_tab(app.menu_tab.prev()),
        (_, KeyCode::Right) => app.goto_tab(app.menu_tab.next()),
        // Tabs with interactive content: step into them.
        (MenuTab::Settings, KeyCode::Down | KeyCode::Enter) => app.menu_focus = MenuFocus::Content,
        // The License tab has a single action, so Enter / L triggers it directly.
        (MenuTab::License, KeyCode::Enter | KeyCode::Char('l' | 'L')) => {
            if app.has_license_screen {
                let _ = tx.send(AppEvent::RequestLicenseScreen);
            }
            // Community binary: no-op, the draw layer says "not available".
        }
        _ => {}
    }
}

fn handle_content_key(app: &mut App, key: KeyEvent) {
    // A pending "reset all?" prompt takes the next key: Y / Enter confirms, anything else cancels.
    if app.confirm_reset {
        app.confirm_reset = false;
        app.menu_note.clear();
        if matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) {
            Setting::reset_all(app);
        }
        return;
    }

    // `None` means the cursor is on the "Reset all" button row below the list.
    match (app.selected_setting(), key.code) {
        (_, KeyCode::Esc)       => app.menu_focus = MenuFocus::Tabs,
        (_, KeyCode::Up)        => app.settings_move(-1),
        (_, KeyCode::Down)      => app.settings_move(1),
        (_, KeyCode::Char('D')) => ask_reset_all(app),
        (None, KeyCode::Enter | KeyCode::Char(' ')) => ask_reset_all(app),
        (None, _) => {}
        (Some(s), KeyCode::Char('d')) => s.reset(app),
        (Some(s), KeyCode::Left)      => s.change(app, false),
        (Some(s), KeyCode::Right | KeyCode::Char(' ')) => s.change(app, true),
        (Some(s), KeyCode::Enter) => match s.kind() {
            Kind::Cycle => s.change(app, true),
            Kind::Text  => app.editing = Some(s.raw_text(app)),
        },
        _ => {}
    }
}

fn ask_reset_all(app: &mut App) {
    app.menu_note.clear();
    app.confirm_reset = true;
}

fn handle_edit_key(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => { app.editing = None; app.menu_note.clear(); }
        KeyCode::Enter => {
            let text = app.editing.take().unwrap_or_default();
            if let Some(s) = app.selected_setting() { s.commit(app, text); }
        }
        KeyCode::Backspace => { if let Some(b) = app.editing.as_mut() { b.pop(); } }
        KeyCode::Char('u') if ctrl => { if let Some(b) = app.editing.as_mut() { b.clear(); } }
        KeyCode::Char(c) if !ctrl  => { if let Some(b) = app.editing.as_mut() { b.push(c); } }
        _ => {}
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
            // take() stops the timer; otherwise the next Tick overwrites `elapsed` and keeps counting.
            app.elapsed  = app.t_start.take().map(|t| t.elapsed()).unwrap_or_default();
            app.progress = count;
            app.total    = count;
            app.log(format!("✓ Done — {count} files in {:.1}s", app.elapsed.as_secs_f32()));
            app.status   = Status::Done { path, count };
        }
        Msg::Err(e) => {
            app.elapsed = app.t_start.take().map(|t| t.elapsed()).unwrap_or_default();
            app.log(format!("✗ Error: {e}"));
            app.status = Status::Err(e);
        }
    }
}