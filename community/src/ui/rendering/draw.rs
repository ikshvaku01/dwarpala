use crate::ui::app::{App, Panel, Status, MenuTab, MenuFocus};
use crate::ui::settings::Setting;
use ratatui::{prelude::*, widgets::*};
use ratatui::widgets::Clear;

// ── Entry point ──────────────────────────────────────────────────────────────

pub fn draw(f: &mut Frame, app: &App) {
    let outer = Block::default()
        .title(Span::styled(" ◈ Dwarpala ", Style::new().fg(Color::Cyan).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::DarkGray));
    let inner = outer.inner(f.area());
    f.render_widget(outer, f.area());

    let [hdr, mid, prog, ftr] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(3),
        Constraint::Length(1),
    ]).areas(inner);

    header(f, hdr, app);
    body(f, mid, app);
    progress_bar(f, prog, app);
    footer(f, ftr, app);

    // Menu overlay is rendered last so it sits on top of everything.
    if app.menu_open {
        draw_menu_overlay(f, app);
    }
}

// ── Main TUI sections ────────────────────────────────────────────────────────
fn header(f: &mut Frame, area: Rect, app: &App) {
    let status_span = match &app.status {
        Status::Idle               => Span::styled("● IDLE",    Style::new().fg(Color::Yellow)),
        Status::Running            => Span::styled("▶ RUNNING", Style::new().fg(Color::Green)),
        Status::Done { count, .. } =>
            Span::styled(format!("✓ DONE ({count} files)"), Style::new().fg(Color::Cyan)),
        Status::Err(_)             => Span::styled("✗ ERROR",   Style::new().fg(Color::Red)),
    };
    let dim = Style::new().fg(Color::DarkGray);
    let mut spans = vec![
        Span::styled("  Root: ", dim),
        Span::styled(app.root.to_string_lossy().into_owned(), Style::new().fg(Color::White)),
        Span::raw("   "),
        Span::styled("Type: ", dim),
        Span::styled(app.ptype.clone(), Style::new().fg(Color::Magenta).bold()),
        Span::raw("   "),
        Span::styled("Files: ", dim),
        Span::styled(app.files.len().to_string(), Style::new().fg(Color::White)),
        Span::raw("   "),
        status_span,
    ];
    // The timer only means something once a compile has started.
    if !matches!(app.status, Status::Idle) {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(format!("{:.1}s", app.elapsed.as_secs_f32()), dim));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn body(f: &mut Frame, area: Rect, app: &App) {
    let [left, right] = Layout::horizontal([
        Constraint::Percentage(55),
        Constraint::Percentage(45),
    ]).areas(area);
    let [files_area, log_area] = Layout::vertical([
        Constraint::Percentage(60),
        Constraint::Percentage(40),
    ]).areas(left);
    file_list(f, files_area, app);
    pattern_list(f, right, app);
    log_panel(f, log_area, app);
}

fn panel_block(title: String, active: bool) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            if active { Style::new().fg(Color::Cyan).bold() }
            else      { Style::new().fg(Color::DarkGray) },
        ))
        .borders(Borders::ALL)
        .border_type(if active { BorderType::Thick } else { BorderType::Plain })
        .border_style(if active { Style::new().fg(Color::Cyan) }
                      else      { Style::new().fg(Color::DarkGray) })
}

fn file_list(f: &mut Frame, area: Rect, app: &App) {
    let active = app.panel == Panel::Files;
    let block  = panel_block(format!("Included Files ({})", app.files.len()), active);
    let inner  = block.inner(area);
    f.render_widget(block, area);
    let items: Vec<ListItem> = app.files.iter()
        .map(|s| ListItem::new(Span::styled(s.clone(), Style::new().fg(Color::White))))
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() { state.select(Some(app.f_scroll)); }
    f.render_stateful_widget(
        List::new(items).highlight_style(Style::new().fg(Color::Black).bg(Color::White)),
        inner, &mut state,
    );
}

fn pattern_list(f: &mut Frame, area: Rect, app: &App) {
    let active = app.panel == Panel::Patterns;
    let block  = panel_block(
        format!("Ignore Patterns ({})  [-] remove", app.patterns.len()), active,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let items: Vec<ListItem> = app.patterns.iter().enumerate().map(|(i, s)| {
        let style = if active && i == app.p_scroll {
            Style::new().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::new().fg(Color::Yellow)
        };
        ListItem::new(Span::styled(s.clone(), style))
    }).collect();
    let mut state = ListState::default();
    if !items.is_empty() { state.select(Some(app.p_scroll)); }
    f.render_stateful_widget(
        List::new(items).highlight_style(Style::new().fg(Color::Black).bg(Color::Yellow)),
        inner, &mut state,
    );
}

fn log_panel(f: &mut Frame, area: Rect, app: &App) {
    let active = app.panel == Panel::Log;
    let block  = panel_block("Log".to_string(), active);
    let inner  = block.inner(area);
    f.render_widget(block, area);
    let h     = inner.height as usize;
    let start = app.l_scroll.saturating_sub(h.saturating_sub(1));
    let lines: Vec<Line> = app.log.iter().skip(start).map(|l| {
        let color = if l.starts_with('+')      { Color::Green }
            else if l.starts_with('-')          { Color::Red }
            else if l.starts_with('✓')          { Color::Cyan }
            else if l.starts_with('✗')          { Color::Red }
            else if l.starts_with('↺')          { Color::Magenta }
            else                                { Color::Gray };
        Line::from(Span::styled(l.clone(), Style::new().fg(color)))
    }).collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn progress_bar(f: &mut Frame, area: Rect, app: &App) {
    let sep   = Block::default().borders(Borders::TOP)
        .border_style(Style::new().fg(Color::DarkGray));
    let inner = sep.inner(area);
    f.render_widget(sep, area);
    let [bar, lbl] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(inner);

    let (ratio, text, color) = match &app.status {
        Status::Idle    => (0.0, "  Not compiled yet".into(), Color::DarkGray),
        Status::Running => {
            let r = if app.total > 0 { app.progress as f64 / app.total as f64 } else { 0.0 };
            (r, format!("  [{}/{}] {}", app.progress, app.total, app.cur_file), Color::Gray)
        }
        Status::Done { path, count } =>
            (1.0, format!("  ✓ {count} files → {}", path.display()), Color::Cyan),
        Status::Err(e)  => (0.0, format!("  ✗ {e}"), Color::Red),
    };
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::new().fg(Color::Cyan).bg(Color::DarkGray))
            .ratio(ratio),
        bar,
    );
    f.render_widget(Paragraph::new(Span::styled(text, Style::new().fg(color))), lbl);
}

fn footer(f: &mut Frame, area: Rect, app: &App) {
    let keys = if app.menu_open {
        menu_hint(app)
    } else if matches!(app.status, Status::Running) {
        "[M] Menu  [Tab] Switch  [↑↓] Scroll  [Q] Quit"
    } else {
        "[R] Compile  [M] Menu  [Tab] Switch  [↑↓] Scroll  [-] Remove Pattern  [Q] Quit"
    };
    f.render_widget(
        Paragraph::new(Span::styled(keys, Style::new().fg(Color::DarkGray))), area,
    );
}

/// Key hints for whichever menu layer has focus.
fn menu_hint(app: &App) -> &'static str {
    if app.editing.is_some() {
        "Type to edit  [Enter] Save  [Esc] Cancel  [Ctrl+U] Clear"
    } else if app.confirm_reset {
        "Reset all settings to defaults?  [Y/Enter] Yes  [Esc] Cancel"
    } else if app.menu_focus == MenuFocus::Content {
        if app.selected_setting().is_none() {
            "[Enter] Reset all to defaults  [↑] Back to settings  [Esc] Back"
        } else {
            "[↑↓] Select  [←→/Enter] Change  [D] Reset  [Shift+D] All  [Esc] Back"
        }
    } else if app.menu_tab == MenuTab::Settings {
        "[←→] Switch tab  [↓/Enter] Edit / reset settings  [Esc] Close menu"
    } else {
        "[←→/Tab] Switch tab  [1-4] Jump  [Esc] Close menu"
    }
}

// ── Menu overlay ─────────────────────────────────────────────────────────────

fn draw_menu_overlay(f: &mut Frame, app: &App) {
    let area  = f.area();
    let w     = 74u16.min(area.width.saturating_sub(4));
    let h     = 30u16.min(area.height.saturating_sub(2));
    let popup = centered_fixed(w, h, area);

    // Erase whatever the main TUI rendered in this region.
    // This must happen before any other widget is drawn into `popup`.
    f.render_widget(Clear, popup);

    let chrome = Block::default()
        .title(Span::styled(" ◈ Menu ", Style::new().fg(Color::Cyan).bold()))
        .title_bottom(Span::styled(
            format!(" {} ", menu_hint(app)),
            Style::new().fg(Color::DarkGray),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::new().fg(Color::Cyan))
        .style(Style::new().bg(Color::Black));

    let inner = chrome.inner(popup);
    f.render_widget(chrome, popup);

    // Layout: tab bar | separator | content
    let [tab_row, sep_row, content] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ]).areas(inner);

    draw_tab_bar(f, tab_row, app);

    // Separator line
    f.render_widget(
        Block::default().borders(Borders::TOP)
            .border_style(Style::new().fg(Color::DarkGray)),
        sep_row,
    );

    match app.menu_tab {
        MenuTab::Home    => draw_home_tab(f, content, app),
        MenuTab::License => draw_license_tab(f, content, app),
        MenuTab::Settings => draw_settings_tab(f, content, app),
        MenuTab::Credits => draw_credits_tab(f, content, app),
    }
}

fn draw_tab_bar(f: &mut Frame, area: Rect, app: &App) {
    let tabs = [
        ("1: Home",     &MenuTab::Home),
        ("2: License",  &MenuTab::License),
        ("3: Settings", &MenuTab::Settings),
        ("4: Credits",  &MenuTab::Credits),
    ];
    let tabs_focused = app.menu_focus == MenuFocus::Tabs;
    let mut spans = vec![Span::raw(" ")];
    for (label, variant) in &tabs {
        let active = &app.menu_tab == *variant;
        let style  = if !active {
            Style::new().fg(Color::DarkGray)
        } else if tabs_focused {
            Style::new().fg(Color::Black).bg(Color::Cyan).bold()   // tab bar has the keyboard
        } else {
            Style::new().fg(Color::Cyan).bold().underlined()       // focus moved into the tab
        };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ── Home tab ─────────────────────────────────────────────────────────────────

fn draw_home_tab(f: &mut Frame, area: Rect, app: &App) {
    let dim  = Style::new().fg(Color::DarkGray);
    let hi   = Style::new().fg(Color::White);
    let bold = Style::new().fg(Color::Cyan).bold();

    let status_str = match &app.status {
        Status::Idle      => "Idle".to_string(),
        Status::Running   => format!("Running ({}/{})", app.progress, app.total),
        Status::Done { count, .. } => format!("Done — {count} files"),
        Status::Err(e)    => format!("Error: {e}"),
    };

    let files_count    = app.files.len().to_string();
    let patterns_count = app.patterns.len().to_string();

    let lines: Vec<Line> = vec![
        Line::from(Span::styled("  Session", bold)),
        Line::from(vec![
            Span::styled("  ─────────────────────────────────────", dim),
        ]),
        kv("  Project", &app.ptype, hi),
        kv("  Edition", app.edition.label(), hi),
        kv("  Style", app.output_style.label(), hi),
        kv("  Files", &files_count, hi),
        kv("  Patterns", &patterns_count, hi),
        kv("  Status", &status_str, hi),
        Line::from(""),
        Line::from(Span::styled("  Keyboard Shortcuts", bold)),
        Line::from(Span::styled("  ─────────────────────────────────────", dim)),
        shortcut("  [R]",       "Compile files into output document"),
        shortcut("  [M]",       "Open / close this menu"),
        shortcut("  [Tab]",     "Cycle between Files / Patterns / Log panels"),
        shortcut("  [↑↓]",      "Scroll the focused panel"),
        shortcut("  [PgUp/Dn]", "Scroll faster"),
        shortcut("  [-]",       "Remove selected ignore pattern"),
        shortcut("  [Q]",       "Quit"),
    ];

    f.render_widget(Paragraph::new(lines), area);
}

fn kv<'a>(key: &'a str, val: &'a str, val_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<14}"), Style::new().fg(Color::DarkGray)),
        Span::styled(val, val_style),
    ])
}

fn shortcut<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<14}"), Style::new().fg(Color::Cyan)),
        Span::styled(desc, Style::new().fg(Color::Gray)),
    ])
}

// ── License tab ──────────────────────────────────────────────────────────────

fn draw_license_tab(f: &mut Frame, area: Rect, app: &App) {
    let dim  = Style::new().fg(Color::DarkGray);
    let bold = Style::new().fg(Color::Cyan).bold();

    let tier_color = match app.edition {
        crate::edition::Edition::Community  => Color::DarkGray,
        crate::edition::Edition::Pro        => Color::Cyan,
        crate::edition::Edition::Team       => Color::Magenta,
        crate::edition::Edition::Enterprise => Color::Yellow,
    };

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled("  Current License", bold)),
        Line::from(Span::styled("  ─────────────────────────────────────", dim)),
        Line::from(vec![
            Span::styled("  Edition       ", dim),
            Span::styled(app.edition.label(), Style::new().fg(tier_color).bold()),
        ]),
        Line::from(""),
    ];

    if app.has_license_screen {
        lines.extend([
            Line::from(Span::styled(
                "  ┌─ License Manager ─────────────────────────────────────────┐",
                dim,
            )),
            Line::from(vec![
                Span::styled("  │  ", dim),
                Span::styled(
                    "[L] or [Enter]  Open the License Manager",
                    Style::new().fg(Color::Green).bold(),
                ),
                Span::styled("                   │", dim),
            ]),
            Line::from(Span::styled(
                "  └────────────────────────────────────────────────────────────┘",
                dim,
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter and validate your Pro / Team / Enterprise key in the",
                Style::new().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  License Manager. The key is saved encrypted and applied on",
                Style::new().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  the next launch.",
                Style::new().fg(Color::Gray),
            )),
        ]);
    } else {
        lines.extend([
            Line::from(Span::styled(
                "  License management is not available in the Community binary.",
                Style::new().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  To unlock Pro / Team / Enterprise features, download the",
                Style::new().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  Enterprise edition from:",
                Style::new().fg(Color::Gray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("    {}", repository()),
                Style::new().fg(Color::Cyan),
            )),
        ]);
    }

    f.render_widget(Paragraph::new(lines), area);
}

// ── Credits tab ──────────────────────────────────────────────────────────────
/// Repository URL from `[workspace.package]` in the root Cargo.toml.
/// Cargo hands it to this crate at build time because community/Cargo.toml has
/// `repository.workspace = true`. Edit it in one place and rebuild.
fn repository() -> &'static str {
    match env!("CARGO_PKG_REPOSITORY") {
        "" => "(not set)",
        url => url,
    }
}

/// Version from `[workspace.package]` in the root Cargo.toml.
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Minimum Rust version from `[workspace.package]`, shown as e.g. `1.93+`.
fn rust_requirement() -> String {
    match env!("CARGO_PKG_RUST_VERSION") {
        "" => "(not set)".to_string(),
        v  => format!("{v}+"),
    }
}

// Generated by community/build.rs from the workspace Cargo.toml.
include!(concat!(env!("OUT_DIR"), "/dep_versions.rs"));

/// Version of `name` as declared in the workspace dependency table, or `?` if it isn't there.
fn dep_version(name: &str) -> &'static str {
    DEP_VERSIONS.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| *v)
        .unwrap_or("?")
}

/// Crates listed under "Core Libraries": (crate name, description).
const CORE_LIBS: &[(&str, &str)] = &[
    ("ratatui",   "TUI framework"),
    ("crossterm", "Terminal backend"),
    ("clap",      "Argument parsing"),
    ("walkdir",   "Directory traversal"),
    ("notify",    "Filesystem events"),
    ("serde",     "Config serialisation"),
    ("regex",     "Pattern matching"),
];

fn draw_credits_tab(f: &mut Frame, area: Rect, app: &App) {
    let dim  = Style::new().fg(Color::DarkGray);
    let bold = Style::new().fg(Color::Cyan).bold();
    let gray = Style::new().fg(Color::Gray);

    // Both come from [workspace.package] in the root Cargo.toml.
    let title = format!("  ◈ Dwarpala  v{}", version());
    let rust  = rust_requirement();

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(title, bold)),
        Line::from(Span::styled(
            "  ────────────────────────────────────────────────────",
            dim,
        )),
        Line::from(Span::styled(
            "  Compile and transform codebases into AI-ready datasets.",
            gray,
        )),
        Line::from(""),
        kv("  Edition",    app.edition.label(),                     Style::new().fg(Color::White)),
        kv("  Repository", repository(),                            Style::new().fg(Color::Cyan)),
        kv("  License",    "MIT (Community) / Proprietary (Enterprise)", gray),
        kv("  Rust",       &rust,                                    gray),
        Line::from(""),
        Line::from(Span::styled(
            "  Built with ♥ using Rust",
            Style::new().fg(Color::Magenta),
        )),
        Line::from(""),
        Line::from(Span::styled("  Core Libraries", bold)),
        Line::from(Span::styled(
            "  ────────────────────────────────────────────────────",
            dim,
        )),
    ];
    lines.extend(CORE_LIBS.iter().map(|&(name, desc)| dep(name, desc)));

    f.render_widget(Paragraph::new(lines), area);
}

fn dep(name: &'static str, desc: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {name:<12}"),        Style::new().fg(Color::Cyan)),
        Span::styled(format!("{:<8}", dep_version(name)), Style::new().fg(Color::DarkGray)),
        Span::styled(desc,                           Style::new().fg(Color::Gray)),
    ])
}

/// Keep the tail of `s` (the interesting end of a path) when it is longer than `max`.
fn fit(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max { return s.to_string(); }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

fn draw_settings_tab(f: &mut Frame, area: Rect, app: &App) {
    let dim     = Style::new().fg(Color::DarkGray);
    let bold    = Style::new().fg(Color::Cyan).bold();
    let gray    = Style::new().fg(Color::Gray);
    let yellow  = Style::new().fg(Color::Yellow);
    let focused = app.menu_focus == MenuFocus::Content;
    let current = app.selected_setting(); // None = the "Reset all" button row

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled("  Settings", bold)),
        Line::from(Span::styled("  ─────────────────────────────────────", dim)),
    ];

    let mut group = "";
    for (i, s) in Setting::ALL.iter().enumerate() {
        if s.group() != group {
            group = s.group();
            if i > 0 { lines.push(Line::from("")); }
            lines.push(Line::from(Span::styled(format!("  {group}"), dim)));
        }
        let selected = focused && i == app.settings_sel;
        let (marker, label_style) = if selected {
            ("  ▶ ", Style::new().fg(Color::Black).bg(Color::Cyan).bold())
        } else {
            ("    ", Style::new().fg(Color::White))
        };
        let value = if selected && app.editing.is_some() {
            let buf = fit(app.editing.as_deref().unwrap_or(""), 28);
            Span::styled(
                format!("  [ {buf}█ ]{}", s.suffix()),
                Style::new().fg(Color::Yellow).bold(),
            )
        } else if selected {
            Span::styled(format!("  ‹ {} ›", fit(&s.value(app), 32)), Style::new().fg(Color::Cyan).bold())
        } else {
            Span::styled(format!("    {}", fit(&s.value(app), 32)), gray)
        };
        let mut row = vec![
            Span::styled(format!("{marker}{:<22}", s.label()), label_style),
            value,
        ];
        if !s.is_default(app) {
            row.push(Span::styled("  ●", yellow)); // differs from the default
        }
        lines.push(Line::from(row));
    }

    // The reset button: always drawn, reachable with ↓, also bound to Shift+D.
    let on_button = focused && current.is_none();
    let button_style = if on_button {
        Style::new().fg(Color::Black).bg(Color::Yellow).bold()
    } else {
        yellow
    };
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(if on_button { "  ▶ " } else { "    " }, yellow.bold()),
        Span::styled("[ ↺ Reset all to defaults ]", button_style),
        Span::styled("   [Shift+D]", dim),
    ]));

    // One fixed status line: confirm prompt, last save result, or blank.
    if app.confirm_reset {
        lines.push(Line::from(Span::styled(
            "  Reset ALL settings?  [Y/Enter] Yes   any other key cancels",
            yellow.bold(),
        )));
    } else if !app.menu_note.is_empty() {
        for l in app.menu_note.lines() {
            let color = if l.starts_with('✗') { Color::Red } else { Color::Green };
            lines.push(Line::from(Span::styled(format!("  {l}"), Style::new().fg(color))));
        }
    } else {
        lines.push(Line::from(""));
    }

    // Details for whatever has focus.
    if !focused {
        lines.push(Line::from(Span::styled(
            "  ↓ or Enter to edit  ·  then [D] resets a row, [Shift+D] resets all", dim,
        )));
    } else if let Some(cur) = current {
        lines.push(Line::from(Span::styled(format!("  {}", cur.description()), gray)));
        let mut def = vec![Span::styled(format!("  Default: {}", fit(&cur.default_value(), 36)), dim)];
        if cur.is_default(app) {
            def.push(Span::styled("   ✓ in use", dim));
        } else {
            def.push(Span::styled("   [D] Reset to default", yellow.bold()));
        }
        lines.push(Line::from(def));
        if app.editing.is_none() {
            if let Some((title, body)) = cur.preview(app) {
                lines.push(Line::from(Span::styled(format!("  {title}"), bold)));
                for l in body {
                    lines.push(Line::from(Span::styled(
                        format!("    {}", fit(&l, 64)), Style::new().fg(Color::White),
                    )));
                }
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  Puts every setting above back to its default value", gray,
        )));
    }

    f.render_widget(Paragraph::new(lines), area);
}

// ── Geometry helper ───────────────────────────────────────────────────────────
fn centered_fixed(w: u16, h: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_core_library_has_a_version() {
        for (name, _) in CORE_LIBS {
            assert_ne!(dep_version(name), "?", "`{name}` is not in [workspace.dependencies]");
        }
    }
}